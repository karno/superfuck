#[derive(Debug, Clone, PartialEq, Eq)]
/// Process exit codes returned by the CLI.
pub enum ExitCode {
    /// Successful execution.
    Success = 0,
    /// The model did not produce any usable suggestion.
    NoSuggestion = 3,
    /// Configuration or local setup error.
    ConfigError = 10,
    /// Remote provider or model response error.
    ProviderError = 11,
}

#[derive(Debug, thiserror::Error)]
/// Errors produced while loading, validating, or resolving configuration.
pub enum ConfigError {
    #[error("failed to read config: {0}")]
    ConfigRead(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    ConfigParse(#[from] toml::de::Error),
    #[error("home config directory is unavailable")]
    MissingConfigDir,
    #[error("config file already exists: {}", .0.display())]
    ConfigAlreadyExists(std::path::PathBuf),
    #[error("default_provider is not configured")]
    MissingDefaultProvider,
    #[error("provider `{0}` is not configured")]
    UnknownProvider(String),
    #[error("no providers are configured")]
    NoProvidersConfigured,
    #[error("invalid config: {0}")]
    ConfigValidation(String),
    #[error("missing api key in env var {0}")]
    MissingApiKey(String),
}

#[derive(Debug, thiserror::Error)]
/// Errors produced while talking to the configured model provider.
pub enum ProviderError {
    #[error("provider request failed: {0}")]
    RequestFailed(String),
    #[error("gateway returned invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, thiserror::Error)]
/// Top-level application error used by the CLI boundary.
pub enum AppError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("no suggestion returned by model")]
    NoSuggestion { raw_output: Option<String> },
}
