use dialoguer::console::style;
use dialoguer::{Select, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::io::{IsTerminal, Write};
use std::time::Duration;

use crate::models::{FixResponse, RiskLevel};
use crate::provider::ProgressPhase;

pub(crate) const PROGRESS_MODE_ENV: &str = "SUPERFUCK_PROGRESS_MODE";
pub(crate) const PROGRESS_MODE_PLAIN: &str = "plain";
pub(crate) const CLEAR_LINE_SEQUENCE: &str = "\r\x1b[2K";

pub(crate) fn progress_message(phase: ProgressPhase, streamed_chars: Option<usize>) -> String {
    match phase {
        ProgressPhase::Preparing => "Preparing request...".to_string(),
        ProgressPhase::QueryingModel => "Waiting for model response...".to_string(),
        ProgressPhase::ReceivingResponse => match streamed_chars {
            Some(chars) => format!("Receiving response... {chars} chars"),
            None => "Receiving response...".to_string(),
        },
        ProgressPhase::ParsingResponse => "Parsing model output...".to_string(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProgressMode {
    None,
    Indicatif,
    Plain,
}

impl ProgressMode {
    pub(crate) fn detect(interactive: bool) -> Self {
        if !interactive {
            return Self::None;
        }

        if env::var(PROGRESS_MODE_ENV).ok().as_deref() == Some(PROGRESS_MODE_PLAIN) {
            return Self::Plain;
        }

        if std::io::stderr().is_terminal() {
            Self::Indicatif
        } else {
            Self::Plain
        }
    }
}

pub(crate) enum ProgressRenderer {
    None,
    Indicatif(IndicatifProgressRenderer),
    Plain(PlainProgressRenderer),
}

impl ProgressRenderer {
    pub(crate) fn new(interactive: bool) -> Self {
        match ProgressMode::detect(interactive) {
            ProgressMode::None => Self::None,
            ProgressMode::Indicatif => Self::Indicatif(IndicatifProgressRenderer::new()),
            ProgressMode::Plain => Self::Plain(PlainProgressRenderer::new()),
        }
    }

    pub(crate) fn set_phase(&mut self, phase: ProgressPhase) {
        match self {
            Self::None => {}
            Self::Indicatif(renderer) => renderer.set_phase(phase),
            Self::Plain(renderer) => renderer.set_phase(phase),
        }
    }

    pub(crate) fn set_streamed_chars(&mut self, streamed_chars: usize) {
        match self {
            Self::None => {}
            Self::Indicatif(renderer) => renderer.set_streamed_chars(streamed_chars),
            Self::Plain(renderer) => renderer.set_streamed_chars(streamed_chars),
        }
    }

    pub(crate) fn finish(&mut self) {
        match self {
            Self::None => {}
            Self::Indicatif(renderer) => renderer.finish(),
            Self::Plain(renderer) => renderer.finish(),
        }
    }
}

pub(crate) struct IndicatifProgressRenderer {
    pb: ProgressBar,
}

impl IndicatifProgressRenderer {
    fn new() -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠴⠦⠧⠇⠏")
                .template("{spinner:.cyan} [{elapsed_precise}] {msg}")
                .expect("valid template"),
        );
        pb.enable_steady_tick(Duration::from_millis(100));
        Self { pb }
    }

    fn set_phase(&self, phase: ProgressPhase) {
        self.pb
            .set_message(style(progress_message(phase, None)).bold().to_string());
    }

    fn set_streamed_chars(&self, streamed_chars: usize) {
        self.pb.set_message(
            style(progress_message(
                ProgressPhase::ReceivingResponse,
                Some(streamed_chars),
            ))
            .bold()
            .to_string(),
        );
    }

    fn finish(&self) {
        self.pb.finish_and_clear();
    }
}

pub(crate) struct PlainProgressRenderer {
    current_phase: ProgressPhase,
}

impl PlainProgressRenderer {
    fn new() -> Self {
        Self {
            current_phase: ProgressPhase::Preparing,
        }
    }

    fn set_phase(&mut self, phase: ProgressPhase) {
        self.current_phase = phase;
        let _ = self.write_line(progress_message(phase, None).as_str());
    }

    fn set_streamed_chars(&mut self, streamed_chars: usize) {
        self.current_phase = ProgressPhase::ReceivingResponse;
        let message = progress_message(self.current_phase, Some(streamed_chars));
        let _ = self.write_line(&message);
    }

    fn finish(&mut self) {
        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(CLEAR_LINE_SEQUENCE.as_bytes());
        let _ = stderr.flush();
    }

    fn write_line(&self, message: &str) -> std::io::Result<()> {
        let mut stderr = std::io::stderr();
        stderr.write_all(CLEAR_LINE_SEQUENCE.as_bytes())?;
        stderr.write_all(message.as_bytes())?;
        stderr.flush()
    }
}

/// Render a normalized fix response as plain text for non-interactive output.
pub fn render_fix_response(response: &FixResponse) -> String {
    let mut output = String::new();
    if let Some(reason) = &response.reason {
        output.push_str(&format!("Diagnosis: {reason}\n\n"));
    }

    if matches!(
        response.primary_fix.risk_level,
        RiskLevel::Medium | RiskLevel::High
    ) {
        output.push_str(&format!(
            "Warning: {}-risk suggestion\n",
            response.primary_fix.risk_level
        ));
    }
    output.push_str(&format!("Top fix: {}\n", response.primary_fix.command));
    if let Some(description) = &response.primary_fix.description {
        output.push_str(&format!("Description: {description}\n"));
    }
    if !response.alternatives.is_empty() {
        output.push_str("\nAlternatives:\n");
        for (index, candidate) in response.alternatives.iter().enumerate() {
            output.push_str(&format!(
                "  {}) {} [{}]\n",
                index + 1,
                candidate.command,
                candidate.risk_level
            ));
            if let Some(description) = &candidate.description {
                output.push_str(&format!("     {description}\n"));
            }
        }
    }
    output.push_str(&format!(
        "\nModel: {}\nTiming: first byte {} ms, total {} ms\nTokens: ↑ {}, ↓ {}\n",
        response.model,
        response.time_to_first_byte_ms,
        response.total_latency_ms,
        response.prompt_tokens,
        response.completion_tokens
    ));
    output
}

/// Render a normalized fix response as an interactive command picker.
pub fn render_interactive_response(response: &FixResponse) -> String {
    let mut items = vec![];
    let mut commands = vec![];

    let p_cmd = &response.primary_fix.command;
    items.push(render_choice_item(
        p_cmd,
        response.primary_fix.description.as_deref(),
    ));
    commands.push(p_cmd.clone());

    for alt in &response.alternatives {
        let a_cmd = &alt.command;
        items.push(render_choice_item(a_cmd, alt.description.as_deref()));
        commands.push(a_cmd.clone());
    }

    if let Err(e) = std::io::Write::write_all(
        &mut std::io::stderr(),
        format!(
            "{} {}{}\n",
            style("Analyzed").green().bold(),
            style(format!(
                "(↑ {} tokens, ↓ {} tokens)",
                response.prompt_tokens, response.completion_tokens
            ))
            .dim(),
            if response.total_latency_ms > 0 {
                format!(
                    " {}",
                    style(format!(
                        "first byte {}ms, total {}ms",
                        response.time_to_first_byte_ms, response.total_latency_ms
                    ))
                    .dim()
                )
            } else {
                "".to_string()
            }
        )
        .as_bytes(),
    ) {
        let _ = e;
    }

    if let Some(reason) = &response.reason {
        if let Err(e) = std::io::Write::write_all(
            &mut std::io::stderr(),
            format!("{} {}\n\n", style("Diagnosis:").yellow().bold(), reason).as_bytes(),
        ) {
            let _ = e;
        }
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("superfuck")
        .default(0)
        .items(&items)
        .interact_opt();

    match selection {
        Ok(Some(index)) => commands[index].clone(),
        _ => String::new(),
    }
}

pub(crate) fn render_choice_item(command: &str, description: Option<&str>) -> String {
    match description.filter(|value| !value.trim().is_empty()) {
        Some(description) => format!(
            "{}  {} {}",
            style(command).cyan().bold(),
            style("#").dim(),
            style(description).dim()
        ),
        None => format!("{}", style(command).cyan().bold()),
    }
}
