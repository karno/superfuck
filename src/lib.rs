pub mod cli;
pub mod config;
pub mod constants;
pub mod doctor;
pub mod error;
pub mod fix;
pub mod models;
pub mod provider;
pub mod shell;

use std::env;

pub use cli::Cli;
pub use error::ExitCode;

use cli::{Commands, ConfigSubcommand};
use config::{
    default_config_template, init_config, load_config, resolved_config_view, select_provider,
};
use doctor::run_doctor;
use error::{AppError, ConfigError};
use fix::fix_command;
use models::FixRequest;
use provider::ProviderClient;
use shell::render_shell_init;

fn format_raw_output(raw_output: Option<&str>) -> String {
    match raw_output.filter(|raw| !raw.is_empty()) {
        Some(raw) => format!(
            "no suggestion returned by model\n--- raw output ---\n'{}'\n--- end raw output ---\n",
            raw
        ),
        None => "no suggestion returned by model\n".to_string(),
    }
}

/// Run the CLI command and return the exit code plus rendered output.
pub async fn run(cli: Cli) -> (ExitCode, String) {
    match run_inner(cli).await {
        Ok(output) => (ExitCode::Success, output),
        Err(AppError::NoSuggestion { raw_output }) => (
            ExitCode::NoSuggestion,
            format_raw_output(raw_output.as_deref()),
        ),
        Err(AppError::Config(ce)) => (ExitCode::ConfigError, format!("{ce}\n")),
        Err(AppError::Provider(pe)) => (ExitCode::ProviderError, format!("{pe}\n")),
    }
}

async fn run_inner(cli: Cli) -> Result<String, AppError> {
    let provider_override = cli.provider.as_deref();
    match cli.command {
        Commands::Fix(cmd) => {
            let (config, _) = load_config()?;
            let selected = select_provider(&config, provider_override)?;
            let client = ProviderClient::new(selected.provider.clone())?;
            let cwd = cmd
                .cwd
                .unwrap_or(env::current_dir().map_err(ConfigError::ConfigRead)?);
            let request = FixRequest {
                command: cmd.command,
                stderr: cmd.stderr,
                exit_code: cmd.exit_code,
                cwd: cwd,
                shell: cmd.shell,
            };
            let response = fix_command(&config, &client, &request, cmd.interactive).await?;
            if cmd.json {
                Ok(format!(
                    "{}\n",
                    serde_json::to_string_pretty(&response).expect("serialize fix response")
                ))
            } else if cmd.interactive {
                Ok(fix::render_interactive_response(&response))
            } else {
                Ok(fix::render_fix_response(&response))
            }
        }
        Commands::Init(cmd) => Ok(render_shell_init(cmd.shell, &cmd.alias)),
        Commands::Doctor => {
            let (config, path) = load_config()?;
            let selected = select_provider(&config, provider_override)?;
            let client = ProviderClient::new(selected.provider.clone())?;
            let report = run_doctor(&selected.name, &client, selected.provider, &path).await?;
            Ok(format!("{report}\n"))
        }
        Commands::Config(cmd) => match cmd.command {
            ConfigSubcommand::Show => {
                let (config, path) = load_config()?;
                let view = resolved_config_view(&config, path, provider_override)?;
                Ok(format!(
                    "{}\n",
                    serde_json::to_string_pretty(&view).expect("serialize config")
                ))
            }
            ConfigSubcommand::Init(init) => {
                if init.stdout {
                    Ok(format!("{}\n", default_config_template()))
                } else {
                    let path = init_config(init.force)?;
                    Ok(format!("Wrote config template to {}\n", path.display()))
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::format_raw_output;

    #[test]
    fn format_raw_output_includes_full_model_response() {
        let rendered = format_raw_output(Some("{\"f\":[]}"));
        assert!(rendered.contains("no suggestion returned by model"));
        assert!(rendered.contains("--- raw output ---"));
        assert!(rendered.contains("'{\"f\":[]}'"));
    }

    #[test]
    fn format_raw_output_omits_raw_section_when_missing() {
        let rendered = format_raw_output(None);
        assert_eq!(rendered, "no suggestion returned by model\n");
    }
}
