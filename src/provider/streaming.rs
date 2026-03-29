use futures_util::StreamExt;
use llm::{LLMProvider, chat::{ChatMessage, Usage}};
use std::time::Instant;

use crate::error::ProviderError;

use super::{ModelProgressEvent, ModelResponse};

pub(crate) async fn collect_streamed_response(
    llm: &dyn LLMProvider,
    messages: &[ChatMessage],
    model: &str,
    started: Instant,
    on_progress: &mut dyn FnMut(ModelProgressEvent),
) -> Result<Option<ModelResponse>, ProviderError> {
    let mut stream = llm
        .chat_stream_struct(messages)
        .await
        .map_err(|err| ProviderError::RequestFailed(err.to_string()))?;
    let mut content = String::new();
    let mut saw_any_chunk = false;
    let mut time_to_first_byte_ms = None;
    let mut usage: Option<Usage> = None;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| ProviderError::RequestFailed(err.to_string()))?;
        if let Some(chunk_usage) = chunk.usage {
            usage = Some(chunk_usage);
        }

        let delta = chunk
            .choices
            .first()
            .and_then(|choice| choice.delta.content.clone());

        if let Some(delta) = delta.filter(|value| !value.is_empty()) {
            if !saw_any_chunk {
                saw_any_chunk = true;
                time_to_first_byte_ms = Some(started.elapsed().as_millis());
                on_progress(ModelProgressEvent::FirstChunk);
            }
            content.push_str(&delta);
            on_progress(ModelProgressEvent::ContentDelta(delta));
        }
    }

    let content = content.trim().to_string();
    if content.is_empty() {
        return Ok(None);
    }

    let total_latency_ms = started.elapsed().as_millis();
    Ok(Some(ModelResponse {
        content,
        model: model.to_string(),
        time_to_first_byte_ms: time_to_first_byte_ms.unwrap_or(total_latency_ms),
        total_latency_ms,
        prompt_tokens: usage.as_ref().map(|usage| usage.prompt_tokens).unwrap_or(0),
        completion_tokens: usage
            .as_ref()
            .map(|usage| usage.completion_tokens)
            .unwrap_or(0),
    }))
}
