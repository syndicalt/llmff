use std::collections::BTreeMap;

use crate::error::LlmffError;
use crate::manifest::{InputSpec, Manifest, StageSpec};

impl Manifest {
    pub fn from_inline_graph(source: &str, input_path: Option<String>) -> Result<Self, LlmffError> {
        let mut inputs = BTreeMap::new();
        let mut graph = Vec::new();
        let mut previous_id = None;

        for (index, raw_stage) in source.split('|').enumerate() {
            let parsed = parse_stage(raw_stage.trim())?;
            let id = format!("{}_{}", parsed.op, index + 1);
            let mut stage = empty_stage(id.clone(), parsed.op.clone());

            if parsed.op == "load" {
                stage.input = Some("prompt".to_string());
                inputs.insert(
                    "prompt".to_string(),
                    InputSpec {
                        path: Some(input_path.clone().unwrap_or_else(|| "-".to_string())),
                        format: None,
                    },
                );
            } else {
                stage.from = previous_id.clone();
            }

            apply_inline_params(&mut stage, parsed)?;
            previous_id = Some(id);
            graph.push(stage);
        }

        if graph.is_empty() {
            return Err(inline_graph_error("inline graph cannot be empty"));
        }

        Ok(Self {
            version: 1,
            inputs,
            graph,
            outputs: BTreeMap::new(),
        })
    }
}

#[derive(Debug, PartialEq)]
struct ParsedStage {
    op: String,
    positional: Option<String>,
    params: BTreeMap<String, String>,
}

fn parse_stage(source: &str) -> Result<ParsedStage, LlmffError> {
    if source.is_empty() {
        return Err(inline_graph_error("inline graph contains an empty stage"));
    }

    let Some(open) = source.find('(') else {
        return Ok(ParsedStage {
            op: source.to_string(),
            positional: None,
            params: BTreeMap::new(),
        });
    };
    if !source.ends_with(')') {
        return Err(inline_graph_error(format!(
            "stage `{source}` has unterminated parameters"
        )));
    }

    let op = source[..open].trim();
    if op.is_empty() {
        return Err(inline_graph_error("inline graph stage requires operation"));
    }
    let body = source[open + 1..source.len() - 1].trim();
    if body.is_empty() {
        return Ok(ParsedStage {
            op: op.to_string(),
            positional: None,
            params: BTreeMap::new(),
        });
    }
    if !body.contains('=') {
        return Ok(ParsedStage {
            op: op.to_string(),
            positional: Some(body.to_string()),
            params: BTreeMap::new(),
        });
    }

    let mut params = BTreeMap::new();
    for entry in body.split(',') {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| inline_graph_error(format!("invalid stage parameter `{entry}`")))?;
        let key = key.trim();
        let value = value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(inline_graph_error(format!(
                "invalid stage parameter `{entry}`"
            )));
        }
        params.insert(key.to_string(), value.to_string());
    }

    Ok(ParsedStage {
        op: op.to_string(),
        positional: None,
        params,
    })
}

