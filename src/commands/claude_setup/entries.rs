// Hook entry definitions for settings.json

use serde_json::Value;

pub fn airis_pre_tool_use_entry() -> Value {
    serde_json::json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": "bash ~/.claude/hooks/airis/docker-first-guard.sh"
        }]
    })
}

pub fn airis_edit_guard_entry() -> Value {
    serde_json::json!({
        "matcher": "Edit|Write",
        "hooks": [{
            "type": "command",
            "command": "bash ~/.claude/hooks/airis/docker-first-edit-guard.sh"
        }]
    })
}

pub fn airis_stop_entry() -> Value {
    serde_json::json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": "bash ~/.claude/hooks/airis/stop-test-check.sh",
            "timeout": 30
        }]
    })
}
