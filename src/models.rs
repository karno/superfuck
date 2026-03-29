use crate::cli::Shell;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
/// Normalized input passed to the fix pipeline.
pub struct FixRequest {
    /// Failed command line entered by the user.
    pub command: String,
    /// Stderr captured from the failed command.
    pub stderr: String,
    /// Exit code reported by the shell, if available.
    pub exit_code: Option<i32>,
    /// Working directory where the command was executed.
    pub cwd: PathBuf,
    /// Shell dialect that produced the command.
    pub shell: Option<Shell>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
/// Risk level assigned to a suggested command.
pub enum RiskLevel {
    /// Low-risk command.
    Low,
    /// Medium-risk command.
    Medium,
    /// High-risk command.
    High,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// A single candidate command returned by the fix pipeline.
pub struct FixCandidate {
    /// Suggested shell command.
    pub command: String,
    /// Optional short explanation for the command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Risk classification derived from the configured pattern lists.
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Normalized fix result returned to CLI rendering and JSON output.
pub struct FixResponse {
    /// Response schema version for serialized output.
    pub schema_version: u32,
    /// Optional short explanation of why the original command failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Primary suggested fix.
    pub primary_fix: FixCandidate,
    /// Additional candidate fixes when the primary fix is uncertain.
    pub alternatives: Vec<FixCandidate>,
    /// Model identifier reported by the selected provider.
    pub model: String,
    /// Time to first streamed content chunk in milliseconds.
    pub time_to_first_byte_ms: u128,
    /// End-to-end request latency in milliseconds.
    pub total_latency_ms: u128,
    /// Prompt token usage reported by the provider.
    pub prompt_tokens: u32,
    /// Completion token usage reported by the provider.
    pub completion_tokens: u32,
}
