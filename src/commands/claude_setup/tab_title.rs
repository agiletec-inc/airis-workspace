// Terminal tab-title hook handler.
//
// Claude Code hooks run WITHOUT a controlling terminal, so a hook cannot write
// an OSC escape sequence directly. Claude Code provides the `terminalSequence`
// hook-output field: the hook prints JSON on stdout and Claude Code emits the
// (allowlisted) OSC sequence on the hook's behalf. Allowed OSC codes: 0, 1, 2,
// 9, 99, 777, BEL.
//
// This handler is invoked as `airis claude tab-title <state>` from hook entries
// that `airis claude setup` injects into ~/.claude/settings.json.

use std::io::Read;
use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::cli::TabTitleState;
use crate::manifest::GlobalConfig;

/// Emit a terminal-title escape sequence for the given agent state.
pub fn emit(state: TabTitleState) -> Result<()> {
    let config = GlobalConfig::load()?;
    let cfg = &config.claude.terminal_title;

    // Feature disabled: emit nothing so the hook is a harmless no-op.
    if !cfg.enabled {
        return Ok(());
    }

    let emoji = match state {
        TabTitleState::Idle => &cfg.idle,
        TabTitleState::Running => &cfg.running,
        TabTitleState::Waiting => &cfg.waiting,
    };

    let title = build_title(emoji, &repo_name());

    // OSC 0 = set icon name + window/tab title (BEL-terminated).
    let osc = format!("\u{1b}]0;{}\u{7}", title);
    println!("{}", json!({ "terminalSequence": osc }));
    Ok(())
}

/// Build the tab title string: `<emoji> <repo>`, or just `<repo>` when the
/// emoji is empty (the idle state defaults to no emoji).
fn build_title(emoji: &str, repo: &str) -> String {
    if emoji.is_empty() {
        repo.to_string()
    } else {
        format!("{} {}", emoji, repo)
    }
}

/// Resolve the repo name from the hook payload's `cwd` (stdin JSON),
/// falling back to `$PWD`, then a generic label.
fn repo_name() -> String {
    let cwd = read_cwd_from_stdin()
        .or_else(|| std::env::var("PWD").ok())
        .unwrap_or_default();

    Path::new(&cwd)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "claude".to_string())
}

/// Read the hook JSON payload from stdin and extract a non-empty `.cwd`.
fn read_cwd_from_stdin() -> Option<String> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok()?;
    let value: Value = serde_json::from_str(&input).ok()?;
    value
        .get("cwd")
        .and_then(|c| c.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::build_title;

    #[test]
    fn title_with_emoji() {
        assert_eq!(build_title("🏃", "agiletec-inc"), "🏃 agiletec-inc");
        assert_eq!(build_title("✋", "airis-workspace"), "✋ airis-workspace");
    }

    #[test]
    fn title_without_emoji_is_repo_only() {
        assert_eq!(build_title("", "agiletec-inc"), "agiletec-inc");
    }
}
