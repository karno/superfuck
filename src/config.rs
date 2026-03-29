use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;

use crate::constants::{
    DEFAULT_MAX_ALTERNATIVES, DEFAULT_TIMEOUT_SECS, SYSTEM_PROMPT, default_high_risk_patterns,
    default_medium_risk_patterns,
};
use crate::error::ConfigError;

const DEFAULT_CONFIG_TEMPLATE: &str = r#"# Provider names are your selectors for `fuck <provider>` and `superfuck -m <provider>`.
# Export the matching API key env vars before use.

default_provider = "chatgpt"
max_alternatives = 3
# language = "Japanese"
# high_risk_patterns = ["rm -rf", "rm -r", "mkfs", "dd ", "git reset --hard", "git clean -fd", "docker system prune", "kubectl delete", "terraform destroy", "npm uninstall", "brew uninstall", ":(){", ">/dev/", "> /", "--force"]
# medium_risk_patterns = ["sudo ", "chmod ", "chown ", "mv ", "cp ", ">"]

[providers.chatgpt]
backend = "openai"
model = "gpt-5.4-mini"
api_key_env = "OPENAI_API_KEY"
timeout_secs = 20

[providers.claude]
backend = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_API_KEY"
timeout_secs = 20

[providers.gemini]
backend = "google"
model = "gemini-2.5-flash"
api_key_env = "GEMINI_API_KEY"
timeout_secs = 20

[providers.local]
backend = "openai_compatible"
base_url = "http://127.0.0.1:8080/v1"
model = "local-model"
timeout_secs = 20
"#;

#[derive(Debug, Clone, Deserialize, Serialize)]
/// Fully loaded application configuration.
pub struct Config {
    /// Provider name selected when no CLI override is supplied.
    pub default_provider: String,
    /// Named provider configurations keyed by selector name.
    pub providers: IndexMap<String, ProviderConfig>,
    /// Maximum number of alternative fixes to keep.
    pub max_alternatives: usize,
    /// Optional language for human-readable explanations.
    pub language: Option<String>,
    /// Base system prompt used for fix generation.
    pub system_prompt: String,
    /// Patterns that classify a command as high risk.
    pub high_risk_patterns: Vec<String>,
    /// Patterns that classify a command as medium risk.
    pub medium_risk_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// Supported provider backend families.
pub enum ProviderBackend {
    /// OpenAI native backend.
    Openai,
    /// Anthropic native backend.
    Anthropic,
    /// Google native backend.
    Google,
    /// OpenAI-compatible HTTP backend.
    OpenaiCompatible,
}

impl ProviderBackend {
    /// Return the default API key environment variable for this backend.
    pub fn default_api_key_env(self) -> Option<&'static str> {
        match self {
            Self::Openai => Some("OPENAI_API_KEY"),
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::Google => Some("GEMINI_API_KEY"),
            Self::OpenaiCompatible => None,
        }
    }
}

impl fmt::Display for ProviderBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Openai => write!(f, "openai"),
            Self::Anthropic => write!(f, "anthropic"),
            Self::Google => write!(f, "google"),
            Self::OpenaiCompatible => write!(f, "openai_compatible"),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
/// Configuration for a single named provider.
pub struct ProviderConfig {
    /// Backend family used to create the provider client.
    pub backend: ProviderBackend,
    /// Model identifier sent to the backend.
    pub model: String,
    /// Optional environment variable holding the API key.
    pub api_key_env: Option<String>,
    /// Request timeout in seconds.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Base URL for OpenAI-compatible providers.
    pub base_url: Option<String>,
}

fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    default_provider: Option<String>,
    providers: Option<IndexMap<String, ProviderConfig>>,
    max_alternatives: Option<usize>,
    language: Option<String>,
    system_prompt: Option<String>,
    high_risk_patterns: Option<Vec<String>>,
    medium_risk_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