fn apply_inline_params(stage: &mut StageSpec, parsed: ParsedStage) -> Result<(), LlmffError> {
    match parsed.op.as_str() {
        "system" | "template" | "write" => {
            if let Some(path) = parsed.positional {
                stage.path = Some(path);
            }
        }
        _ => {
            if parsed.positional.is_some() {
                return Err(inline_graph_error(format!(
                    "stage `{}` does not accept a positional argument",
                    parsed.op
                )));
            }
        }
    }

    for (key, value) in parsed.params {
        match key.as_str() {
            "model" => stage.model = Some(value),
            "temperature" => {
                stage.temperature = Some(value.parse::<f32>().map_err(|error| {
                    inline_graph_error(format!("invalid temperature `{value}`: {error}"))
                })?)
            }
            "top_p" => {
                stage.top_p = Some(value.parse::<f32>().map_err(|error| {
                    inline_graph_error(format!("invalid top_p `{value}`: {error}"))
                })?)
            }
            "max_tokens" => {
                stage.max_tokens = Some(value.parse::<u32>().map_err(|error| {
                    inline_graph_error(format!("invalid max_tokens `{value}`: {error}"))
                })?)
            }
            "stop" => {
                stage.stop = parse_semicolon_list(&value, "stop", "sequence")?;
            }
            "schema" => stage.schema = Some(value),
            "schema_path" => stage.schema_path = Some(value),
            "path" => stage.path = Some(value),
            "key" => stage.key = Some(value),
            "command" => {
                stage.command = Some(parse_command(&value)?);
            }
            "method" => stage.method = Some(value),
            "url" => stage.url = Some(value),
            "documents" => {
                stage.documents = parse_semicolon_list(&value, "documents", "path")?;
            }
            "top_k" => {
                stage.top_k = Some(value.parse::<usize>().map_err(|error| {
                    inline_graph_error(format!("invalid top_k `{value}`: {error}"))
                })?)
            }
            "strategy" => stage.strategy = Some(value),
            other => {
                if let Some(name) = other.strip_prefix("header:") {
                    if name.is_empty() {
                        return Err(inline_graph_error("inline header name cannot be empty"));
                    }
                    stage.headers.insert(name.to_string(), value);
                } else {
                    return Err(inline_graph_error(format!(
                        "unknown inline graph parameter `{other}`"
                    )));
                }
            }
        }
    }

    Ok(())
}

fn parse_command(source: &str) -> Result<Vec<String>, LlmffError> {
    let command = source
        .split(';')
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if command.is_empty() {
        return Err(inline_graph_error(
            "command must contain at least one argument",
        ));
    }

    Ok(command)
}

fn parse_semicolon_list(
    source: &str,
    field_name: &str,
    item_name: &str,
) -> Result<Vec<String>, LlmffError> {
    let values = source
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err(inline_graph_error(format!(
            "{field_name} must contain at least one {item_name}"
        )));
    }

    Ok(values)
}

fn empty_stage(id: String, op: String) -> StageSpec {
    StageSpec {
        id,
        op,
        input: None,
        from: None,
        path: None,
        model: None,
        temperature: None,
        top_p: None,
        max_tokens: None,
        stop: Vec::new(),
        schema: None,
        schema_path: None,
        when: None,
        on_success: None,
        on_invalid: None,
        on_skipped: None,
        field: None,
        cases: BTreeMap::new(),
        default: None,
        command: None,
        method: None,
        url: None,
        headers: BTreeMap::new(),
        documents: Vec::new(),
        top_k: None,
        strategy: None,
        key: None,
    }
}

fn inline_graph_error(message: impl Into<String>) -> LlmffError {
    LlmffError::GraphValidation(message.into())
}

#[cfg(test)]
mod tests {
    use crate::manifest::Manifest;

    #[test]
    fn parses_linear_inline_graph() {
        let manifest = Manifest::from_inline_graph(
            "load | infer(model=mock:good,temperature=0.2,top_p=0.9,max_tokens=256,stop=END;DONE) | write(-)",
            Some("question.txt".to_string()),
        )
        .expect("inline graph should parse");

        assert_eq!(manifest.version, 1);
        assert_eq!(
            manifest.inputs["prompt"].path.as_deref(),
            Some("question.txt")
        );
        assert!(manifest.outputs.is_empty());

        assert_eq!(manifest.graph[0].id, "load_1");
        assert_eq!(manifest.graph[0].op, "load");
        assert_eq!(manifest.graph[0].input.as_deref(), Some("prompt"));

        assert_eq!(manifest.graph[1].id, "infer_2");
        assert_eq!(manifest.graph[1].op, "infer");
        assert_eq!(manifest.graph[1].from.as_deref(), Some("load_1"));
        assert_eq!(manifest.graph[1].model.as_deref(), Some("mock:good"));
        assert_eq!(manifest.graph[1].temperature, Some(0.2));
        assert_eq!(manifest.graph[1].top_p, Some(0.9));
        assert_eq!(manifest.graph[1].max_tokens, Some(256));
        assert_eq!(manifest.graph[1].stop, vec!["END", "DONE"]);

        assert_eq!(manifest.graph[2].id, "write_3");
        assert_eq!(manifest.graph[2].op, "write");
        assert_eq!(manifest.graph[2].from.as_deref(), Some("infer_2"));
        assert_eq!(manifest.graph[2].path.as_deref(), Some("-"));
    }

