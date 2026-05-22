// Integration test for `templates/ui/airis-tab-title.sh` — the actual hook
// script that `airis ui install` ships into ~/.claude/hooks/.
//
// The script is what runs on every Claude Code hook event, so it is tested
// here end-to-end (real `sh`, real `jq`) rather than through the Rust wiring.

use std::io::Write;
use std::process::{Command, Stdio};

fn script() -> String {
    format!(
        "{}/templates/ui/airis-tab-title.sh",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Run the hook script with `state` and a stdin `payload`. Emoji args are the
/// markers `R` (running) / `W` (waiting) / `` (idle) so assertions stay simple.
/// Returns the tab title — the text inside the emitted OSC 0 sequence — or an
/// empty string when the script produced no output.
fn run(state: &str, payload: &str) -> String {
    let mut child = Command::new("sh")
        .arg(script())
        .args([state, "R", "W", ""])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn airis-tab-title.sh");
    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(payload.as_bytes()).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "script must exit 0");
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return String::new();
    }
    let v: serde_json::Value = serde_json::from_str(stdout).expect("hook output must be JSON");
    let seq = v["terminalSequence"]
        .as_str()
        .expect("terminalSequence field");
    // OSC 0 sequence: ESC ] 0 ; <title> BEL
    seq.split_once("]0;")
        .and_then(|(_, rest)| rest.split('\u{7}').next())
        .unwrap_or("")
        .to_string()
}

/// Write a transcript fixture and return a stop-event payload pointing at it.
fn stop_payload(dir: &std::path::Path, last_line: &str) -> String {
    let tp = dir.join("transcript.jsonl");
    std::fs::write(&tp, format!("{last_line}\n")).unwrap();
    format!(
        r#"{{"cwd":"/x/myrepo","transcript_path":"{}"}}"#,
        tp.display()
    )
}

#[test]
fn idle_has_no_emoji() {
    assert_eq!(run("idle", r#"{"cwd":"/x/myrepo"}"#), "myrepo");
}

#[test]
fn running_shows_running_emoji() {
    assert_eq!(run("running", r#"{"cwd":"/x/myrepo"}"#), "R myrepo");
}

#[test]
fn waiting_shows_waiting_emoji() {
    assert_eq!(run("waiting", r#"{"cwd":"/x/myrepo"}"#), "W myrepo");
}

#[test]
fn stop_after_ask_user_question_is_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let payload = stop_payload(
        dir.path(),
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"AskUserQuestion"}]}}"#,
    );
    assert_eq!(run("stop", &payload), "W myrepo");
}

#[test]
fn stop_after_exit_plan_mode_is_waiting() {
    let dir = tempfile::tempdir().unwrap();
    let payload = stop_payload(
        dir.path(),
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"ExitPlanMode"}]}}"#,
    );
    assert_eq!(run("stop", &payload), "W myrepo");
}

#[test]
fn stop_after_completion_is_idle() {
    let dir = tempfile::tempdir().unwrap();
    let payload = stop_payload(
        dir.path(),
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Done."}]}}"#,
    );
    assert_eq!(run("stop", &payload), "myrepo");
}

#[test]
fn stop_after_ordinary_tool_use_is_idle() {
    let dir = tempfile::tempdir().unwrap();
    let payload = stop_payload(
        dir.path(),
        r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash"}]}}"#,
    );
    assert_eq!(run("stop", &payload), "myrepo");
}

#[test]
fn stop_with_missing_transcript_is_idle() {
    assert_eq!(
        run(
            "stop",
            r#"{"cwd":"/x/myrepo","transcript_path":"/nonexistent/x.jsonl"}"#,
        ),
        "myrepo"
    );
}

#[test]
fn unknown_state_emits_nothing() {
    assert_eq!(run("bogus", r#"{"cwd":"/x/myrepo"}"#), "");
}

#[test]
fn garbage_stdin_falls_back_gracefully() {
    // Bad JSON on stdin must not crash the script; idle just yields the repo
    // name from $PWD (whatever the test runner's cwd is) with no emoji.
    let title = run("idle", "not json at all");
    assert!(!title.is_empty(), "still emits a title");
    assert!(!title.starts_with("R ") && !title.starts_with("W "));
}