/// Resolved configuration view returned by `config show`.
pub struct ResolvedConfigView {
    /// Absolute path to the config file that was loaded.
    pub config_path: PathBuf,
    /// Provider name selected by default.
    pub default_provider: String,
    /// Provider name selected after applying CLI overrides.
    pub selected_provider: String,
    /// Resolved provider information keyed by provider name.
    pub providers: IndexMap<String, ResolvedProviderView>,
    /// Maximum number of alternatives to keep.
    pub max_alternatives: usize,
    /// Optional explanation language.
    pub language: Option<String>,
    /// Effective system prompt.
    pub system_prompt: String,
    /// Effective high-risk command patterns.
    pub high_risk_patterns: Vec<String>,
    /// Effective medium-risk command patterns.
    pub medium_risk_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
/// Resolved public view for a single provider entry.
pub struct ResolvedProviderView {
    /// Backend family used by the provider.
    pub backend: ProviderBackend,
    /// Model identifier configured for the provider.
    pub model: String,
    /// API key environment variable name, if one is required.
    pub api_key_env: Option<String>,
    /// Whether the configured API key environment variable is currently set.
    pub api_key_present: bool,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Base URL for OpenAI-compatible providers.
    pub base_url: Option<String>,
}

#[derive(Debug, Clone)]
/// Selected provider name and borrowed configuration.
pub struct SelectedProvider<'a> {
    /// Resolved provider name.
    pub name: String,
    /// Borrowed provider configuration for the resolved name.
    pub provider: &'a ProviderConfig,
}

/// Load, validate, and resolve the application configuration from disk.
pub fn load_config() -> Result<(Config, PathBuf), ConfigError> {
    let config_path = config_path()?;
    let file_config = if config_path.exists() {
        let raw = fs::read_to_string(&config_path)?;
        toml::from_str::<FileConfig>(&raw)?
    } else {
        FileConfig::default()
    };

    let config = Config {
        default_provider: file_config
            .default_provider
            .ok_or(ConfigError::MissingDefaultProvider)?,
        providers: file_config
            .providers
            .ok_or(ConfigError::NoProvidersConfigured)?,
        max_alternatives: env::var("SUPERFUCK_MAX_ALTERNATIVES")
            .ok()
            .and_then(|value| value.parse().ok())
            .or(file_config.max_alternatives)
            .unwrap_or(DEFAULT_MAX_ALTERNATIVES),
        language: env::var("SUPERFUCK_LANGUAGE")
            .ok()
            .or(file_config.language)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        system_prompt: env::var("SUPERFUCK_SYSTEM_PROMPT")
            .ok()
            .or(file_config.system_prompt)
            .unwrap_or_else(|| SYSTEM_PROMPT.to_string()),
        high_risk_patterns: file_config
            .high_risk_patterns
            .unwrap_or_else(default_high_risk_patterns),
        medium_risk_patterns: file_config
            .medium_risk_patterns
            .unwrap_or_else(default_medium_risk_patterns),
    };

    if config.providers.is_empty() {
        return Err(ConfigError::NoProvidersConfigured);
    }

    for (name, provider) in &config.providers {
        validate_provider_config(name, provider)?;
    }

    resolve_provider_name(&config, None)?;

    Ok((config, config_path))
}

fn validate_provider_config(name: &str, provider: &ProviderConfig) -> Result<(), ConfigError> {
    match provider.backend {
        ProviderBackend::OpenaiCompatible => {
            let base_url = provider.base_url.as_deref().map(str::trim).unwrap_or_default();
            if base_url.is_empty() {
                return Err(ConfigError::ConfigValidation(format!(
                    "provider `{name}` requires `base_url` when backend = \"openai_compatible\""
                )));
            }
        }
        _ => {
            if provider.base_url.is_some() {
                return Err(ConfigError::ConfigValidation(format!(
                    "provider `{name}` cannot set `base_url` unless backend = \"openai_compatible\""
                )));
            }
        }
    }
    Ok(())
}

/// Return the default config template as a string.
pub fn default_config_template() -> String {
    DEFAULT_CONFIG_TEMPLATE.to_string()
}

/// Initialize the config file on disk and return its path.
pub fn init_config(force: bool) -> Result<PathBuf, ConfigError> {
    let path = config_path()?;
    write_config_template(&path, force)?;
    Ok(path)
}

fn write_config_template(path: &PathBuf, force: bool) -> Result<(), ConfigError> {
    if path.exists() && !force {
        return Err(ConfigError::ConfigAlreadyExists(path.clone()));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, default_config_template())?;
    Ok(())
}

