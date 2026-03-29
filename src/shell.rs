use crate::cli::Shell;

/// Render the shell integration snippet for the requested shell and alias.
pub fn render_shell_init(shell: Shell, alias: &str) -> String {
    match shell {
        Shell::Zsh => zsh_init(alias),
        Shell::Bash => bash_init(alias),
    }
}

fn zsh_init(alias: &str) -> String {
    format!(
        r#"{alias}() {{
  local last_status=$?
  local provider_args=()
  if [ $last_status -eq 0 ]; then
    echo "{alias}: previous command succeeded"
    return 0
  fi
  if [ $# -gt 1 ]; then
    echo "usage: {alias} [model]"
    return 2
  fi
  if [ $# -eq 1 ]; then
    provider_args=(--provider "$1")
  fi

  local last_command
  last_command=$(fc -ln -1 | sed 's/^[[:space:]]*//')
  local stderr_text="${{SUPERFUCK_STDERR:-command failed}}"
  
  local choice
  choice=$(SUPERFUCK_PROGRESS_MODE=plain command superfuck "${{provider_args[@]}}" fix --command "$last_command" --stderr "$stderr_text" --exit-code "$last_status" --cwd "$PWD" --shell zsh --interactive) || return $?
  
  if [ -n "$choice" ]; then
    print -s "$choice"
    eval "$choice"
  fi
}}
"#
    )
}

fn bash_init(alias: &str) -> String {
    format!(
        r#"{alias}() {{
  local last_status=$?
  local provider_args=()
  if [ $last_status -eq 0 ]; then
    echo "{alias}: previous command succeeded"
    return 0
  fi
  if [ $# -gt 1 ]; then
    echo "usage: {alias} [model]"
    return 2
  fi
  if [ $# -eq 1 ]; then
    provider_args=(--provider "$1")
  fi

  local last_command
  last_command=$(history 1 | sed 's/^[[:space:]]*[0-9]\+[[:space:]]*//')
  local stderr_text="${{SUPERFUCK_STDERR:-command failed}}"
  
  local choice
  choice=$(SUPERFUCK_PROGRESS_MODE=plain command superfuck "${{provider_args[@]}}" fix --command "$last_command" --stderr "$stderr_text" --exit-code "$last_status" --cwd "$PWD" --shell bash --interactive) || return $?

  if [ -n "$choice" ]; then
    history -s "$choice"
    eval "$choice"
  fi
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zsh_init_exposes_fuck_function() {
        let snippet = render_shell_init(Shell::Zsh, "fuck");
        assert!(snippet.contains("fuck() {"));
        assert!(snippet.contains("SUPERFUCK_PROGRESS_MODE=plain command superfuck \"${provider_args[@]}\" fix"));
        assert!(snippet.contains("provider_args=(--provider \"$1\")"));
        assert!(snippet.contains("usage: fuck [model]"));
        assert!(snippet.contains("print -s \"$choice\""));
        assert!(snippet.contains("eval \"$choice\""));
        assert!(!snippet.contains("zle -N fuck"));
    }

    #[test]
    fn bash_init_exposes_fuck_function() {
        let snippet = render_shell_init(Shell::Bash, "fuck");
        assert!(snippet.contains("fuck() {"));
        assert!(snippet.contains("SUPERFUCK_PROGRESS_MODE=plain command superfuck \"${provider_args[@]}\" fix"));
        assert!(snippet.contains("provider_args=(--provider \"$1\")"));
        assert!(snippet.contains("usage: fuck [model]"));
        assert!(snippet.contains("history -s \"$choice\""));
        assert!(snippet.contains("eval \"$choice\""));
    }

    #[test]
    fn zsh_init_supports_custom_alias() {
        let snippet = render_shell_init(Shell::Zsh, "wtf");
        assert!(snippet.contains("wtf() {"));
        assert!(snippet.contains("usage: wtf [model]"));
        assert!(snippet.contains("wtf: previous command succeeded"));
        assert!(!snippet.contains("fuck() {"));
    }

    #[test]
    fn bash_init_supports_custom_alias() {
        let snippet = render_shell_init(Shell::Bash, "wtf");
        assert!(snippet.contains("wtf() {"));
        assert!(snippet.contains("usage: wtf [model]"));
        assert!(snippet.contains("wtf: previous command succeeded"));
        assert!(!snippet.contains("fuck() {"));
    }
}
