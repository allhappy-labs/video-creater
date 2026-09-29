use crate::provider_credentials::{ProviderCredentialSource, ProviderCredentialStatus};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ProviderValidationState {
    NotChecked,
    Available,
    BalanceUnavailable,
    Missing,
    Rejected,
    Unavailable,
}

impl ProviderValidationState {
    pub fn failed(self) -> bool {
        matches!(self, Self::Missing | Self::Rejected | Self::Unavailable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderReadinessGroup {
    Configured,
    NeedsAttention,
    Optional,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub provider: String,
    pub display_name: String,
    pub credential_source: ProviderCredentialSource,
    pub configured: bool,
    pub validation_state: ProviderValidationState,
    pub account_label: Option<String>,
    pub balance_label: Option<String>,
    pub dependent_model_ids: Vec<String>,
    pub last_checked_at: Option<String>,
    pub diagnostic_code: Option<String>,
}

impl ProviderHealth {
    pub fn readiness_group(&self) -> ProviderReadinessGroup {
        if self.configured {
            if self.validation_state.failed() {
                ProviderReadinessGroup::NeedsAttention
            } else {
                ProviderReadinessGroup::Configured
            }
        } else if self.dependent_model_ids.is_empty() {
            ProviderReadinessGroup::Optional
        } else {
            ProviderReadinessGroup::NeedsAttention
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccountValidation {
    pub provider: String,
    pub validation_state: ProviderValidationState,
    pub account_label: Option<String>,
    pub balance_label: Option<String>,
    pub last_checked_at: Option<String>,
    pub diagnostic_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModelDependency {
    pub provider: String,
    pub model_id: String,
}

impl ProviderModelDependency {
    pub fn new(provider: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_id: model_id.into(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProviderHealthError {
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
}

pub fn aggregate_provider_health(
    credentials: &[ProviderCredentialStatus],
    validations: &[ProviderAccountValidation],
    dependencies: &[ProviderModelDependency],
    provider_filter: Option<&str>,
) -> Result<Vec<ProviderHealth>, ProviderHealthError> {
    let provider_filter = provider_filter
        .map(str::trim)
        .filter(|provider| !provider.is_empty());
    if let Some(provider) = provider_filter {
        if !credentials.iter().any(|status| status.provider == provider) {
            return Err(ProviderHealthError::UnsupportedProvider(
                provider.to_string(),
            ));
        }
    }

    let validations = validations
        .iter()
        .map(|validation| (validation.provider.as_str(), validation))
        .collect::<BTreeMap<_, _>>();
    let mut dependencies_by_provider = BTreeMap::<&str, BTreeSet<&str>>::new();
    for dependency in dependencies {
        dependencies_by_provider
            .entry(dependency.provider.as_str())
            .or_default()
            .insert(dependency.model_id.as_str());
    }

    Ok(credentials
        .iter()
        .filter(|status| provider_filter.is_none_or(|provider| status.provider == provider))
        .map(|status| {
            let validation = validations.get(status.provider.as_str()).copied();
            let validation_state = validation
                .map(|validation| validation.validation_state)
                .unwrap_or_else(|| inferred_validation_state(status));
            let dependent_model_ids = dependencies_by_provider
                .get(status.provider.as_str())
                .into_iter()
                .flat_map(|model_ids| model_ids.iter())
                .map(|model_id| (*model_id).to_string())
                .collect::<Vec<_>>();
            let diagnostic_code = validation
                .and_then(|validation| validation.diagnostic_code.clone())
                .or_else(|| default_diagnostic_code(validation_state, &dependent_model_ids));

            ProviderHealth {
                provider: status.provider.clone(),
                display_name: status.display_name.clone(),
                credential_source: status.source,
                configured: status.configured,
                validation_state,
                account_label: validation.and_then(|validation| validation.account_label.clone()),
                balance_label: validation.and_then(|validation| validation.balance_label.clone()),
                dependent_model_ids,
                last_checked_at: validation
                    .and_then(|validation| validation.last_checked_at.clone()),
                diagnostic_code,
            }
        })
        .collect())
}

fn inferred_validation_state(status: &ProviderCredentialStatus) -> ProviderValidationState {
    match status.source {
        ProviderCredentialSource::Keychain => ProviderValidationState::NotChecked,
        ProviderCredentialSource::Missing => ProviderValidationState::Missing,
        ProviderCredentialSource::Unavailable => ProviderValidationState::Unavailable,
    }
}

fn default_diagnostic_code(
    validation_state: ProviderValidationState,
    dependent_model_ids: &[String],
) -> Option<String> {
    match validation_state {
        ProviderValidationState::Rejected => Some("providers.credentialRejected".to_string()),
        ProviderValidationState::Unavailable => Some("providers.validationUnavailable".to_string()),
        ProviderValidationState::Missing if !dependent_model_ids.is_empty() => {
            Some("providers.credentialMissing".to_string())
        }
        ProviderValidationState::NotChecked
        | ProviderValidationState::Available
        | ProviderValidationState::BalanceUnavailable
        | ProviderValidationState::Missing => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_credentials::{ProviderCredentialSource, ProviderCredentialStatus};
    use serde_json::Value;

    fn credential_status(
        provider: &str,
        configured: bool,
        source: ProviderCredentialSource,
    ) -> ProviderCredentialStatus {
        ProviderCredentialStatus {
            provider: provider.to_string(),
            display_name: provider.to_string(),
            configured,
            source,
        }
    }

    fn validation(
        provider: &str,
        validation_state: ProviderValidationState,
    ) -> ProviderAccountValidation {
        ProviderAccountValidation {
            provider: provider.to_string(),
            validation_state,
            account_label: Some("Fixture account".to_string()),
            balance_label: Some("12.50 USD".to_string()),
            last_checked_at: Some("2026-07-17T10:00:00Z".to_string()),
            diagnostic_code: None,
        }
    }

    fn assert_secret_free(value: &Value, fixture_secrets: &[&str]) {
        match value {
            Value::Object(entries) => {
                for (key, child) in entries {
                    let normalized = key.to_ascii_lowercase();
                    assert!(
                        !matches!(
                            normalized.as_str(),
                            "secret" | "token" | "apikey" | "credentialvalue"
                        ),
                        "secret-shaped key was serialized: {key}"
                    );
                    assert_secret_free(child, fixture_secrets);
                }
            }
            Value::Array(values) => {
                for child in values {
                    assert_secret_free(child, fixture_secrets);
                }
            }
            Value::String(value) => {
                for fixture_secret in fixture_secrets {
                    assert_ne!(
                        value, fixture_secret,
                        "raw fixture credential was serialized"
                    );
                    assert!(
                        !value.contains(fixture_secret),
                        "fixture credential was embedded in serialized output"
                    );
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    #[test]
    fn aggregation_applies_configured_attention_and_optional_group_rules() {
        let credentials = vec![
            credential_status("configured", true, ProviderCredentialSource::Keychain),
            credential_status("rejected", true, ProviderCredentialSource::Keychain),
            credential_status("required", false, ProviderCredentialSource::Missing),
            credential_status("optional", false, ProviderCredentialSource::Missing),
        ];
        let validations = vec![
            validation("configured", ProviderValidationState::Available),
            validation("rejected", ProviderValidationState::Rejected),
            validation("required", ProviderValidationState::Missing),
            validation("optional", ProviderValidationState::Missing),
        ];
        let dependencies = vec![
            ProviderModelDependency::new("configured", "configured:model-a"),
            ProviderModelDependency::new("rejected", "rejected:model-b"),
            ProviderModelDependency::new("required", "required:model-c"),
        ];

        let health = aggregate_provider_health(&credentials, &validations, &dependencies, None)
            .expect("aggregate provider health");
        let by_provider = health
            .iter()
            .map(|health| (health.provider.as_str(), health.readiness_group()))
            .collect::<std::collections::BTreeMap<_, _>>();

        assert_eq!(
            by_provider["configured"],
            ProviderReadinessGroup::Configured
        );
        assert_eq!(
            by_provider["rejected"],
            ProviderReadinessGroup::NeedsAttention
        );
        assert_eq!(
            by_provider["required"],
            ProviderReadinessGroup::NeedsAttention
        );
        assert_eq!(by_provider["optional"], ProviderReadinessGroup::Optional);
    }

    #[test]
    fn aggregation_sorts_and_deduplicates_enabled_model_dependencies() {
        let credentials = vec![credential_status(
            "fal.ai",
            true,
            ProviderCredentialSource::Keychain,
        )];
        let validations = vec![validation(
            "fal.ai",
            ProviderValidationState::BalanceUnavailable,
        )];
        let dependencies = vec![
            ProviderModelDependency::new("fal.ai", "fal.ai:z-model"),
            ProviderModelDependency::new("fal.ai", "fal.ai:a-model"),
            ProviderModelDependency::new("fal.ai", "fal.ai:a-model"),
        ];

        let health = aggregate_provider_health(&credentials, &validations, &dependencies, None)
            .expect("aggregate provider health");

        assert_eq!(
            health[0].dependent_model_ids,
            vec!["fal.ai:a-model", "fal.ai:z-model"]
        );
        assert_eq!(
            health[0].credential_source,
            ProviderCredentialSource::Keychain
        );
        assert_eq!(health[0].account_label.as_deref(), Some("Fixture account"));
        assert_eq!(health[0].balance_label.as_deref(), Some("12.50 USD"));
    }

    #[test]
    fn provider_filter_rejects_unknown_provider_and_returns_only_requested_provider() {
        let credentials = vec![
            credential_status("fal.ai", false, ProviderCredentialSource::Missing),
            credential_status("openai", false, ProviderCredentialSource::Missing),
        ];

        let filtered = aggregate_provider_health(&credentials, &[], &[], Some("openai"))
            .expect("filter provider");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].provider, "openai");
        assert_eq!(
            filtered[0].validation_state,
            ProviderValidationState::Missing
        );

        assert_eq!(
            aggregate_provider_health(&credentials, &[], &[], Some("unknown")),
            Err(ProviderHealthError::UnsupportedProvider(
                "unknown".to_string()
            ))
        );
    }

    #[test]
    fn serialized_provider_health_recursively_excludes_secret_keys_and_fixture_values() {
        let fixture_secrets = [
            "fixture-keychain-secret-61a0",
            "fixture-environment-token-bb20",
        ];
        let credentials = vec![credential_status(
            "openai",
            true,
            ProviderCredentialSource::Keychain,
        )];
        let validations = vec![validation(
            "openai",
            ProviderValidationState::BalanceUnavailable,
        )];
        let dependencies = vec![ProviderModelDependency::new("openai", "openai:gpt-image-2")];
        let health = aggregate_provider_health(&credentials, &validations, &dependencies, None)
            .expect("aggregate provider health");

        let serialized = serde_json::to_value(&health).expect("serialize provider health");
        assert_secret_free(&serialized, &fixture_secrets);
        assert_eq!(
            serialized[0]["credentialSource"],
            serde_json::json!("keychain")
        );
        assert!(serialized[0].get("envVar").is_none());
    }
}
