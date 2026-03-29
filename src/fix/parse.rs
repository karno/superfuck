use crate::error::ProviderError;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
/// Wire-format model output parsed from the raw LLM response.
pub struct ModelOutput {
    /// Optional short explanation for the failure.
    #[serde(rename = "r", alias = "reason")]
    pub reason: Option<String>,
    /// Ordered list of suggested fixes from the model.
    #[serde(rename = "f", alias = "fixes")]
    pub fixes: Vec<ModelFix>,
}

#[derive(Debug, Deserialize)]
/// Wire-format fix candidate parsed from model JSON.
pub struct ModelFix {
    /// Suggested command string.
    #[serde(rename = "c", alias = "command")]
    pub command: String,
    /// Optional short explanation for the suggested command.
    #[serde(rename = "d", alias = "description")]
    pub description: Option<String>,
}

/// Parse raw model text into the wire-format fix output.
pub fn parse_model_output(raw: &str) -> Result<ModelOutput, ProviderError> {
    let json_str = extract_json_object(raw);
    serde_json::from_str(json_str).map_err(|err| {
        ProviderError::InvalidResponse(format!(
            "{}\n--- raw output ---\n'{}'\n--- end raw output ---",
            err, raw
        ))
    })
}

fn extract_json_object(raw: &str) -> &str {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if s <= e => &raw[s..=e],
        _ => raw,
    }
}
