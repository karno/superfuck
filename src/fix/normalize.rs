use crate::config::Config;
use crate::error::{AppError, ProviderError};
use crate::models::{FixCandidate, FixResponse, RiskLevel};
use crate::provider::ModelResponse;

use super::parse::ModelOutput;

pub(crate) fn build_fix_response(
    parsed: ModelOutput,
    raw_response: ModelResponse,
    config: &Config,
) -> Result<FixResponse, AppError> {
    let primary_candidate = parsed.fixes.first().ok_or(AppError::NoSuggestion {
        raw_output: None,
    })?;

    let primary_command = normalize_candidate(primary_candidate.command.clone())?;
    let primary_fix = FixCandidate {
        risk_level: classify_risk(
            &primary_command,
            &config.high_risk_patterns,
            &config.medium_risk_patterns,
        ),
        description: normalize_optional_text(primary_candidate.description.clone()),
        command: primary_command,
    };

    let alternatives = parsed
        .fixes
        .into_iter()
        .skip(1)
        .filter_map(|candidate| {
            let cmd = normalize_candidate(candidate.command).ok()?;
            Some(FixCandidate {
                risk_level: classify_risk(
                    &cmd,
                    &config.high_risk_patterns,
                    &config.medium_risk_patterns,
                ),
                description: normalize_optional_text(candidate.description),
                command: cmd,
            })
        })
        .filter(|candidate| candidate.command != primary_fix.command)
        .take(config.max_alternatives)
        .collect::<Vec<_>>();

    Ok(FixResponse {
        schema_version: 2,
        reason: normalize_optional_text(parsed.reason),
        primary_fix,
        alternatives,
        model: raw_response.model,
        time_to_first_byte_ms: raw_response.time_to_first_byte_ms,
        total_latency_ms: raw_response.total_latency_ms,
        prompt_tokens: raw_response.prompt_tokens,
        completion_tokens: raw_response.completion_tokens,
    })
}

/// Normalize a single command candidate returned by the model.
pub fn normalize_candidate(candidate: String) -> Result<String, AppError> {
    let trimmed = candidate.trim().trim_matches('`').trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::NoSuggestion { raw_output: None });
    }
    if trimmed.contains('\n') {
        return Err(ProviderError::InvalidResponse("candidate command must be single-line".into()).into());
    }
    Ok(trimmed)
}

pub(crate) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Classify a suggested command using the configured risk pattern lists.
pub fn classify_risk(
    command: &str,
    high_risk_patterns: &[String],
    medium_risk_patterns: &[String],
) -> RiskLevel {
    let lower = command.to_ascii_lowercase();
    if high_risk_patterns
        .iter()
        .any(|pattern| lower.contains(pattern))
    {
        return RiskLevel::High;
    }
    if medium_risk_patterns
        .iter()
        .any(|pattern| lower.contains(pattern))
    {
        return RiskLevel::Medium;
    }
    RiskLevel::Low
}
