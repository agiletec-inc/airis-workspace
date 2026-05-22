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

use std::fs;
use std::io::Read;
use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::cli::TabTitleState;
use crate::manifest::GlobalConfig;

/// Tool calls that mean "Claude is asking the user something and is now
/// blocked on their answer". Mirrors the `PreToolUse` matcher in
/// `settings_hooks::HOOK_EVENTS` so the two stay consistent.
const WAITING_TOOLS: &[&str] = &["AskUserQuestion", "ExitPlanMode"];

/// Emit a terminal-title escape sequence for the given agent state.
pub fn emit(state: TabTitleState) -> Result<()> {
    let config = GlobalConfig::load()?;
    let cfg = &config.claude.terminal_title;

    // Feature disabled: emit nothing so the hook is a harmless no-op.
    if !cfg.enabled {
        return Ok(());
    }

    // The hook JSON payload arrives once on stdin; read it once and reuse it
    // both for the repo name (`cwd`) and the Stop resolution (`transcript_path`).
    let payload = read_hook_payload();

    // `Stop` fires on every turn end — including when Claude ends its turn by
    // asking via AskUserQuestion / ExitPlanMode. A blind `idle` there would
    // overwrite the `waiting` emoji set by the PreToolUse hook, making "asking
    // a question" look identical to "task complete". Resolve it instead by
    // inspecting the transcript.
    let effective = match state {
        TabTitleState::Stop => resolve_stop_state(payload.as_ref()),
        other => other,
    };

    let emoji = match effective {
        TabTitleState::Idle => &cfg.idle,
        TabTitleState::Running => &cfg.running,
        TabTitleState::Waiting => &cfg.waiting,
        // `Stop` is always resolved to a concrete state above; should it ever
        // reach here, fall back to idle (no emoji).
        TabTitleState::Stop => &cfg.idle,
    };

    let title = build_title(emoji, &repo_name(payload.as_ref()));

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

/// Decide what a `Stop` event means: `Waiting` when Claude ended its turn
/// asking the user a question, otherwise `Idle` (task complete).
///
/// The Stop hook payload carries no flag to tell the two apart, so the
/// transcript (`transcript_path`) is the only signal available.
fn resolve_stop_state(payload: Option<&Value>) -> TabTitleState {
    let asking = payload
        .and_then(|p| p.get("transcript_path"))
        .and_then(Value::as_str)
        .map(transcript_ends_with_question)
        .unwrap_or(false);

    if asking {
        TabTitleState::Waiting
    } else {
        TabTitleState::Idle
    }
}

/// True when the transcript's last assistant turn ends with a tool call that
/// blocks on user input. Missing or unreadable transcript → false (idle).
fn transcript_ends_with_question(transcript_path: &str) -> bool {
    match fs::read_to_string(transcript_path) {
        Ok(content) => last_assistant_calls_waiting_tool(&content),
        Err(_) => false,
    }
}

/// Scan a JSONL transcript from the end for the last `assistant` entry and
/// report whether its message content includes an AskUserQuestion /
/// ExitPlanMode `tool_use`.
fn last_assistant_calls_waiting_tool(transcript: &str) -> bool {
    for line in transcript.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        // Found the last assistant entry — its content decides the state.
        return entry
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array)
            .map(|items| items.iter().any(is_waiting_tool_use))
            .unwrap_or(false);
    }
    false
}

/// True for a `tool_use` content item whose tool name blocks on user input.
fn is_waiting_tool_use(item: &Value) -> bool {
    if item.get("type").and_then(Value::as_str) != Some("tool_use") {
        return false;
    }
    item.get("name")
        .and_then(Value::as_str)
        .map(|name| WAITING_TOOLS.contains(&name))
        .unwrap_or(false)
}

/// Resolve the repo name from the hook payload's `cwd`, falling back to
/// `$PWD`, then a generic label.
fn repo_name(payload: Option<&Value>) -> String {
    let cwd = payload
        .and_then(|p| p.get("cwd"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| std::env::var("PWD").ok())
        .unwrap_or_default();

    Path::new(&cwd)
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "claude".to_string())
}

/// Read the hook JSON payload from stdin. Returns `None` when stdin is empty
/// or not valid JSON — e.g. when the command is run manually for testing.
fn read_hook_payload() -> Option<Value> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).ok()?;
    serde_json::from_str(input.trim()).ok()
}

#[cfg(test)]
mod tests {
    use super::{build_title, last_assistant_calls_waiting_tool};

    #[test]
    fn title_with_emoji() {
        assert_eq!(build_title("🏃", "agiletec-inc"), "🏃 agiletec-inc");
        assert_eq!(build_title("✋", "airis-workspace"), "✋ airis-workspace");
    }

    #[test]
    fn title_without_emoji_is_repo_only() {
        assert_eq!(build_title("", "agiletec-inc"), "agiletec-inc");
    }

    #[test]
    fn stop_after_ask_user_question_is_waiting() {
        let transcript = concat!(
            r#"{"type":"user","message":{"role":"user","content":"hi"}}"#,
            "\n",
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"AskUserQuestion","input":{}}]}}"#,
        );
        assert!(last_assistant_calls_waiting_tool(transcript));
    }

    #[test]
    fn stop_after_exit_plan_mode_is_waiting() {
        let transcript = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"ExitPlanMode","input":{}}]}}"#;
        assert!(last_assistant_calls_waiting_tool(transcript));
    }

    #[test]
    fn stop_after_plain_text_response_is_idle() {
        let transcript = concat!(
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash"}]}}"#,
            "\n",
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result"}]}}"#,
            "\n",
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done."}]}}"#,
        );
        assert!(!last_assistant_calls_waiting_tool(transcript));
    }

    #[test]
    fn stop_after_ordinary_tool_use_is_idle() {
        let transcript = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash"}]}}"#;
        assert!(!last_assistant_calls_waiting_tool(transcript));
    }

    #[test]
    fn empty_or_garbage_transcript_is_idle() {
        assert!(!last_assistant_calls_waiting_tool(""));
        assert!(!last_assistant_calls_waiting_tool("not json\n\n"));
    }

    #[test]
    fn trailing_blank_lines_are_skipped() {
        let transcript = concat!(
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"AskUserQuestion"}]}}"#,
            "\n\n  \n",
        );
        assert!(last_assistant_calls_waiting_tool(transcript));
    }
}