/// Resolve a provider name from CLI input, environment, or config default.
pub fn resolve_provider_name(
    config: &Config,
    cli_provider: Option<&str>,
) -> Result<String, ConfigError> {
    let requested = cli_provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| env::var("SUPERFUCK_PROVIDER").ok())
        .unwrap_or_else(|| config.default_provider.clone());

    if requested.is_empty() {
        return Err(ConfigError::MissingDefaultProvider);
    }

    if config.providers.contains_key(&requested) {
        return Ok(requested);
    }

    config
        .providers
        .iter()
        .find(|(name, _)| name.starts_with(&requested))
        .map(|(name, _)| name.clone())
        .ok_or(ConfigError::UnknownProvider(requested))
}

/// Return the resolved provider entry for the current invocation.
pub fn select_provider<'a>(
    config: &'a Config,
    cli_provider: Option<&str>,
) -> Result<SelectedProvider<'a>, ConfigError> {
    let name = resolve_provider_name(config, cli_provider)?;
    let provider = config
        .providers
        .get(&name)
        .ok_or_else(|| ConfigError::UnknownProvider(name.clone()))?;
    Ok(SelectedProvider { name, provider })
}

/// Build the user-facing resolved config view for `config show`.
pub fn resolved_config_view(
    config: &Config,
    config_path: PathBuf,
    cli_provider: Option<&str>,
) -> Result<ResolvedConfigView, ConfigError> {
    let selected_provider = resolve_provider_name(config, cli_provider)?;
    let providers = config
        .providers
        .iter()
        .map(|(name, provider)| {
            (
                name.clone(),
                ResolvedProviderView {
                    backend: provider.backend,
                    model: provider.model.clone(),
                    api_key_env: provider.api_key_env.clone(),
                    api_key_present: provider
                        .api_key_env
                        .as_ref()
                        .and_then(|env_name| env::var(env_name).ok())
                        .map(|value| !value.is_empty())
                        .unwrap_or(false),
                    timeout_secs: provider.timeout_secs,
                    base_url: provider.base_url.clone(),
                },
            )
        })
        .collect::<IndexMap<_, _>>();

    Ok(ResolvedConfigView {
        config_path,
        default_provider: config.default_provider.clone(),
        selected_provider,
        providers,
        max_alternatives: config.max_alternatives,
        language: config.language.clone(),
        system_prompt: config.system_prompt.clone(),
        high_risk_patterns: config.high_risk_patterns.clone(),
        medium_risk_patterns: config.medium_risk_patterns.clone(),
    })
}

