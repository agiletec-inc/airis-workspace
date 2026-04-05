pub mod dir_sync;
pub mod templates;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::Value;

/// Plugin ID in installed_plugins.json
const AIRIS_PLUGIN_ID: &str = "airis-mcp-gateway@airis-mcp-gateway";

/// Legacy hooks directory (to clean up during migration)
const LEGACY_HOOKS_DIR: &str = "hooks/airis";

/// Get the ~/.claude/ directory path
fn claude_home() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".claude"))
}

fn print_status(label: &str, ok: bool) {
    if ok {
        println!("  {} {}", "✓".green(), label);
    } else {
        println!("  {} {}", "✗".red(), label);
    }
}

/// Check if the airis-mcp-gateway plugin is installed
fn check_plugin_installed(claude_home: &std::path::Path) -> Result<bool> {
    let plugins_path = claude_home.join("plugins/installed_plugins.json");
    if !plugins_path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(&plugins_path)
        .with_context(|| format!("Failed to read {}", plugins_path.display()))?;
    let value: Value = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", plugins_path.display()))?;
    Ok(value
        .get("plugins")
        .and_then(|p| p.get(AIRIS_PLUGIN_ID))
        .is_some())
}

/// Remove legacy hook files and settings.json entries from before plugin migration
fn clean_legacy_hooks(claude_home: &std::path::Path) -> Result<usize> {
    let mut cleaned = 0;

    // Remove legacy hooks directory (~/.claude/hooks/airis/)
    let legacy_dir = claude_home.join(LEGACY_HOOKS_DIR);
    if legacy_dir.exists() {
        fs::remove_dir_all(&legacy_dir)
            .with_context(|| format!("Failed to remove {}", legacy_dir.display()))?;
        println!(
            "   {} {} (legacy)",
            "✓".green(),
            legacy_dir.display().to_string().dimmed()
        );
        cleaned += 1;
    }

    // Remove legacy commands
    let legacy_cmd = claude_home.join("commands/playwright-cli.md");
    if legacy_cmd.exists() {
        fs::remove_file(&legacy_cmd)
            .with_context(|| format!("Failed to remove {}", legacy_cmd.display()))?;
        println!(
            "   {} {} (legacy)",
            "✓".green(),
            legacy_cmd.display().to_string().dimmed()
        );
        cleaned += 1;
    }

    // Remove legacy airis entries from settings.json
    let settings_path = claude_home.join("settings.json");
    if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        if let Ok(mut value) = serde_json::from_str::<Value>(&content) {
            let had_entries = remove_legacy_airis_entries(&mut value);
            if had_entries {
                let pretty = serde_json::to_string_pretty(&value)?;
                fs::write(&settings_path, pretty)?;
                println!(
                    "   {} {} (legacy hooks removed)",
                    "✓".green(),
                    settings_path.display().to_string().dimmed()
                );
                cleaned += 1;
            }
        }
    }

    Ok(cleaned)
}

/// Remove airis-managed hook entries from settings.json
/// Returns true if any entries were removed
fn remove_legacy_airis_entries(settings: &mut Value) -> bool {
    let mut removed = false;
    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for (_event, entries) in hooks.iter_mut() {
            if let Some(arr) = entries.as_array_mut() {
                let before = arr.len();
                arr.retain(|entry| !is_legacy_airis_entry(entry));
                if arr.len() < before {
                    removed = true;
                }
            }
        }
    }
    removed
}

/// Check if a hook entry is a legacy airis-managed entry (references ~/.claude/hooks/airis/)
fn is_legacy_airis_entry(entry: &Value) -> bool {
    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
        return hooks.iter().any(|hook| {
            hook.get("command")
                .and_then(|c| c.as_str())
                .is_some_and(|cmd| {
                    cmd.contains("~/.claude/hooks/airis/")
                        || cmd.contains(".claude/hooks/airis/")
                })
        });
    }
    false
}

// ── Public commands ─────────────────────────────────────────────────

/// Install/sync global Claude Code configuration to ~/.claude/
pub fn setup_global() -> Result<()> {
    println!(
        "{}",
        "🛡️  Syncing global Claude Code configuration...".bright_blue()
    );
    println!();

    let home = claude_home()?;

    // 1. Check plugin status
    let plugin_ok = check_plugin_installed(&home)?;
    if plugin_ok {
        println!(
            "   {} airis-mcp-gateway plugin installed",
            "✓".green()
        );
    } else {
        println!(
            "   {} airis-mcp-gateway plugin not found",
            "⚠".yellow()
        );
        println!(
            "     Run: {}",
            "claude plugin install airis-mcp-gateway".bright_cyan()
        );
    }
    println!();

    // 2. Sync CLAUDE.md
    println!("  {}:", "CLAUDE.md".bold());
    let claude_md = templates::global_claude_md();
    let updated = dir_sync::sync_single_file(&home, &claude_md)?;
    if updated {
        println!(
            "   {} {}",
            "✓".green(),
            home.join("CLAUDE.md").display().to_string().dimmed()
        );
    } else {
        println!(
            "   {} {} (unchanged)",
            "–".dimmed(),
            home.join("CLAUDE.md").display().to_string().dimmed()
        );
    }
    println!();

    // 3. Sync managed directories (rules/)
    for managed in templates::managed_dirs() {
        println!("  {}:", managed.rel_dir.bold());
        let result = dir_sync::sync_managed_dir(&home, &managed)?;

        for path in &result.written {
            println!("   {} {}", "✓".green(), path.display().to_string().dimmed());
        }
        for path in &result.deleted {
            println!(
                "   {} {} (orphan removed)",
                "✓".green(),
                path.display().to_string().dimmed()
            );
        }
        for path in &result.unchanged {
            println!(
                "   {} {} (unchanged)",
                "–".dimmed(),
                path.display().to_string().dimmed()
            );
        }
        println!();
    }

    // 4. Clean legacy hooks (transition period)
    let legacy_count = clean_legacy_hooks(&home)?;
    if legacy_count > 0 {
        println!();
        println!(
            "  {} Cleaned {} legacy file(s)",
            "🧹".dimmed(),
            legacy_count
        );
    }

    println!("{}", "✅ Global configuration synced".green());

    Ok(())
}

