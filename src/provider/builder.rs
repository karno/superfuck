use llm::{
    LLMProvider,
    builder::{LLMBackend, LLMBuilder},
    chat::ChatMessage,
};
use std::env;

use crate::config::{ProviderBackend, ProviderConfig};
use crate::error::{AppError, ConfigError, ProviderError};

const DEFAULT_MAX_TOKENS: u32 = 256;
pub(crate) const OPENAI_COMPATIBLE_PLACEHOLDER_KEY: &str = "local-placeholder-key";

pub(crate) fn build_messages(prompt: &str, system_prompt: &str) -> Vec<ChatMessage> {
    vec![ChatMessage::user()
        .content(format!("{system_prompt}\n\n{prompt}"))
        .build()]
}

pub(crate) fn build_provider(provider: &ProviderConfig) -> Result<Box<dyn LLMProvider>, AppError> {
    let api_key = resolve_api_key(provider)?;
    let normalize_response = !matches!(provider.backend, ProviderBackend::OpenaiCompatible);
    let mut builder = LLMBuilder::new()
        .backend(match provider.backend {
            ProviderBackend::Openai | ProviderBackend::OpenaiCompatible => LLMBackend::OpenAI,
            ProviderBackend::Anthropic => LLMBackend::Anthropic,
            ProviderBackend::Google => LLMBackend::Google,
        })
        .api_key(api_key)
        .model(provider.model.clone())
        .max_tokens(DEFAULT_MAX_TOKENS)
        .timeout_seconds(provider.timeout_secs)
        // Some OpenAI-compatible local servers include `tool_calls: []` in every
        // stream delta. With llm's normalization enabled, those empty arrays can
        // cause content-only chunks to be discarded.
        .normalize_response(normalize_response);

    if matches!(provider.backend, ProviderBackend::OpenaiCompatible) {
        builder = builder.base_url(
            provider
                .base_url
                .clone()
                .expect("validated openai_compatible providers have base_url"),
        );
    }

    builder
        .build()
        .map_err(|err| ProviderError::RequestFailed(err.to_string()).into())
}

pub(crate) fn resolve_api_key(provider: &ProviderConfig) -> Result<String, ConfigError> {
    if let Some(env_name) = &provider.api_key_env {
        let key = env::var(env_name).map_err(|_| ConfigError::MissingApiKey(env_name.clone()))?;
        if key.is_empty() {
            return Err(ConfigError::MissingApiKey(env_name.clone()));
        }
        return Ok(key);
    }

    if matches!(provider.backend, ProviderBackend::OpenaiCompatible) {
        return Ok(OPENAI_COMPATIBLE_PLACEHOLDER_KEY.to_string());
    }

    Err(ConfigError::MissingApiKey(
        provider
            .backend
            .default_api_key_env()
            .unwrap_or("API_KEY")
            .to_string(),
    ))
}
