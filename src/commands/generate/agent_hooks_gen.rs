use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{Value, json};
use std::fs;

use crate::manifest::Manifest;

/// Generate AI agent hooks (.claude/settings.json, etc.)
pub fn generate_agent_hooks(manifest: &Manifest) -> Result<()> {
    if !manifest
        .docs
        .vendors
        .contains(&crate::manifest::DocsVendor::Claude)
    {
        return Ok(());
    }

    println!("   {} Generating AI agent quality gates...", "🤖".cyan());

    // 1. Claude Code
    generate_claude_settings()?;

    Ok(())
}

fn generate_claude_settings() -> Result<()> {
    let home = dirs::home_dir().context("No home directory found")?;
    let settings_path = home.join(".claude/settings.json");

    if !settings_path.exists() {
        // If it doesn't exist, we don't force create it (might not be installed)
        return Ok(());
    }

    let content = fs::read_to_string(&settings_path)?;
    let mut settings: Value = serde_json::from_str(&content)?;

    // Add or update the Stop hook for airis-workspace
    let hooks = settings.get_mut("hooks").and_then(|h| h.as_object_mut());

    if let Some(hooks_obj) = hooks {
        let stop_hooks = hooks_obj.entry("Stop").or_insert_with(|| json!([]));

        if let Some(stop_arr) = stop_hooks.as_array_mut() {
            // Remove existing airis-managed entries
            stop_arr.retain(|h| {
                let cmd = h
                    .get("hooks")
                    .and_then(|h_list| h_list.as_array())
                    .and_then(|list| list.first())
                    .and_then(|first| first.get("command"))
                    .and_then(|c| c.as_str());

                match cmd {
                    Some(c) => !c.contains("airis verify"),
                    None => true,
                }
            });

            // Add the agent-based verification hook
            stop_arr.push(json!({
                "matcher": "",
                "hooks": [{
                    "type": "agent",
                    "prompt": "作業を完了する前に `airis verify` を実行して、型チェック・テスト・静的解析がすべてパスしていることを確認してください。失敗した場合は、その内容を修正して再実行してください。妥協してはいけません。",
                    "timeout": 300
                }]
            }));
        }
    }

    fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings)? + "\n",
    )?;
    println!(
        "     {} Updated ~/.claude/settings.json (Stop hook added)",
        "✓".green()
    );

    Ok(())
}
