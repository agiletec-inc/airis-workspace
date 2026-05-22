// settings.json hook injection for the terminal tab-title feature.
//
// `airis claude setup` injects (and `airis claude uninstall` removes) hook
// entries that call `airis claude tab-title <state>`. The injection is
// idempotent: every run first strips all airis-managed tab-title entries
// (identified by a marker substring in the hook command), then re-adds the
// current set. Entries that don't carry the marker — including the user's own
// hooks and the `type:"agent"` airis-verify Stop hook — are never touched.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};

use crate::manifest::TerminalTitleSection;

/// Marker substring identifying an airis-managed tab-title hook command.
const MARKER: &str = "claude tab-title";
/// Marker for the legacy hand-written shell-script hook (migrated away).
const LEGACY_SCRIPT_MARKER: &str = "warp-tab-title.sh";

/// Hook events and the agent state each one reports.
/// `matcher` follows Claude Code's hook schema (regex for tool events).
const HOOK_EVENTS: &[(&str, &str, &str)] = &[
    ("SessionStart", "", "idle"),
    ("UserPromptSubmit", "", "running"),
    ("PostToolUse", "", "running"),
    ("PreToolUse", "AskUserQuestion|ExitPlanMode", "waiting"),
    ("Notification", "", "waiting"),
    ("Stop", "", "idle"),
];

/// Outcome of an `apply` call.
pub struct HookSyncResult {
    /// Number of tab-title hook entries injected (0 when disabled).
    pub injected: usize,
    /// True when legacy `warp-tab-title.sh` entries were migrated away.
    pub migrated_legacy: bool,
}

/// Inject or remove airis-managed tab-title hook entries in settings.json.
///
/// When `cfg.enabled` is false, entries are only removed.
pub fn apply(
    claude_home: &Path,
    airis_bin: &str,
    cfg: &TerminalTitleSection,
) -> Result<HookSyncResult> {
    let settings_path = claude_home.join("settings.json");

    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .with_context(|| format!("Failed to read {}", settings_path.display()))?;
        match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "   {} {} is not valid JSON ({}); skipping tab-title hooks",
                    "⚠".yellow(),
                    settings_path.display(),
                    e
                );
                return Ok(HookSyncResult {
                    injected: 0,
                    migrated_legacy: false,
                });
            }
        }
    } else {
        json!({})
    };

    let result = mutate(&mut settings, airis_bin, cfg.enabled);

    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let mut content = serde_json::to_string_pretty(&settings)?;
    content.push('\n');
    fs::write(&settings_path, content)
        .with_context(|| format!("Failed to write {}", settings_path.display()))?;

    Ok(result)
}

/// Pure settings.json mutation: strip managed entries, optionally re-add.
/// Separated from I/O so it can be unit-tested directly.
pub fn mutate(settings: &mut Value, airis_bin: &str, enabled: bool) -> HookSyncResult {
    // Defensive: settings.json is always a JSON object in practice, but never
    // panic on a malformed top-level value.
    if !settings.is_object() {
        *settings = json!({});
    }
    if !settings.get("hooks").map(Value::is_object).unwrap_or(false) {
        settings["hooks"] = json!({});
    }

    let mut migrated_legacy = false;

    // Phase 1: strip all existing tab-title / legacy entries from every event.
    if let Some(hooks) = settings["hooks"].as_object_mut() {
        for entries in hooks.values_mut() {
            if let Some(arr) = entries.as_array_mut() {
                arr.retain(|entry| {
                    let (is_managed, is_legacy) = classify_entry(entry);
                    if is_legacy {
                        migrated_legacy = true;
                    }
                    !is_managed
                });
            }
        }
    }

    // Phase 2: re-add the current entry set (skipped when disabled).
    let mut injected = 0;
    if enabled {
        let hooks = settings["hooks"].as_object_mut().expect("hooks is object");
        for (event, matcher, state) in HOOK_EVENTS {
            // Quote the binary path so an install path with spaces still works.
            let command = format!("\"{}\" claude tab-title {}", airis_bin, state);
            let entry = json!({
                "matcher": matcher,
                "hooks": [{ "type": "command", "command": command }],
            });
            hooks
                .entry((*event).to_string())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .expect("event value is array")
                .push(entry);
            injected += 1;
        }
    }

    HookSyncResult {
        injected,
        migrated_legacy,
    }
}

/// Classify a hook entry. Returns `(is_managed, is_legacy)`:
/// - `is_managed`: an airis tab-title entry (current or legacy) — remove it
///   before re-adding.
/// - `is_legacy`: references the old `warp-tab-title.sh` script.
///
/// Entries without a `hooks[].command` string — e.g. `type:"agent"` hooks —
/// never match, so user hooks and the airis-verify Stop hook are preserved.
fn classify_entry(entry: &Value) -> (bool, bool) {
    let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) else {
        return (false, false);
    };
    let mut managed = false;
    let mut legacy = false;
    for hook in hooks {
        if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
            if cmd.contains(LEGACY_SCRIPT_MARKER) {
                managed = true;
                legacy = true;
            } else if cmd.contains(MARKER) {
                managed = true;
            }
        }
    }
    (managed, legacy)
}

/// Count airis-managed tab-title hook entries currently in settings.json.
/// Used by `status()`.
pub fn count_managed_entries(claude_home: &Path) -> usize {
    let settings_path = claude_home.join("settings.json");
    let Ok(content) = fs::read_to_string(&settings_path) else {
        return 0;
    };
    let Ok(settings) = serde_json::from_str::<Value>(&content) else {
        return 0;
    };
    let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) else {
        return 0;
    };
    hooks
        .values()
        .filter_map(|entries| entries.as_array())
        .flatten()
        .filter(|entry| classify_entry(entry).0)
        .count()
}
