use std::sync::Arc;

use crate::backend::Backend;
use crate::error::LlmffError;

use super::Engine;

pub(super) struct ResolvedBackendMetadata {
    pub(super) backend_alias: String,
    pub(super) provider_model: String,
}

pub(super) struct ResolvedBackend<'a> {
    pub(super) backend: &'a Arc<dyn Backend>,
    pub(super) provider_model: &'a str,
}

impl std::ops::Deref for ResolvedBackend<'_> {
    type Target = Arc<dyn Backend>;

    fn deref(&self) -> &Self::Target {
        self.backend
    }
}

impl Engine {
    pub(super) fn backend_for_model<'a>(
        &'a self,
        model: &'a str,
    ) -> Result<ResolvedBackend<'a>, LlmffError> {
        if let Some(backend) = self.backends.get(model) {
            return Ok(ResolvedBackend {
                backend,
                provider_model: model,
            });
        }

        if let Some((alias, provider_model)) = model.split_once(':') {
            if let Some(backend) = self.backends.get(alias) {
                return Ok(ResolvedBackend {
                    backend,
                    provider_model,
                });
            }
        }

        Err(LlmffError::Backend(format!(
            "no backend configured for `{model}`"
        )))
    }

    pub(super) fn resolve_backend_metadata(&self, model: &str) -> Option<ResolvedBackendMetadata> {
        if self.backends.contains_key(model) {
            return Some(ResolvedBackendMetadata {
                backend_alias: model.to_string(),
                provider_model: model.to_string(),
            });
        }

        let (alias, provider_model) = model.split_once(':')?;
        self.backends
            .contains_key(alias)
            .then(|| ResolvedBackendMetadata {
                backend_alias: alias.to_string(),
                provider_model: provider_model.to_string(),
            })
    }
}
