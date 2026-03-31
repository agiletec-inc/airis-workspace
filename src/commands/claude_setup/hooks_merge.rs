// Hook merge/remove operations for settings.json

use anyhow::{Context, Result};
use serde_json::Value;

use super::entries::{airis_edit_guard_entry, airis_pre_tool_use_entry, airis_stop_entry};

/// Check if a hook entry is airis-managed (command contains "airis/")
pub fn is_airis_entry(entry: &Value) -> bool {
    // Check in hooks array within entry
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        return hooks.iter().any(|hook| {
            hook.get("command")
                .and_then(|c| c.as_str())
                .map(|cmd| cmd.contains("airis/"))
                .unwrap_or(false)
        });
    }
    false
}

/// Check if settings.json has an airis hook in the given event
pub fn has_airis_hook(settings: &Value, event: &str) -> bool {
    settings
        .get("hooks")
        .and_then(|h| h.get(event))
        .and_then(|arr| arr.as_array())
        .map(|entries| entries.iter().any(is_airis_entry))
        .unwrap_or(false)
}

/// Merge airis hooks into settings.json value, preserving existing user hooks
pub fn merge_airis_hooks(settings: &mut Value) -> Result<()> {
    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = serde_json::json!({});
    }

    // Process PreToolUse — Bash guard + Edit/Write guard
    merge_event_hooks_multi(
        settings,
        "PreToolUse",
        vec![airis_pre_tool_use_entry(), airis_edit_guard_entry()],
        true,
    )?;

    // Process Stop
    merge_event_hooks(settings, "Stop", airis_stop_entry(), false)?;

    Ok(())
}

/// Merge multiple airis hook entries into an event array
/// If `prepend` is true, adds airis entries at the beginning; otherwise at the end
pub fn merge_event_hooks_multi(
    settings: &mut Value,
    event: &str,
    airis_entries: Vec<Value>,
    prepend: bool,
) -> Result<()> {
    let hooks = settings["hooks"]
        .as_object_mut()
        .context("hooks is not an object")?;

    let entries = hooks
        .entry(event)
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .context(format!("hooks.{} is not an array", event))?;

    // Remove existing airis entries
    entries.retain(|e| !is_airis_entry(e));

    // Add airis entries
    if prepend {
        for (i, entry) in airis_entries.into_iter().enumerate() {
            entries.insert(i, entry);
        }
    } else {
        for entry in airis_entries {
            entries.push(entry);
        }
    }

    Ok(())
}

/// Merge a single airis hook entry into an event array
/// If `prepend` is true, adds airis entry at the beginning; otherwise at the end
pub fn merge_event_hooks(
    settings: &mut Value,
    event: &str,
    airis_entry: Value,
    prepend: bool,
) -> Result<()> {
    let hooks = settings["hooks"]
        .as_object_mut()
        .context("hooks is not an object")?;

    let entries = hooks
        .entry(event)
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .context(format!("hooks.{} is not an array", event))?;

    // Remove existing airis entries
    entries.retain(|e| !is_airis_entry(e));

    // Add airis entry
    if prepend {
        entries.insert(0, airis_entry);
    } else {
        entries.push(airis_entry);
    }

    Ok(())
}

/// Remove all airis-managed hooks from settings.json
pub fn remove_airis_hooks(settings: &mut Value) {
    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_event, entries) in hooks.iter_mut() {
            if let Some(arr) = entries.as_array_mut() {
                arr.retain(|e| !is_airis_entry(e));
            }
        }
    }
}
