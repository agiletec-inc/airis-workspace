use std::path::Path;

use serde_json::json;

use super::{
    count_tab_title_entries, strip_tab_title_entries, unwire_statusline, wire_statusline,
    wire_tab_title,
};
use crate::manifest::TerminalTitleSection;

fn home() -> &'static Path {
    Path::new("/home/u/.claude")
}

#[test]
fn wire_tab_title_injects_all_events() {
    let mut settings = json!({});
    let n = wire_tab_title(&mut settings, home(), &TerminalTitleSection::default());
    assert_eq!(n, 7);
    assert_eq!(count_tab_title_entries(&settings), 7);
    // Notification carries two matchers (permission/elicitation + idle).
    assert_eq!(
        settings["hooks"]["Notification"].as_array().unwrap().len(),
        2
    );
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "PostToolUse",
        "PreToolUse",
        "Stop",
    ] {
        assert_eq!(
            settings["hooks"][event].as_array().unwrap().len(),
            1,
            "{event}"
        );
    }
}

#[test]
fn wire_tab_title_command_calls_script_not_binary() {
    let mut settings = json!({});
    wire_tab_title(&mut settings, home(), &TerminalTitleSection::default());
    let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(cmd.contains("airis-tab-title.sh"));
    assert!(cmd.contains(" stop "));
    // Must not invoke the airis binary — that is the version-skew bug.
    assert!(!cmd.contains("claude tab-title"));
}

#[test]
fn wire_tab_title_is_idempotent() {
    let mut settings = json!({});
    let cfg = TerminalTitleSection::default();
    wire_tab_title(&mut settings, home(), &cfg);
    wire_tab_title(&mut settings, home(), &cfg);
    assert_eq!(count_tab_title_entries(&settings), 7, "no duplicates");
}

#[test]
fn wire_tab_title_disabled_adds_nothing() {
    let mut settings = json!({});
    let cfg = TerminalTitleSection {
        enabled: false,
        ..TerminalTitleSection::default()
    };
    let n = wire_tab_title(&mut settings, home(), &cfg);
    assert_eq!(n, 0);
    assert_eq!(count_tab_title_entries(&settings), 0);
}

#[test]
fn strip_preserves_user_and_agent_hooks() {
    let mut settings = json!({
        "hooks": {
            "PreToolUse": [
                { "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-hook.sh" }] }
            ],
            "Stop": [
                { "matcher": "", "hooks": [{ "type": "agent", "prompt": "verify", "timeout": 300 }] }
            ]
        }
    });
    wire_tab_title(&mut settings, home(), &TerminalTitleSection::default());
    // user hook + injected waiting hook
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    // agent hook + injected stop hook
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 2);

    strip_tab_title_entries(&mut settings);
    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 1);
    assert_eq!(pre[0]["hooks"][0]["command"], "my-hook.sh");
    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 1);
    assert_eq!(stop[0]["hooks"][0]["type"], "agent");
}

#[test]
fn strip_removes_legacy_implementations() {
    let mut settings = json!({
        "hooks": {
            "Stop": [
                { "matcher": "", "hooks": [{ "type": "command", "command": "\"/bin/airis\" claude tab-title idle" }] },
                { "matcher": "", "hooks": [{ "type": "command", "command": "sh ~/.claude/hooks/warp-tab-title.sh idle" }] }
            ]
        }
    });
    strip_tab_title_entries(&mut settings);
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 0);
}

#[test]
fn statusline_wire_and_unwire() {
    let mut settings = json!({});
    wire_statusline(&mut settings, home());
    let cmd = settings["statusLine"]["command"].as_str().unwrap();
    assert!(cmd.contains("statusline-command.sh"));

    unwire_statusline(&mut settings);
    assert!(settings.get("statusLine").is_none());
}

#[test]
fn unwire_statusline_keeps_user_statusline() {
    let mut settings = json!({
        "statusLine": { "type": "command", "command": "my-own-statusline" }
    });
    unwire_statusline(&mut settings);
    assert!(
        settings.get("statusLine").is_some(),
        "non-airis statusLine kept"
    );
}
