pub mod dir_sync;
pub mod templates;

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::Value;

use crate::manifest::GlobalConfig;

/// Plugin ID in installed_plugins.json
const AIRIS_PLUGIN_ID: &str = "airis-mcp-gateway@airis-mcp-gateway";

/// Legacy hooks directory (to clean up during migration)
const LEGACY_HOOKS_DIR: &str = "hooks/airis";

/// Get the ~/.claude/ directory path
fn claude_home() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".claude"))
}

/// Get the ~/.airis/ directory path
fn airis_home() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".airis"))
}

/// Resolve a source path that may contain ~ for home directory
fn resolve_source_path(source: &str) -> Result<PathBuf> {
    if let Some(rest) = source.strip_prefix("~/") {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(rest))
    } else if let Some(rest) = source.strip_prefix('~') {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(rest))
    } else {
        Ok(PathBuf::from(source))
    }
}

/// Get the registry path for tracking synced files
fn registry_path() -> Result<PathBuf> {
    Ok(airis_home()?.join("claude-registry.toml"))
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
                    cmd.contains("~/.claude/hooks/airis/") || cmd.contains(".claude/hooks/airis/")
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
    let global_config = GlobalConfig::load()?;

    // 1. Check plugin status
    let plugin_ok = check_plugin_installed(&home)?;
    if plugin_ok {
        println!("   {} airis-mcp-gateway plugin installed", "✓".green());
    } else {
        println!("   {} airis-mcp-gateway plugin not found", "⚠".yellow());
        println!(
            "     Run: {}",
            "claude plugin install airis-mcp-gateway".bright_cyan()
        );
    }
    println!();

    // 2. Resolve source directory
    let source_dir = resolve_source_path(&global_config.claude.source)?;

    // 3. Initialize source from embedded templates if not exists
    if !source_dir.exists() {
        templates::initialize_source_dir(&source_dir)?;
        println!(
            "   {} Initialized source: {}",
            "✓".green(),
            source_dir.display().to_string().dimmed()
        );
        println!();
    }

    // 4. Sync from source to ~/.claude/ (registry-based)
    println!(
        "  {} → {}:",
        source_dir.display().to_string().dimmed(),
        home.display().to_string().dimmed()
    );
    let reg_path = registry_path()?;
    let result = dir_sync::sync_from_source(&source_dir, &home, &reg_path)?;

    for path in &result.written {
        println!("   {} {}", "✓".green(), path.dimmed());
    }
    for path in &result.deleted {
        println!("   {} {} (orphan removed)", "✓".green(), path.dimmed());
    }
    for path in &result.unchanged {
        println!("   {} {} (unchanged)", "–".dimmed(), path.dimmed());
    }
    println!();

    // 5. Clean legacy hooks (transition period)
    let legacy_count = clean_legacy_hooks(&home)?;
    if legacy_count > 0 {
        println!(
            "  {} Cleaned {} legacy file(s)",
            "🧹".dimmed(),
            legacy_count
        );
        println!();
    }

    println!("{}", "✅ Global configuration synced".green());

    Ok(())
}

/// Show current setup status
pub fn status() -> Result<()> {
    println!(
        "{}",
        "Claude Code Configuration Status".bright_blue().bold()
    );
    println!();

    let home = claude_home()?;
    let global_config = GlobalConfig::load()?;
    let source_dir = resolve_source_path(&global_config.claude.source)?;

    // Plugin status
    let plugin_ok = check_plugin_installed(&home)?;
    println!("  Plugin:");
    print_status(
        &format!("  {} (hooks, skills, permissions)", AIRIS_PLUGIN_ID),
        plugin_ok,
    );

    // Source directory
    println!();
    println!("  Source:");
    print_status(&format!("  {}", source_dir.display()), source_dir.exists());

    // Registry-tracked files
    let reg_path = registry_path()?;
    let registry = dir_sync::load_claude_registry(&reg_path);

    println!();
    println!("  Synced files:");
    let mut all_synced = true;
    if registry.is_empty() {
        println!("    (none — run `airis claude setup` to sync)");
        all_synced = false;
    } else {
        for rel_path in &registry {
            let target = home.join(rel_path);
            let source = source_dir.join(rel_path);
            let target_exists = target.exists();
            let in_sync = target_exists
                && source.exists()
                && fs::read_to_string(&target).unwrap_or_default()
                    == fs::read_to_string(&source).unwrap_or_default();
            if in_sync {
                print_status(&format!("  {} (current)", rel_path), true);
            } else if target_exists {
                print_status(&format!("  {} (outdated)", rel_path), false);
                all_synced = false;
            } else {
                print_status(&format!("  {} (missing)", rel_path), false);
                all_synced = false;
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
    if plugin_ok && all_synced {
        println!("{}", "✅ All configuration up to date".green());
    } else {
        println!(
            "{}",
            "⚠️  Some items need attention. Run `airis claude setup` to sync.".yellow()
        );
    }

    Ok(())
}

/// Remove airis-managed configuration from ~/.claude/
pub fn uninstall() -> Result<()> {
    println!("{}", "🗑️  Removing airis configuration...".bright_blue());
    println!();

    let home = claude_home()?;

    // 1. Remove registry-tracked files only
    let reg_path = registry_path()?;
    let registry = dir_sync::load_claude_registry(&reg_path);
    for rel_path in &registry {
        let target = home.join(rel_path);
        if target.exists() {
            fs::remove_file(&target)
                .with_context(|| format!("Failed to remove {}", target.display()))?;
            println!(
                "   {} Removed {}",
                "✓".green(),
                target.display().to_string().dimmed()
            );
        }
    }

    // 2. Clear registry
    if reg_path.exists() {
        fs::remove_file(&reg_path)?;
    }

    // 3. Clean legacy hooks
    clean_legacy_hooks(&home)?;

    println!();
    println!("{}", "✅ airis configuration removed".green());
    println!();
    println!(
        "  {} Source directory preserved: {}",
        "ℹ".dimmed(),
        "~/.airis/claude/".bright_cyan()
    );
    println!(
        "  {} Plugin uninstall (if needed): {}",
        "ℹ".dimmed(),
        "claude plugin uninstall airis-mcp-gateway".bright_cyan()
    );

    Ok(())
}
