mod entries;
pub mod hooks_merge;
pub mod scripts;

#[cfg(test)]
mod tests;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::Value;

use hooks_merge::{has_airis_hook, merge_airis_hooks, remove_airis_hooks};
use scripts::{
    docker_first_edit_guard_script, docker_first_guard_script, playwright_cli_command,
    stop_test_check_script,
};

/// Directory name for airis-managed hooks inside ~/.claude/hooks/
const AIRIS_HOOKS_DIR: &str = "airis";

/// Get the ~/.claude/ directory path
fn claude_home() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    Ok(home.join(".claude"))
}

/// Get the hooks directory path (~/.claude/hooks/airis/)
fn hooks_dir() -> Result<PathBuf> {
    Ok(claude_home()?.join("hooks").join(AIRIS_HOOKS_DIR))
}

/// Get the commands directory path (~/.claude/commands/)
fn commands_dir() -> Result<PathBuf> {
    Ok(claude_home()?.join("commands"))
}

/// Get the settings.json path (~/.claude/settings.json)
fn settings_path() -> Result<PathBuf> {
    Ok(claude_home()?.join("settings.json"))
}

fn print_status(label: &str, ok: bool) {
    if ok {
        println!("  {} {}", "✓".green(), label);
    } else {
        println!("  {} {}", "✗".red(), label);
    }
}

// ── Public commands ─────────────────────────────────────────────────

