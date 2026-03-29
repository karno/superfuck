mod normalize;
mod parse;
mod prompt;
mod render;

use crate::config::Config;
use crate::error::AppError;
use crate::models::{FixRequest, FixResponse};
use crate::provider::{ModelClient, ModelProgressEvent, ProgressPhase};
use normalize::build_fix_response;
use render::ProgressRenderer;

pub use normalize::normalize_candidate;
pub use parse::{ModelFix, ModelOutput, parse_model_output};
pub use prompt::{build_prompt, build_system_prompt};
pub use render::{render_fix_response, render_interactive_response};

/// Run the fix generation pipeline for a failed command.
pub async fn fix_command<C: ModelClient + ?Sized>(
    config: &Config,
    client: &C,
    request: &FixRequest,
    interactive: bool,
) -> Result<FixResponse, AppError> {
    let prompt = build_prompt(request);
    let system_prompt = build_system_prompt(
        &config.system_prompt,
        config.max_alternatives,
        config.language.as_deref(),
    );
    let mut progress = ProgressRenderer::new(interactive);
    progress.set_phase(ProgressPhase::Preparing);
    progress.set_phase(ProgressPhase::QueryingModel);
    let mut streamed_chars = 0usize;
    let mut on_progress = |event| match event {
        ModelProgressEvent::FirstChunk => {
            progress.set_phase(ProgressPhase::ReceivingResponse);
        }
        ModelProgressEvent::ContentDelta(delta) => {
            streamed_chars += delta.chars().count();
            progress.set_streamed_chars(streamed_chars);
        }
    };
    let raw_response = match client
        .complete_with_progress(&prompt, &system_prompt, &mut on_progress)
        .await
    {
        Ok(response) => response,
        Err(err) => {
            progress.finish();
            return Err(err);
        }
    };

    progress.set_phase(ProgressPhase::ParsingResponse);
    progress.finish();
    let parsed = parse_model_output(&raw_response.content)?;

    build_fix_response(parsed, raw_response.clone(), config).map_err(|err| match err {
        AppError::NoSuggestion { .. } => AppError::NoSuggestion {
            raw_output: Some(raw_response.content),
        },
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Shell;
    use crate::config::{Config, ProviderBackend, ProviderConfig};
    use crate::constants::{
        DEFAULT_MAX_ALTERNATIVES, SYSTEM_PROMPT, default_high_risk_patterns,
        default_medium_risk_patterns,
    };
    use crate::models::{FixCandidate, RiskLevel};
    use crate::provider::ModelResponse;
    use async_trait::async_trait;
    use indexmap::IndexMap;
    use normalize::normalize_optional_text;
    use render::{CLEAR_LINE_SEQUENCE, ProgressMode, progress_message, render_choice_item};
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug)]
    struct MockClient {
        response: ModelResponse,
        seen_prompt: Mutex<Option<String>>,
        seen_system_prompt: Mutex<Option<String>>,
    }

    impl MockClient {
        fn new(response: ModelResponse) -> Self {
            Self {
                response,
                seen_prompt: Mutex::new(None),
                seen_system_prompt: Mutex::new(None),
            }
        }
    }

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock")
    }

    #[async_trait(?Send)]
    impl ModelClient for MockClient {
        async fn complete_with_progress(
            &self,
            prompt: &str,
            system_prompt: &str,
            _on_progress: &mut dyn FnMut(ModelProgressEvent),
        ) -> Result<ModelResponse, AppError> {
            *self.seen_prompt.lock().expect("prompt lock") = Some(prompt.to_string());
            *self.seen_system_prompt.lock().expect("system prompt lock") =
                Some(system_prompt.to_string());
            Ok(self.response.clone())
        }

        async fn healthcheck(&self) -> Result<ModelResponse, AppError> {
            Ok(self.response.clone())
        }
    }

    fn test_config() -> Config {
        Config {
            default_provider: "test".to_string(),
            providers: IndexMap::from([(
                "test".to_string(),
                ProviderConfig {
                    backend: ProviderBackend::OpenaiCompatible,
                    model: "test-model".to_string(),
                    api_key_env: None,
                    timeout_secs: 5,
                    base_url: Some("http://127.0.0.1:4000/v1".to_string()),
                },
            )]),
            max_alternatives: DEFAULT_MAX_ALTERNATIVES,
            language: None,
            system_prompt: SYSTEM_PROMPT.to_string(),
            high_risk_patterns: default_high_risk_patterns(),
            medium_risk_patterns: default_medium_risk_patterns(),
        }
    }

    #[test]
    fn prompt_contains_expected_fields() {
        let request = FixRequest {
            command: "gti status".to_string(),
            stderr: "command not found: gti".to_string(),
            exit_code: Some(127),
            cwd: PathBuf::from("/tmp/project"),
            shell: Some(Shell::Zsh),
        };
        let prompt = build_prompt(&request);
        assert!(prompt.contains("command: gti status"));
        assert!(prompt.contains("stderr: command not found: gti"));
        assert!(prompt.contains("exit_code: 127"));
        assert!(prompt.contains("cwd: /tmp/project"));
        assert!(prompt.contains("shell: zsh"));
    }

    #[test]
    fn progress_message_formats_streamed_chars() {
        assert_eq!(
            progress_message(ProgressPhase::ReceivingResponse, None),
            "Receiving response..."
        );
        assert_eq!(
            progress_message(ProgressPhase::ReceivingResponse, Some(42)),
            "Receiving response... 42 chars"
        );
    }

    #[test]
    fn plain_progress_uses_full_line_clear_sequence() {
        assert_eq!(CLEAR_LINE_SEQUENCE, "\r\x1b[2K");
    }

    #[test]
    fn progress_mode_prefers_plain_hint() {
        let _guard = env_lock();
        unsafe {
            std::env::set_var(render::PROGRESS_MODE_ENV, render::PROGRESS_MODE_PLAIN);
        }
        assert_eq!(ProgressMode::detect(true), ProgressMode::Plain);
        unsafe {
            std::env::remove_var(render::PROGRESS_MODE_ENV);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fix_command_uses_injected_client() {
        let config = test_config();
        let request = FixRequest {
            command: "gti status".to_string(),
            stderr: "command not found: gti".to_string(),
            exit_code: Some(127),
            cwd: PathBuf::from("/tmp/project"),
            shell: Some(Shell::Zsh),
        };
        let client = MockClient::new(ModelResponse {
            content: r#"{"r":"Typo in command","f":[{"c":"git status","d":"Status"}]}"#.to_string(),
            model: "test-model".to_string(),
            time_to_first_byte_ms: 5,
            total_latency_ms: 12,
            prompt_tokens: 10,
            completion_tokens: 20,
        });

        let response = fix_command(&config, &client, &request, false)
            .await
            .expect("fix response");

        assert_eq!(response.primary_fix.command, "git status");
        assert_eq!(response.primary_fix.description.as_deref(), Some("Status"));
        assert_eq!(response.reason.as_deref(), Some("Typo in command"));
        assert!(
            client
                .seen_prompt
                .lock()
                .expect("prompt lock")
                .as_deref()
                .expect("prompt seen")
                .contains("gti status")
        );
        assert!(
            client
                .seen_system_prompt
                .lock()
                .expect("system prompt lock")
                .as_deref()
                .expect("system prompt seen")
                .contains("Primary fix first.")
        );
        assert!(
            client
                .seen_system_prompt
                .lock()
                .expect("system prompt lock")
                .as_deref()
                .expect("system prompt seen")
                .contains("max 4 items.")
        );
    }

    #[test]
    fn system_prompt_injects_total_fix_limit() {
        let prompt = build_system_prompt("base prompt", 0, None);
        assert!(prompt.contains("base prompt"));
        assert!(prompt.contains("max 1 items."));
        assert!(prompt.contains("Keep \"r\" and \"d\" brief and only when helpful."));

        let prompt = build_system_prompt("base prompt", 3, None);
        assert!(prompt.contains("max 4 items."));
    }

    #[test]
    fn system_prompt_keeps_reason_and_description_brief() {
        let prompt = build_system_prompt("base prompt", 1, None);
        assert!(prompt.contains("Keep \"r\" and \"d\" brief and only when helpful."));
    }

    #[test]
    fn system_prompt_injects_language_when_configured() {
        let prompt = build_system_prompt("base prompt", 1, Some("Japanese"));
        assert!(prompt.contains("Write 'r' and every 'd' in Japanese."));
        assert!(prompt.contains("Keep the command strings in 'c' unchanged."));
    }

    #[test]
    fn parser_rejects_malformed_json() {
        let error = parse_model_output("not-json").expect_err("should fail");
        assert!(matches!(error, crate::error::ProviderError::InvalidResponse(_)));
        assert!(error.to_string().contains("--- raw output ---"));
        assert!(error.to_string().contains("'not-json'"));
    }

    #[test]
    fn parser_extracts_json_from_markdown() {
        let raw = "```json\n{\"r\":\"Typo\",\"f\":[{\"c\":\"ls\",\"d\":\"list files\"}]}\n```";
        let parsed = parse_model_output(raw).expect("should parse");
        assert_eq!(parsed.reason.as_deref(), Some("Typo"));
        assert_eq!(parsed.fixes[0].command, "ls");
    }

    #[test]
    fn risk_classifier_marks_patterns() {
        assert_eq!(
            normalize::classify_risk(
                "rm -rf build",
                &default_high_risk_patterns(),
                &default_medium_risk_patterns(),
            ),
            RiskLevel::High
        );
        assert_eq!(
            normalize::classify_risk(
                "sudo chmod 777 script.sh",
                &default_high_risk_patterns(),
                &default_medium_risk_patterns(),
            ),
            RiskLevel::Medium
        );
        assert_eq!(
            normalize::classify_risk(
                "git status",
                &default_high_risk_patterns(),
                &default_medium_risk_patterns(),
            ),
            RiskLevel::Low
        );
    }

    #[test]
    fn renderer_handles_alternatives() {
        let response = FixResponse {
            schema_version: 2,
            reason: Some("You misspelled the command.".to_string()),
            primary_fix: FixCandidate {
                command: "git status".to_string(),
                description: Some("Shows working tree status".to_string()),
                risk_level: RiskLevel::Low,
            },
            alternatives: vec![FixCandidate {
                command: "git stash".to_string(),
                description: Some("Stashes your changes".to_string()),
                risk_level: RiskLevel::Low,
            }],
            model: "test-model".to_string(),
            time_to_first_byte_ms: 5,
            total_latency_ms: 12,
            prompt_tokens: 50,
            completion_tokens: 100,
        };
        let output = render_fix_response(&response);
        assert!(output.contains("Diagnosis: You misspelled the command."));
        assert!(output.contains("Top fix: git status"));
        assert!(output.contains("Description: Shows working tree status"));
        assert!(output.contains("Alternatives:"));
        assert!(output.contains("  1) git stash [low]"));
        assert!(output.contains("     Stashes your changes"));
        assert!(output.contains("Timing: first byte 5 ms, total 12 ms"));
    }

    #[test]
    fn parser_accepts_legacy_schema() {
        let parsed = parse_model_output(
            r#"{"reason":"Typo in command","fixes":[{"command":"git status","description":"Status"}]}"#,
        )
        .expect("parsed legacy response");
        assert_eq!(parsed.reason.as_deref(), Some("Typo in command"));
        assert_eq!(parsed.fixes[0].command, "git status");
        assert_eq!(parsed.fixes[0].description.as_deref(), Some("Status"));
    }

    #[test]
    fn parser_accepts_missing_reason_and_description() {
        let parsed =
            parse_model_output(r#"{"f":[{"c":"git status"},{"c":"git stash list","d":""}]}"#)
                .expect("parsed sparse response");
        assert_eq!(parsed.reason, None);
        assert_eq!(parsed.fixes[0].description, None);
        assert_eq!(parsed.fixes[1].description.as_deref(), Some(""));
    }

    #[test]
    fn model_output_normalizes_into_candidates() {
        let parsed = parse_model_output(
            r#"{"r":"Typo in command","f":[{"c":"git status","d":"Status"},{"c":"git stash list","d":"Stash"}]}"#,
        )
        .expect("parsed response");
        let config = test_config();
        let response = build_fix_response(
            parsed,
            ModelResponse {
                content: String::new(),
                model: "test-model".to_string(),
                time_to_first_byte_ms: 5,
                total_latency_ms: 12,
                prompt_tokens: 10,
                completion_tokens: 20,
            },
            &config,
        )
        .expect("normalized response");

        assert_eq!(response.primary_fix.command, "git status");
        assert_eq!(response.primary_fix.description.as_deref(), Some("Status"));
        assert_eq!(response.alternatives.len(), 1);
        assert_eq!(response.alternatives[0].command, "git stash list");
        assert_eq!(response.alternatives[0].description.as_deref(), Some("Stash"));
    }

    #[test]
    fn renderer_omits_missing_reason_and_description() {
        let response = FixResponse {
            schema_version: 2,
            reason: None,
            primary_fix: FixCandidate {
                command: "git status".to_string(),
                description: None,
                risk_level: RiskLevel::Low,
            },
            alternatives: vec![FixCandidate {
                command: "git stash".to_string(),
                description: None,
                risk_level: RiskLevel::Low,
            }],
            model: "test-model".to_string(),
            time_to_first_byte_ms: 5,
            total_latency_ms: 12,
            prompt_tokens: 50,
            completion_tokens: 100,
        };

        let output = render_fix_response(&response);
        assert!(!output.contains("Diagnosis:"));
        assert!(!output.contains("Description:"));
        assert!(!output.contains("     "));

        let item = render_choice_item("git status", None);
        assert!(item.contains("git status"));
        assert!(!item.contains("#"));
    }

    #[test]
    fn normalize_optional_text_trims_and_drops_empty_values() {
        assert_eq!(normalize_optional_text(None), None);
        assert_eq!(normalize_optional_text(Some("".to_string())), None);
        assert_eq!(normalize_optional_text(Some("   ".to_string())), None);
        assert_eq!(
            normalize_optional_text(Some("  useful  ".to_string())).as_deref(),
            Some("useful")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fix_command_attaches_raw_output_to_no_suggestion() {
        let config = test_config();
        let request = FixRequest {
            command: "gti status".to_string(),
            stderr: "command failed".to_string(),
            exit_code: Some(1),
            cwd: PathBuf::from("/tmp/project"),
            shell: Some(Shell::Zsh),
        };
        let client = MockClient::new(ModelResponse {
            content: r#"{"r":"unknown","f":[]}"#.to_string(),
            model: "test-model".to_string(),
            time_to_first_byte_ms: 5,
            total_latency_ms: 12,
            prompt_tokens: 10,
            completion_tokens: 20,
        });

        let error = fix_command(&config, &client, &request, false)
            .await
            .expect_err("no suggestion expected");

        match error {
            AppError::NoSuggestion { raw_output } => {
                assert_eq!(raw_output.as_deref(), Some(r#"{"r":"unknown","f":[]}"#));
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
