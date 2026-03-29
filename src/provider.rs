mod builder;
mod client;
mod streaming;

use async_trait::async_trait;
use crate::error::AppError;

pub use client::ProviderClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// High-level phases reported while generating a fix.
pub enum ProgressPhase {
    /// Request preparation before the model call starts.
    Preparing,
    /// Waiting for the provider to begin responding.
    QueryingModel,
    /// Streaming content is being received.
    ReceivingResponse,
    /// The raw model output is being parsed into fix data.
    ParsingResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Progress events emitted by the provider client during completion.
pub enum ModelProgressEvent {
    /// The first non-empty streamed content chunk was received.
    FirstChunk,
    /// Additional streamed content was received.
    ContentDelta(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Normalized provider response consumed by the fix pipeline.
pub struct ModelResponse {
    /// Raw content returned by the model after stream aggregation or non-streaming fallback.
    pub content: String,
    /// Model identifier used for the request.
    pub model: String,
    /// Time to first streamed content chunk in milliseconds.
    pub time_to_first_byte_ms: u128,
    /// End-to-end provider latency in milliseconds.
    pub total_latency_ms: u128,
    /// Prompt token usage reported by the provider.
    pub prompt_tokens: u32,
    /// Completion token usage reported by the provider.
    pub completion_tokens: u32,
}

#[async_trait(?Send)]
/// Abstraction over model backends used by fix and doctor flows.
pub trait ModelClient {
    /// Complete a prompt without observing intermediate progress events.
    async fn complete(
        &self,
        prompt: &str,
        system_prompt: &str,
    ) -> Result<ModelResponse, AppError> {
        self.complete_with_progress(prompt, system_prompt, &mut |_| {})
            .await
    }

    /// Complete a prompt while emitting progress events for streaming UIs.
    async fn complete_with_progress(
        &self,
        prompt: &str,
        system_prompt: &str,
        on_progress: &mut dyn FnMut(ModelProgressEvent),
    ) -> Result<ModelResponse, AppError>;

    /// Run a lightweight provider healthcheck request.
    async fn healthcheck(&self) -> Result<ModelResponse, AppError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProviderBackend, ProviderConfig};

    fn provider(
        backend: ProviderBackend,
        api_key_env: Option<&str>,
        base_url: Option<&str>,
    ) -> ProviderConfig {
        ProviderConfig {
            backend,
            model: "test-model".to_string(),
            api_key_env: api_key_env.map(str::to_string),
            timeout_secs: 20,
            base_url: base_url.map(str::to_string),
        }
    }

    #[test]
    fn openai_compatible_without_api_key_uses_placeholder() {
        let provider = provider(
            ProviderBackend::OpenaiCompatible,
            None,
            Some("http://127.0.0.1:8080/v1"),
        );
        let key = builder::resolve_api_key(&provider).expect("placeholder key");
        assert_eq!(key, builder::OPENAI_COMPATIBLE_PLACEHOLDER_KEY);
    }

    #[test]
    fn openai_requires_api_key_env() {
        let provider = provider(ProviderBackend::Openai, None, None);
        let err = builder::resolve_api_key(&provider).expect_err("missing key");
        assert_eq!(err.to_string(), "missing api key in env var OPENAI_API_KEY");
    }

    #[test]
    fn build_messages_embeds_system_prompt() {
        let messages = builder::build_messages("prompt", "system");
        assert_eq!(messages.len(), 1);
        let debug = format!("{messages:?}");
        assert!(debug.contains("system"));
        assert!(debug.contains("prompt"));
    }
}