/// Install global Claude Code hooks to ~/.claude/
pub fn setup_global() -> Result<()> {
    println!(
        "{}",
        "🛡️  Setting up Claude Code Docker-First hooks...".bright_blue()
    );
    println!();

    // 1. Create hooks directory
    let dir = hooks_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;

    // 2. Write docker-first-guard.sh
    let guard_path = dir.join("docker-first-guard.sh");
    fs::write(&guard_path, docker_first_guard_script())
        .with_context(|| format!("Failed to write {}", guard_path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&guard_path, fs::Permissions::from_mode(0o755))?;
    println!(
        "   {} {}",
        "✓".green(),
        guard_path.display().to_string().dimmed()
    );

    // 3. Write docker-first-edit-guard.sh
    let edit_guard_path = dir.join("docker-first-edit-guard.sh");
    fs::write(&edit_guard_path, docker_first_edit_guard_script())
        .with_context(|| format!("Failed to write {}", edit_guard_path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&edit_guard_path, fs::Permissions::from_mode(0o755))?;
    println!(
        "   {} {}",
        "✓".green(),
        edit_guard_path.display().to_string().dimmed()
    );

    // 5. Write stop-test-check.sh
    let stop_path = dir.join("stop-test-check.sh");
    fs::write(&stop_path, stop_test_check_script())
        .with_context(|| format!("Failed to write {}", stop_path.display()))?;
    #[cfg(unix)]
    fs::set_permissions(&stop_path, fs::Permissions::from_mode(0o755))?;
    println!(
        "   {} {}",
        "✓".green(),
        stop_path.display().to_string().dimmed()
    );

    // 6. Write global command files (~/.claude/commands/)
    let cmd_dir = commands_dir()?;
    fs::create_dir_all(&cmd_dir)
        .with_context(|| format!("Failed to create {}", cmd_dir.display()))?;

    let pw_path = cmd_dir.join("playwright-cli.md");
    fs::write(&pw_path, playwright_cli_command())
        .with_context(|| format!("Failed to write {}", pw_path.display()))?;
    println!(
        "   {} {}",
        "✓".green(),
        pw_path.display().to_string().dimmed()
    );

    // 7. Merge hooks into settings.json
    let settings = settings_path()?;
    let mut value = if settings.exists() {
        let content = fs::read_to_string(&settings)
            .with_context(|| format!("Failed to read {}", settings.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", settings.display()))?
    } else {
        serde_json::json!({})
    };

    merge_airis_hooks(&mut value)?;

    let pretty = serde_json::to_string_pretty(&value)?;
    fs::write(&settings, pretty)
        .with_context(|| format!("Failed to write {}", settings.display()))?;
    println!(
        "   {} {}",
        "✓".green(),
        settings.display().to_string().dimmed()
    );

    println!();
    println!("{}", "✅ Claude Code hooks installed".green());
    println!();
    println!(
        "  {} Docker-First Bash guard blocks host package managers",
        "•".dimmed()
    );
    println!(
        "  {} Docker-First Edit guard blocks host paths in Docker/CI files",
        "•".dimmed()
    );
    println!(
        "  {} Stop hook runs tests when Claude finishes",
        "•".dimmed()
    );
    println!(
        "  {} /playwright-cli command for browser automation",
        "•".dimmed()
    );

    Ok(())
}

/// Show current setup status
pub fn status() -> Result<()> {
    println!("{}", "Claude Code Hook Status".bright_blue().bold());
    println!();

    let dir = hooks_dir()?;

    // Check hook scripts
    let guard_ok = dir.join("docker-first-guard.sh").exists();
    let edit_guard_ok = dir.join("docker-first-edit-guard.sh").exists();
    let stop_ok = dir.join("stop-test-check.sh").exists();

    println!("  Hook scripts:");
    print_status("  docker-first-guard.sh", guard_ok);
    print_status("  docker-first-edit-guard.sh", edit_guard_ok);
    print_status("  stop-test-check.sh", stop_ok);

    // Check settings.json
    let settings = settings_path()?;
    let (pre_tool_ok, stop_hook_ok) = if settings.exists() {
        let content = fs::read_to_string(&settings)?;
        let value: Value = serde_json::from_str(&content).unwrap_or(serde_json::json!({}));
        (
            has_airis_hook(&value, "PreToolUse"),
            has_airis_hook(&value, "Stop"),
        )
    } else {
        (false, false)
    };

    // Check command files
    let cmd_dir = commands_dir()?;
    let pw_ok = cmd_dir.join("playwright-cli.md").exists();

    println!();
    println!("  Command files:");
    print_status("  playwright-cli.md", pw_ok);

    println!();
    println!("  settings.json entries:");
    print_status("  PreToolUse (Docker-First guard)", pre_tool_ok);
    print_status("  Stop (test check)", stop_hook_ok);

    let all_ok = guard_ok && edit_guard_ok && stop_ok && pre_tool_ok && stop_hook_ok && pw_ok;
    println!();
    if all_ok {
        println!("{}", "✅ All hooks installed and configured".green());
    } else {
        println!(
            "{}",
            "⚠️  Some hooks are missing. Run `airis guards install --hooks` to install.".yellow()
        );
    }

    Ok(())
}

/// Remove airis-managed hooks
pub fn uninstall() -> Result<()> {
    println!(
        "{}",
        "🗑️  Removing airis Claude Code hooks...".bright_blue()
    );
    println!();

    // 1. Remove hooks directory
    let dir = hooks_dir()?;
    if dir.exists() {
        fs::remove_dir_all(&dir).with_context(|| format!("Failed to remove {}", dir.display()))?;
        println!(
            "   {} Removed {}",
            "✓".green(),
            dir.display().to_string().dimmed()
        );
    } else {
        println!(
            "   {} {} (not found)",
            "–".dimmed(),
            dir.display().to_string().dimmed()
        );
    }

    // 2. Remove airis entries from settings.json
    let settings = settings_path()?;
    if settings.exists() {
        let content = fs::read_to_string(&settings)?;
        let mut value: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse {}", settings.display()))?;

        remove_airis_hooks(&mut value);

        let pretty = serde_json::to_string_pretty(&value)?;
        fs::write(&settings, pretty)?;
        println!(
            "   {} Cleaned {}",
            "✓".green(),
            settings.display().to_string().dimmed()
        );
    }

    println!();
    println!("{}", "✅ airis hooks removed".green());

    Ok(())
}
