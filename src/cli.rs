use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "superfuck", version, about = "LLM-powered command fixer")]
/// Top-level CLI definition for the `superfuck` binary.
pub struct Cli {
    /// Optional provider name or prefix used to override the configured default provider.
    #[arg(short = 'm', long, alias = "model", global = true)]
    pub provider: Option<String>,
    /// Selected subcommand to execute.
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
/// Supported top-level subcommands.
pub enum Commands {
    /// Request a corrected shell command from the selected provider.
    Fix(FixCommand),
    /// Render shell integration for `bash` or `zsh`.
    Init(InitCommand),
    /// Run a provider connectivity and configuration check.
    Doctor,
    /// Inspect or initialize config files.
    Config(ConfigCommand),
}

#[derive(Debug, Parser)]
/// Arguments for the `fix` subcommand.
pub struct FixCommand {
    /// Failed shell command to correct.
    #[arg(long)]
    pub command: String,
    /// Observed stderr text for the failed command.
    #[arg(long)]
    pub stderr: String,
    /// Exit code returned by the failed command, if known.
    #[arg(long)]
    pub exit_code: Option<i32>,
    /// Working directory where the command was executed.
    #[arg(long)]
    pub cwd: Option<PathBuf>,
    /// Shell dialect used to run the command.
    #[arg(long, value_enum)]
    pub shell: Option<Shell>,
    /// Enable interactive selection UI for the suggested fixes.
    #[arg(short, long)]
    pub interactive: bool,
    /// Emit the normalized response as JSON instead of human-readable text.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Parser)]
/// Arguments for the `init` subcommand.
pub struct InitCommand {
    /// Target shell for the generated integration snippet.
    #[arg(value_enum)]
    pub shell: Shell,
    /// Shell function name to generate.
    #[arg(long = "as", default_value = "fuck", value_parser = parse_shell_alias)]
    pub alias: String,
}

#[derive(Debug, Parser)]
/// Arguments for the `config` subcommand group.
pub struct ConfigCommand {
    /// Nested config action to perform.
    #[command(subcommand)]
    pub command: ConfigSubcommand,
}

#[derive(Debug, Subcommand)]
/// Supported `config` subcommands.
pub enum ConfigSubcommand {
    /// Print the resolved configuration view.
    Show,
    /// Initialize a config file or print the template.
    Init(ConfigInitCommand),
}

#[derive(Debug, Parser)]
/// Arguments for `config init`.
pub struct ConfigInitCommand {
    /// Overwrite an existing config file.
    #[arg(long)]
    pub force: bool,
    /// Print the template to stdout instead of writing it to disk.
    #[arg(long)]
    pub stdout: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ValueEnum, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
/// Shells supported by the CLI and generated integration snippets.
pub enum Shell {
    /// GNU Bash.
    Bash,
    /// Z shell.
    Zsh,
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bash => write!(f, "bash"),
            Self::Zsh => write!(f, "zsh"),
        }
    }
}

fn parse_shell_alias(value: &str) -> Result<String, String> {
    let mut chars = value.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {}
        _ => {
            return Err(
                "alias must start with an ASCII letter or underscore and contain only ASCII letters, digits, or underscores"
                    .to_string(),
            )
        }
    }

    if chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        Ok(value.to_string())
    } else {
        Err(
            "alias must start with an ASCII letter or underscore and contain only ASCII letters, digits, or underscores"
                .to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_defaults_alias_to_fuck() {
        let cli = Cli::try_parse_from(["superfuck", "init", "zsh"]).expect("parse init");
        let Commands::Init(init) = cli.command else {
            panic!("expected init command");
        };
        assert_eq!(init.alias, "fuck");
    }

    #[test]
    fn init_accepts_custom_alias() {
        let cli = Cli::try_parse_from(["superfuck", "init", "bash", "--as", "wtf"])
            .expect("parse init alias");
        let Commands::Init(init) = cli.command else {
            panic!("expected init command");
        };
        assert_eq!(init.alias, "wtf");
    }

    #[test]
    fn init_rejects_invalid_alias() {
        let err = Cli::try_parse_from(["superfuck", "init", "zsh", "--as", "wtf-now"])
            .expect_err("invalid alias should fail");
        let message = err.to_string();
        assert!(message.contains("alias must start with an ASCII letter or underscore"));

        let err = Cli::try_parse_from(["superfuck", "init", "zsh", "--as", "123wtf"])
            .expect_err("numeric alias should fail");
        assert!(
            err.to_string()
                .contains("alias must start with an ASCII letter or underscore")
        );
    }

    #[test]
    fn config_init_defaults_to_file_write_mode() {
        let cli = Cli::try_parse_from(["superfuck", "config", "init"]).expect("parse config init");
        let Commands::Config(config) = cli.command else {
            panic!("expected config command");
        };
        let ConfigSubcommand::Init(init) = config.command else {
            panic!("expected config init command");
        };
        assert!(!init.force);
        assert!(!init.stdout);
    }

    #[test]
    fn config_init_accepts_force_and_stdout() {
        let cli = Cli::try_parse_from(["superfuck", "config", "init", "--force", "--stdout"])
            .expect("parse config init flags");
        let Commands::Config(config) = cli.command else {
            panic!("expected config command");
        };
        let ConfigSubcommand::Init(init) = config.command else {
            panic!("expected config init command");
        };
        assert!(init.force);
        assert!(init.stdout);
    }
}
