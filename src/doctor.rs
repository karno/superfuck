use std::env;
use std::path::Path;

use crate::config::{ProviderConfig, ProviderBackend};
use crate::error::{AppError, ConfigError};
use crate::provider::ModelClient;

/// Run a lightweight diagnostic check for the selected provider.
pub async fn run_doctor<C: ModelClient + ?Sized>(
    provider_name: &str,
    client: &C,
    provider_config: &ProviderConfig,
    path: &Path,
) -> Result<String, AppError> {
    let api_key_status = match &provider_config.api_key_env {
        Some(env_name) => {
            let key = env::var(env_name).map_err(|_| ConfigError::MissingApiKey(env_name.clone()))?;
            if key.is_empty() {
                return Err(ConfigError::MissingApiKey(env_name.clone()).into());
            }
            format!("{env_name} (present)")
        }
        None if matches!(provider_config.backend, ProviderBackend::OpenaiCompatible) => {
            "none (using local placeholder key)".to_string()
        }
        None => "none".to_string(),
    };

    let health = client.healthcheck().await?;
    let endpoint = provider_config
        .base_url
        .as_deref()
        .map(|value| format!("Endpoint: {value}\n"))
        .unwrap_or_default();

    Ok(format!(
        "Config path: {}\nProvider: {}\nBackend: {}\nModel: {}\n{}API key env: {}\nProvider check: ok\nTiming: first byte {} ms, total {} ms",
        path.display(),
        provider_name,
        provider_config.backend,
        provider_config.model,
        endpoint,
        api_key_status,
        health.time_to_first_byte_ms,
        health.total_latency_ms,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::config::{Config, ProviderConfig};
    use crate::constants::{
        DEFAULT_MAX_ALTERNATIVES, SYSTEM_PROMPT, default_high_risk_patterns,
        default_medium_risk_patterns,
    };
    use crate::provider::{ModelProgressEvent, ModelResponse};
    use indexmap::IndexMap;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug)]
    struct MockClient {
        response: ModelResponse,
    }

    #[async_trait(?Send)]
    impl ModelClient for MockClient {
        async fn complete_with_progress(
            &self,
            _prompt: &str,
            _system_prompt: &str,
            _on_progress: &mut dyn FnMut(ModelProgressEvent),
        ) -> Result<ModelResponse, AppError> {
            Ok(self.response.clone())
        }

        async fn healthcheck(&self) -> Result<ModelResponse, AppError> {
            Ok(self.response.clone())
        }
    }

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    fn test_config(backend: ProviderBackend, base_url: Option<&str>) -> Config {
        Config {
            default_provider: "test".to_string(),
            providers: IndexMap::from([(
                "test".to_string(),
                ProviderConfig {
                    backend,
                    model: "test-model".to_string(),
                    api_key_env: Some("SUPERFUCK_TEST_API_KEY".to_string()),
                    timeout_secs: 5,
                    base_url: base_url.map(str::to_string),
                },
            )]),
            max_alternatives: DEFAULT_MAX_ALTERNATIVES,
            language: None,
            system_prompt: SYSTEM_PROMPT.to_string(),
            high_risk_patterns: default_high_risk_patterns(),
            medium_risk_patterns: default_medium_risk_patterns(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn doctor_detects_missing_api_key() {
        let _guard = env_lock();
        unsafe {
            env::remove_var("SUPERFUCK_TEST_API_KEY");
        }
        let config = test_config(ProviderBackend::Openai, None);
        let provider_config = config.providers.get("test").expect("provider");
        let client = MockClient {
            response: ModelResponse {
                content: "ok".to_string(),
                model: "test-model".to_string(),
                time_to_first_byte_ms: 1,
                total_latency_ms: 1,
                prompt_tokens: 0,
                completion_tokens: 0,
            },
        };
        let err = run_doctor("test", &client, provider_config, Path::new("/tmp/config.toml"))
            .await
            .expect_err("should fail");
        assert!(matches!(err, crate::error::AppError::Config(ConfigError::MissingApiKey(_))));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn doctor_uses_injected_client_healthcheck() {
        let _guard = env_lock();
        unsafe {
            env::set_var("SUPERFUCK_TEST_API_KEY", "test-key");
        }
        let config = test_config(ProviderBackend::OpenaiCompatible, Some("http://127.0.0.1:9/v1"));
        let provider_config = config.providers.get("test").expect("provider");
        let client = MockClient {
            response: ModelResponse {
                content: "ok".to_string(),
                model: "test-model".to_string(),
                time_to_first_byte_ms: 5,
                total_latency_ms: 12,
                prompt_tokens: 10,
                completion_tokens: 20,
            },
        };

        let report = run_doctor("test", &client, provider_config, Path::new("/tmp/config.toml"))
            .await
            .expect("doctor report");

        assert!(report.contains("Backend: openai_compatible"));
        assert!(report.contains("Provider check: ok"));
        assert!(report.contains("Endpoint: http://127.0.0.1:9/v1"));
        assert!(report.contains("Timing: first byte 5 ms, total 12 ms"));
    }
}
