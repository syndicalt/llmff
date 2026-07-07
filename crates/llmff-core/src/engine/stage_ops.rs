use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::LlmffError;
use crate::manifest::{Manifest, StageSpec};
use crate::stage::specs::{CachePolicy, CacheSpec, LoadSpec, RouteSpec, WriteSpec};
use crate::value::{StageStatus, Value};

use super::{
    decode_input, read_input, resolve_path, serialize_value, timestamp_ms, write_output, Engine,
    StageOutcome,
};

const CACHE_RECORD_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
struct CacheRecord {
    version: u32,
    value: Value,
}

impl Engine {
    pub(super) fn execute_load(
        &self,
        manifest: &Manifest,
        stage: &StageSpec,
        cwd: &Path,
    ) -> Result<StageStatus, LlmffError> {
        let typed = LoadSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message,
        })?;
        let input_name = &typed.input_name;
        let input = manifest
            .inputs
            .get(input_name)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("unknown input `{input_name}`"),
            })?;
        let path = input
            .path
            .as_ref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "input requires path".to_string(),
            })?;
        let text = read_input(cwd, path)?;

        decode_input(stage, input_name, input.format.as_deref(), text)
    }

    pub(super) fn execute_route(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
    ) -> Result<StageStatus, LlmffError> {
        let typed = RouteSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message,
        })?;
        let source_id = stage
            .from
            .as_ref()
            .expect("RouteSpec::parse guarantees from");
        let source = statuses
            .get(source_id)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("route source `{source_id}` is not available"),
            })?;

        let selected = if let Some(field) = &typed.field {
            select_field_route(stage, field, source)?
        } else {
            select_status_route(stage, source)
        };

        let target_id = selected.ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "route did not match any target".to_string(),
        })?;
        statuses
            .get(target_id)
            .cloned()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("route target `{target_id}` is not available"),
            })
    }

    pub(super) fn execute_write(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
    ) -> Result<StageStatus, LlmffError> {
        let parent = stage
            .from
            .as_ref()
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: "write requires parent stage".to_string(),
            })?;
        let status = statuses
            .get(parent)
            .ok_or_else(|| LlmffError::StageExecution {
                stage_id: stage.id.clone(),
                message: format!("unknown parent stage `{parent}`"),
            })?;
        let value = match status {
            StageStatus::Success(value) => value,
            StageStatus::Invalid { errors, .. } => {
                return Err(LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("parent stage is invalid: {}", errors.join("; ")),
                });
            }
            StageStatus::Skipped => {
                return Err(LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: "parent stage was skipped".to_string(),
                });
            }
        };
        let typed = WriteSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message,
        })?;

        write_output(cwd, &typed.path, &serialize_value(value)?)?;

        Ok(StageStatus::Success(value.clone()))
    }

    pub(super) fn execute_cache(
        &self,
        stage: &StageSpec,
        statuses: &BTreeMap<String, StageStatus>,
        cwd: &Path,
    ) -> Result<StageOutcome, LlmffError> {
        let typed = CacheSpec::parse(stage).map_err(|message| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message,
        })?;
        let value = parent_success_value(stage, statuses)?;
        let cache_path = typed.path.unwrap_or_else(|| ".llmff/cache".to_string());
        let cache_dir = resolve_path(cwd, &cache_path);
        let cache_file = cache_dir.join(format!("{}.json", cache_digest(stage, value)?));

        if typed.cache_policy == CachePolicy::Bypass {
            return Ok(StageOutcome::with_cache(
                StageStatus::Success(value.clone()),
                false,
                cache_path,
            ));
        }

        if typed.cache_policy != CachePolicy::Refresh && cache_file.exists() {
            let source = std::fs::read_to_string(&cache_file).map_err(|error| {
                LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!(
                        "failed to read cache file `{}`: {error}",
                        cache_file.display()
                    ),
                }
            })?;
            let record: CacheRecord =
                serde_json::from_str(&source).map_err(|error| LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!("invalid cache file `{}`: {error}", cache_file.display()),
                })?;
            if record.version != CACHE_RECORD_VERSION {
                return Err(LlmffError::StageExecution {
                    stage_id: stage.id.clone(),
                    message: format!(
                        "unsupported cache file `{}` version {}",
                        cache_file.display(),
                        record.version
                    ),
                });
            }

            return Ok(StageOutcome::with_cache(
                StageStatus::Success(record.value),
                true,
                cache_path,
            ));
        }

        std::fs::create_dir_all(&cache_dir).map_err(|error| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!(
                "failed to create cache directory `{}`: {error}",
                cache_dir.display()
            ),
        })?;
        let record = CacheRecord {
            version: CACHE_RECORD_VERSION,
            value: value.clone(),
        };
        let encoded = serde_json::to_vec_pretty(&record).map_err(LlmffError::Json)?;
        write_cache_file(stage, &cache_file, &encoded)?;

        Ok(StageOutcome::with_cache(
            StageStatus::Success(value.clone()),
            false,
            cache_path,
        ))
    }
}

