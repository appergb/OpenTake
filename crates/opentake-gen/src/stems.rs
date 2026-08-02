//! Explicit routing policy for stem separation.
//!
//! Local execution never uploads media. Hosted execution is available only
//! after the user selects a concrete provider/model, acknowledges upload, and
//! the normal generation registry proves that provider is configured.

use crate::{GenError, ModelRoute, ProviderRegistry};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StemProviderSelection {
    Local,
    Hosted {
        provider: String,
        model: String,
        upload_confirmed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StemExecutionPlan {
    Local,
    Hosted {
        provider: String,
        model: String,
        vendor_model: String,
    },
}

pub fn resolve_stem_execution(
    selection: StemProviderSelection,
    registry: &ProviderRegistry,
) -> Result<StemExecutionPlan, GenError> {
    match selection {
        StemProviderSelection::Local => Ok(StemExecutionPlan::Local),
        StemProviderSelection::Hosted {
            provider,
            model,
            upload_confirmed,
        } => {
            if provider.trim().is_empty() || model.trim().is_empty() {
                return Err(GenError::Other(anyhow::anyhow!(
                    "stem provider and model must be selected explicitly"
                )));
            }
            if !upload_confirmed {
                return Err(GenError::Other(anyhow::anyhow!(
                    "stem upload requires explicit privacy confirmation"
                )));
            }
            if !registry.has_prefix(&provider) {
                return Err(GenError::NotConfigured);
            }
            let route = ModelRoute::parse(&model)?;
            if route.prefix != provider {
                return Err(GenError::Other(anyhow::anyhow!(
                    "stem model prefix does not match selected provider"
                )));
            }
            Ok(StemExecutionPlan::Hosted {
                provider,
                model,
                vendor_model: route.vendor_model,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::*;
    use crate::{GenerationJob, GenerationParams, ProviderAdapter};

    struct StemCapableProvider;

    #[async_trait]
    impl ProviderAdapter for StemCapableProvider {
        fn prefix(&self) -> &'static str {
            "stemhost"
        }

        async fn submit(
            &self,
            _route: &ModelRoute,
            _params: &GenerationParams,
        ) -> Result<GenerationJob, GenError> {
            unreachable!("routing test does not submit")
        }

        async fn poll(&self, _job_id: &str) -> Result<GenerationJob, GenError> {
            unreachable!("routing test does not poll")
        }

        async fn upload(
            &self,
            _path: &std::path::Path,
            _content_type: &str,
        ) -> Result<String, GenError> {
            unreachable!("routing test does not upload")
        }
    }

    #[test]
    fn local_never_requires_a_provider() {
        assert_eq!(
            resolve_stem_execution(StemProviderSelection::Local, &ProviderRegistry::new()).unwrap(),
            StemExecutionPlan::Local
        );
    }

    #[test]
    fn hosted_requires_confirmation_configuration_and_matching_prefix() {
        let registry = ProviderRegistry::new().with(Arc::new(StemCapableProvider));
        let unconfirmed = resolve_stem_execution(
            StemProviderSelection::Hosted {
                provider: "stemhost".into(),
                model: "stemhost:separate-v1".into(),
                upload_confirmed: false,
            },
            &registry,
        );
        assert!(unconfirmed.is_err());
        let plan = resolve_stem_execution(
            StemProviderSelection::Hosted {
                provider: "stemhost".into(),
                model: "stemhost:separate-v1".into(),
                upload_confirmed: true,
            },
            &registry,
        )
        .unwrap();
        assert_eq!(
            plan,
            StemExecutionPlan::Hosted {
                provider: "stemhost".into(),
                model: "stemhost:separate-v1".into(),
                vendor_model: "separate-v1".into(),
            }
        );
    }
}