/// Show current setup status
pub fn status() -> Result<()> {
    println!("{}", "Claude Code Configuration Status".bright_blue().bold());
    println!();

    let home = claude_home()?;

    // Plugin status
    let plugin_ok = check_plugin_installed(&home)?;
    println!("  Plugin:");
    print_status(
        &format!(
            "  {} (hooks, skills, permissions)",
            AIRIS_PLUGIN_ID
        ),
        plugin_ok,
    );

    // CLAUDE.md status
    let claude_md = templates::global_claude_md();
    let claude_md_path = home.join(claude_md.rel_path);
    let claude_md_ok = claude_md_path.exists();
    let claude_md_current = claude_md_ok
        && fs::read_to_string(&claude_md_path)
            .map(|c| c == claude_md.content)
            .unwrap_or(false);

    println!();
    println!("  Global config:");
    if claude_md_current {
        print_status("  CLAUDE.md (current)", true);
    } else if claude_md_ok {
        print_status("  CLAUDE.md (outdated)", false);
    } else {
        print_status("  CLAUDE.md (missing)", false);
    }

    // Rules status
    for managed in templates::managed_dirs() {
        let target_dir = home.join(managed.rel_dir);
        for file in managed.files {
            let file_path = target_dir.join(file.rel_path);
            let exists = file_path.exists();
            let current = exists
                && fs::read_to_string(&file_path)
                    .map(|c| c == file.content)
                    .unwrap_or(false);
            let label = format!("  {}/{}", managed.rel_dir, file.rel_path);
            if current {
                print_status(&format!("{} (current)", label), true);
            } else if exists {
                print_status(&format!("{} (outdated)", label), false);
            } else {
                print_status(&format!("{} (missing)", label), false);
            }
        }

        // Check for orphans
        if target_dir.exists() {
            let expected: std::collections::HashSet<&str> =
                managed.files.iter().map(|f| f.rel_path).collect();
            if let Ok(entries) = fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_file() {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        if !expected.contains(name_str.as_ref()) {
                            println!(
                                "  {} {}/{} (orphan — will be removed on sync)",
                                "⚠".yellow(),
                                managed.rel_dir,
                                name_str,
                            );
                        }
                    }
                }
            }
        }
    }

    // Legacy check
    let legacy_dir = home.join(LEGACY_HOOKS_DIR);
    let legacy_cmd = home.join("commands/playwright-cli.md");
    if legacy_dir.exists() || legacy_cmd.exists() {
        println!();
        println!("  Legacy:");
        if legacy_dir.exists() {
            println!(
                "  {} {} (will be cleaned on sync)",
                "⚠".yellow(),
                legacy_dir.display()
            );
        }
        if legacy_cmd.exists() {
            println!(
                "  {} {} (will be cleaned on sync)",
                "⚠".yellow(),
                legacy_cmd.display()
            );
        }
    }

    println!();
    let all_ok = plugin_ok && claude_md_current;
    if all_ok {
        println!("{}", "✅ All configuration up to date".green());
    } else {
        println!(
            "{}",
            "⚠️  Some items need attention. Run `airis guards install --hooks` to sync.".yellow()
        );
    }

    Ok(())
}

/// Remove airis-managed configuration
pub fn uninstall() -> Result<()> {
    println!(
        "{}",
        "🗑️  Removing airis configuration...".bright_blue()
    );
    println!();

    let home = claude_home()?;

    // 1. Remove managed rules directory
    for managed in templates::managed_dirs() {
        let target_dir = home.join(managed.rel_dir);
        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)
                .with_context(|| format!("Failed to remove {}", target_dir.display()))?;
            println!(
                "   {} Removed {}",
                "✓".green(),
                target_dir.display().to_string().dimmed()
            );
        }
    }

    // 2. Remove CLAUDE.md
    let claude_md_path = home.join("CLAUDE.md");
    if claude_md_path.exists() {
        fs::remove_file(&claude_md_path)
            .with_context(|| format!("Failed to remove {}", claude_md_path.display()))?;
        println!(
            "   {} Removed {}",
            "✓".green(),
            claude_md_path.display().to_string().dimmed()
        );
    }

    // 3. Clean legacy hooks
    clean_legacy_hooks(&home)?;

    println!();
    println!("{}", "✅ airis configuration removed".green());
    println!();
    println!(
        "  {} Plugin uninstall (if needed): {}",
        "ℹ".dimmed(),
        "claude plugin uninstall airis-mcp-gateway".bright_cyan()
    );

    Ok(())
}