fn select_status_route<'a>(stage: &'a StageSpec, source: &StageStatus) -> Option<&'a str> {
    match source {
        StageStatus::Success(_) => stage.on_success.as_deref().or(stage.default.as_deref()),
        StageStatus::Invalid { .. } => stage.on_invalid.as_deref().or(stage.default.as_deref()),
        StageStatus::Skipped => stage.on_skipped.as_deref().or(stage.default.as_deref()),
    }
}

fn select_field_route<'a>(
    stage: &'a StageSpec,
    field: &str,
    source: &StageStatus,
) -> Result<Option<&'a str>, LlmffError> {
    let StageStatus::Success(Value::Json(serde_json::Value::Object(object))) = source else {
        return Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "field route requires successful JSON object source".to_string(),
        });
    };
    let value = object
        .get(field)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("field route source is missing field `{field}`"),
        })?;
    let key = route_value_key(value).ok_or_else(|| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!("field route `{field}` must be string, number, or boolean"),
    })?;

    Ok(stage
        .cases
        .get(&key)
        .map(String::as_str)
        .or(stage.default.as_deref()))
}

fn route_value_key(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(boolean) => Some(boolean.to_string()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            None
        }
    }
}

fn parent_success_value<'a>(
    stage: &StageSpec,
    statuses: &'a BTreeMap<String, StageStatus>,
) -> Result<&'a Value, LlmffError> {
    let parent = stage
        .from
        .as_ref()
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "cache requires parent stage".to_string(),
        })?;
    let status = statuses
        .get(parent)
        .ok_or_else(|| LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("unknown parent stage `{parent}`"),
        })?;

    match status {
        StageStatus::Success(value) => Ok(value),
        StageStatus::Invalid { errors, .. } => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: format!("parent stage is invalid: {}", errors.join("; ")),
        }),
        StageStatus::Skipped => Err(LlmffError::StageExecution {
            stage_id: stage.id.clone(),
            message: "parent stage was skipped".to_string(),
        }),
    }
}

fn cache_digest(stage: &StageSpec, value: &Value) -> Result<String, LlmffError> {
    let preimage = if let Some(key) = &stage.key {
        serde_json::json!({
            "version": CACHE_RECORD_VERSION,
            "stage_id": stage.id,
            "key": key,
        })
    } else {
        serde_json::json!({
            "version": CACHE_RECORD_VERSION,
            "stage_id": stage.id,
            "key": stage.id,
            "value": value,
        })
    };
    let encoded = serde_json::to_vec(&preimage).map_err(LlmffError::Json)?;
    let digest = Sha256::digest(encoded);

    Ok(digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>())
}

fn write_cache_file(
    stage: &StageSpec,
    cache_file: &Path,
    encoded: &[u8],
) -> Result<(), LlmffError> {
    let tmp_file = cache_file.with_extension(format!(
        "json.tmp.{}.{}",
        std::process::id(),
        timestamp_ms()
    ));
    std::fs::write(&tmp_file, encoded).map_err(|error| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!(
            "failed to write cache file `{}`: {error}",
            tmp_file.display()
        ),
    })?;
    std::fs::rename(&tmp_file, cache_file).map_err(|error| LlmffError::StageExecution {
        stage_id: stage.id.clone(),
        message: format!(
            "failed to move cache file `{}` into `{}`: {error}",
            tmp_file.display(),
            cache_file.display()
        ),
    })?;

    Ok(())
}
