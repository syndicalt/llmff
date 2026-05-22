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
            "schema" => stage.schema = Some(value),
            "schema_path" => stage.schema_path = Some(value),
            "path" => stage.path = Some(value),
            other => {
                return Err(inline_graph_error(format!(
                    "unknown inline graph parameter `{other}`"
                )));
            }
        }
    }

    Ok(())
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
            "load | infer(model=mock:good,temperature=0.2,top_p=0.9,max_tokens=256) | write(-)",
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
    fn rejects_empty_inline_stage() {
        let error = Manifest::from_inline_graph("load | | write(-)", None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("inline graph contains an empty stage"));
    }
}