/// Return the filesystem path used for the config file.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    let base = if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(dir)
    } else if let Some(home) = dirs::home_dir() {
        home.join(".config")
    } else {
        dirs::config_dir().ok_or(ConfigError::MissingConfigDir)?
    };
    Ok(base.join("superfuck").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(
        backend: ProviderBackend,
        model: &str,
        api_key_env: Option<&str>,
        base_url: Option<&str>,
    ) -> ProviderConfig {
        ProviderConfig {
            backend,
            model: model.to_string(),
            api_key_env: api_key_env.map(str::to_string),
            timeout_secs: 20,
            base_url: base_url.map(str::to_string),
        }
    }

    fn test_config() -> Config {
        Config {
            default_provider: "claude".to_string(),
            providers: IndexMap::from([
                (
                    "claude".to_string(),
                    provider(
                        ProviderBackend::Anthropic,
                        "claude-model",
                        Some("CLAUDE_API_KEY"),
                        None,
                    ),
                ),
                (
                    "chatgpt".to_string(),
                    provider(
                        ProviderBackend::Openai,
                        "gpt-5.4-mini",
                        Some("OPENAI_API_KEY"),
                        None,
                    ),
                ),
                (
                    "local".to_string(),
                    provider(
                        ProviderBackend::OpenaiCompatible,
                        "local-model",
                        None,
                        Some("http://127.0.0.1:8080/v1"),
                    ),
                ),
            ]),
            max_alternatives: DEFAULT_MAX_ALTERNATIVES,
            language: None,
            system_prompt: SYSTEM_PROMPT.to_string(),
            high_risk_patterns: default_high_risk_patterns(),
            medium_risk_patterns: default_medium_risk_patterns(),
        }
    }

    #[test]
    fn cli_provider_overrides_default_provider() {
        let config = test_config();
        let selected = resolve_provider_name(&config, Some("local")).expect("provider");
        assert_eq!(selected, "local");
    }

    #[test]
    fn exact_match_beats_prefix_match() {
        let config = test_config();
        let selected = resolve_provider_name(&config, Some("chatgpt")).expect("provider");
        assert_eq!(selected, "chatgpt");
    }

    #[test]
    fn prefix_match_uses_first_provider_in_config_order() {
        let config = test_config();
        let selected = resolve_provider_name(&config, Some("c")).expect("provider");
        assert_eq!(selected, "claude");
    }

    #[test]
    fn longer_prefix_can_select_later_provider() {
        let config = test_config();
        assert_eq!(
            resolve_provider_name(&config, Some("cl")).expect("provider"),
            "claude"
        );
        assert_eq!(
            resolve_provider_name(&config, Some("ch")).expect("provider"),
            "chatgpt"
        );
    }

    #[test]
    fn unknown_provider_is_rejected() {
        let config = test_config();
        let err = resolve_provider_name(&config, Some("missing")).expect_err("should fail");
        assert!(matches!(err, ConfigError::UnknownProvider(name) if name == "missing"));
    }

    #[test]
    fn default_config_template_includes_default_providers() {
        let template = default_config_template();
        assert!(template.contains("backend = \"openai\""));
        assert!(template.contains("backend = \"anthropic\""));
        assert!(template.contains("backend = \"google\""));
        assert!(template.contains("backend = \"openai_compatible\""));
        assert!(template.contains("[providers.local]"));
    }

    #[test]
    fn openai_compatible_requires_base_url() {
        let err = validate_provider_config(
            "local",
            &provider(ProviderBackend::OpenaiCompatible, "local-model", None, None),
        )
        .expect_err("missing base_url");
        assert_eq!(
            err.to_string(),
            "invalid config: provider `local` requires `base_url` when backend = \"openai_compatible\""
        );
    }

    #[test]
    fn native_backends_reject_base_url() {
        let err = validate_provider_config(
            "chatgpt",
            &provider(
                ProviderBackend::Openai,
                "gpt-5.4-mini",
                Some("OPENAI_API_KEY"),
                Some("https://api.openai.com/v1"),
            ),
        )
        .expect_err("unexpected base_url");
        assert_eq!(
            err.to_string(),
            "invalid config: provider `chatgpt` cannot set `base_url` unless backend = \"openai_compatible\""
        );
    }

    #[test]
    fn init_config_writes_template() {
        let temp_base =
            env::temp_dir().join(format!("superfuck-config-init-{}", std::process::id()));
        if temp_base.exists() {
            fs::remove_dir_all(&temp_base).expect("cleanup stale temp dir");
        }
        fs::create_dir_all(&temp_base).expect("create temp dir");
        let path = temp_base.join("superfuck").join("config.toml");

        write_config_template(&path, false).expect("write config");
        let written = fs::read_to_string(&path).expect("read config");

        assert_eq!(written, default_config_template());

        fs::remove_dir_all(&temp_base).expect("cleanup temp dir");
    }

    #[test]
    fn init_config_rejects_existing_file_without_force() {
        let temp_base = env::temp_dir().join(format!(
            "superfuck-config-init-existing-{}",
            std::process::id()
        ));
        if temp_base.exists() {
            fs::remove_dir_all(&temp_base).expect("cleanup stale temp dir");
        }
        fs::create_dir_all(temp_base.join("superfuck")).expect("create config dir");
        let path = temp_base.join("superfuck").join("config.toml");
        fs::write(&path, "existing").expect("seed config");

        let err = write_config_template(&path, false).expect_err("existing config should fail");
        assert!(matches!(err, ConfigError::ConfigAlreadyExists(existing) if existing == path));

        fs::remove_dir_all(&temp_base).expect("cleanup temp dir");
    }
}