    #[test]
    fn parses_path_like_positional_stages() {
        let manifest =
            Manifest::from_inline_graph("load | template(prompt.tmpl) | write(answer.txt)", None)
                .expect("inline graph should parse");

        assert_eq!(manifest.inputs["prompt"].path.as_deref(), Some("-"));
        assert_eq!(manifest.graph[1].id, "template_2");
        assert_eq!(manifest.graph[1].from.as_deref(), Some("load_1"));
        assert_eq!(manifest.graph[1].path.as_deref(), Some("prompt.tmpl"));
        assert_eq!(manifest.graph[2].id, "write_3");
        assert_eq!(manifest.graph[2].from.as_deref(), Some("template_2"));
        assert_eq!(manifest.graph[2].path.as_deref(), Some("answer.txt"));
    }

    #[test]
    fn parses_retrieve_stage_parameters() {
        let manifest = Manifest::from_inline_graph(
            "load | retrieve(documents=docs/rust.txt;docs/python.txt,top_k=1) | write(-)",
            Some("question.txt".to_string()),
        )
        .expect("inline graph should parse");

        assert_eq!(manifest.graph[1].id, "retrieve_2");
        assert_eq!(manifest.graph[1].op, "retrieve");
        assert_eq!(manifest.graph[1].from.as_deref(), Some("load_1"));
        assert_eq!(
            manifest.graph[1].documents,
            vec!["docs/rust.txt", "docs/python.txt"]
        );
        assert_eq!(manifest.graph[1].top_k, Some(1));
    }

    #[test]
    fn parses_retrieve_strategy_parameter() {
        let manifest = Manifest::from_inline_graph(
            "load | retrieve(documents=docs/rust.txt,strategy=embedding) | write(-)",
            Some("question.txt".to_string()),
        )
        .expect("inline graph should parse");

        assert_eq!(manifest.graph[1].op, "retrieve");
        assert_eq!(manifest.graph[1].strategy.as_deref(), Some("embedding"));
    }

    #[test]
    fn parses_cache_stage_parameters() {
        let manifest = Manifest::from_inline_graph(
            "load | cache(path=.llmff/cache,key=prompt-v1) | write(-)",
            Some("question.txt".to_string()),
        )
        .expect("inline graph should parse");

        assert_eq!(manifest.graph[1].id, "cache_2");
        assert_eq!(manifest.graph[1].op, "cache");
        assert_eq!(manifest.graph[1].from.as_deref(), Some("load_1"));
        assert_eq!(manifest.graph[1].path.as_deref(), Some(".llmff/cache"));
        assert_eq!(manifest.graph[1].key.as_deref(), Some("prompt-v1"));
    }

    #[test]
    fn parses_tool_stage_parameters() {
        let manifest = Manifest::from_inline_graph(
            "load | tool(command=/usr/bin/cat,method=POST,url=http://127.0.0.1:8000/process,header:content-type=application/json) | write(-)",
            Some("question.txt".to_string()),
        )
        .expect("inline graph should parse");

        assert_eq!(manifest.graph[1].id, "tool_2");
        assert_eq!(manifest.graph[1].op, "tool");
        assert_eq!(manifest.graph[1].from.as_deref(), Some("load_1"));
        assert_eq!(
            manifest.graph[1].command.as_deref(),
            Some(&["/usr/bin/cat".to_string()][..])
        );
        assert_eq!(manifest.graph[1].method.as_deref(), Some("POST"));
        assert_eq!(
            manifest.graph[1].url.as_deref(),
            Some("http://127.0.0.1:8000/process")
        );
        assert_eq!(
            manifest.graph[1].headers["content-type"],
            "application/json"
        );
    }

    #[test]
    fn rejects_empty_inline_stage() {
        let error = Manifest::from_inline_graph("load | | write(-)", None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("inline graph contains an empty stage"));
    }

    #[test]
    fn rejects_empty_inline_retrieve_documents() {
        let error = Manifest::from_inline_graph("load | retrieve(documents=;) | write(-)", None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("documents must contain at least one path"));
    }
}
