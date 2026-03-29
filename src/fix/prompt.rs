use crate::models::FixRequest;

/// Build the user prompt describing the failed command context.
pub fn build_prompt(request: &FixRequest) -> String {
    let cwd = request.cwd.display();
    let shell = request
        .shell
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let exit_code = request
        .exit_code
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "Failed command context:\ncommand: {}\nstderr: {}\nexit_code: {}\ncwd: {}\nshell: {}\nReturn the best corrected command.",
        request.command, request.stderr, exit_code, cwd, shell
    )
}

/// Build the full system prompt for fix generation.
pub fn build_system_prompt(
    base_prompt: &str,
    max_alternatives: usize,
    language: Option<&str>,
) -> String {
    let max_total_fixes = max_alternatives.saturating_add(1);
    let mut prompt = format!(
        r#"{base_prompt}
        Primary fix first. Alternatives only if needed, max {max_total_fixes} items.
        Keep "r" and "d" brief and only when helpful."#
    );
    if let Some(language) = language.map(str::trim).filter(|value| !value.is_empty()) {
        prompt.push_str(&format!(
            "\nWrite 'r' and every 'd' in {language}. Keep the command strings in 'c' unchanged."
        ));
    }
    prompt
}
