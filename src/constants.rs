/// Default OpenAI-compatible base URL used in examples and tests.
pub const DEFAULT_BASE_URL: &str = "http://127.0.0.1:4000/v1";
/// Default model name used in examples and tests.
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
/// Default request timeout in seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 20;
/// Default number of alternative fixes to request in addition to the primary fix.
pub const DEFAULT_MAX_ALTERNATIVES: usize = 3;
/// Base system prompt used for fix generation.
pub const SYSTEM_PROMPT: &str = r#"You correct failed terminal commands.
Return only short and simple JSON with the following schema:
{"r":"failing reason","f":[{"c":"fixed command","d":"fix description"},...]}
First item is primary. Omit r and d when not needed.
Do not include markdown fences or extra keys."#;

/// Built-in patterns that classify a command as high risk.
pub fn default_high_risk_patterns() -> Vec<String> {
    [
        "rm -rf",
        "rm -r",
        "mkfs",
        "dd ",
        "git reset --hard",
        "git clean -fd",
        "docker system prune",
        "kubectl delete",
        "terraform destroy",
        "npm uninstall",
        "brew uninstall",
        ":(){",
        ">/dev/",
        "> /",
        "--force",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// Built-in patterns that classify a command as medium risk.
pub fn default_medium_risk_patterns() -> Vec<String> {
    ["sudo ", "chmod ", "chown ", "mv ", "cp ", ">"]
        .into_iter()
        .map(str::to_string)
        .collect()
}
