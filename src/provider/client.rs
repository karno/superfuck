use crate::config::ProviderConfig;
use crate::constants::SYSTEM_PROMPT;
use crate::error::{AppError, ProviderError};
use async_trait::async_trait;
use llm::{LLMProvider, chat::ChatMessage};
use std::time::Instant;

use super::{ModelClient, ModelProgressEvent, ModelResponse};
use super::builder::{build_messages, build_provider};
use super::streaming::collect_streamed_response;

/// Concrete model client backed by a resolved provider configuration.
pub struct ProviderClient {
    provider: ProviderConfig,
    llm: Box<dyn LLMProvider>,
}

impl ProviderClient {
    /// Build a provider client from a resolved provider configuration.
    pub fn new(provider: ProviderConfig) -> Result<Self, AppError> {
        let llm = build_provider(&provider)?;
        Ok(Self { provider, llm })
    }

    async fn complete_non_streaming(
        &self,
        messages: &[ChatMessage],
        started: Instant,
    ) -> Result<ModelResponse, AppError> {
        let response = self
            .llm
            .chat(messages)
            .await
            .map_err(|err| ProviderError::RequestFailed(err.to_string()))?;

        let total_latency_ms = started.elapsed().as_millis();
        let content = response
            .text()
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| ProviderError::InvalidResponse("missing response message content".into()))?;

        let usage = response.usage();
        Ok(ModelResponse {
            content,
            model: self.provider.model.clone(),
            time_to_first_byte_ms: total_latency_ms,
            total_latency_ms,
            prompt_tokens: usage.as_ref().map(|usage| usage.prompt_tokens).unwrap_or(0),
            completion_tokens: usage
                .as_ref()
                .map(|usage| usage.completion_tokens)
                .unwrap_or(0),
        })
    }
}

#[async_trait(?Send)]
impl ModelClient for ProviderClient {
    async fn complete_with_progress(
        &self,
        prompt: &str,
        system_prompt: &str,
        on_progress: &mut dyn FnMut(ModelProgressEvent),
    ) -> Result<ModelResponse, AppError> {
        let started = Instant::now();
        let messages = build_messages(prompt, system_prompt);

        match collect_streamed_response(
            self.llm.as_ref(),
            &messages,
            &self.provider.model,
            started,
            on_progress,
        )
        .await
        {
            Ok(Some(response)) => Ok(response),
            Ok(None) | Err(_) => self.complete_non_streaming(&messages, started).await,
        }
    }

    async fn healthcheck(&self) -> Result<ModelResponse, AppError> {
        let prompt = "Return {\"r\":\"healthcheck\",\"f\":[{\"c\":\"echo ok\",\"d\":\"ok\"}]}";
        let response = self.complete(prompt, SYSTEM_PROMPT).await?;
        if response.content.is_empty() {
            return Err(ProviderError::InvalidResponse("empty healthcheck response".into()).into());
        }
        Ok(response)
    }
}
