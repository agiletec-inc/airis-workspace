// `airis ui` — cosmetic Claude Code integrations: the terminal tab-title
// hooks and the statusline.
//
// Unlike `airis guards` (enforcement) these are purely visual. Both ship as
// self-contained shell scripts written into ~/.claude/, and the settings.json
// hooks invoke those scripts directly — they do NOT call the `airis` binary at
// hook time. A rebuilt, stale, or missing `airis` can therefore never break
// them.

use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::Confirm;
use dialoguer::theme::ColorfulTheme;
use serde_json::{Value, json};

use crate::manifest::{GlobalConfig, TerminalTitleSection};

#[cfg(test)]
mod tests;

/// Tab-title hook script, written to ~/.claude/hooks/airis-tab-title.sh.
const TAB_TITLE_SCRIPT: &str = include_str!("../../../templates/ui/airis-tab-title.sh");
/// Statusline script, written to ~/.claude/statusline-command.sh.
const STATUSLINE_SCRIPT: &str = include_str!("../../../templates/ui/statusline.sh");

/// ~/.claude-relative paths of the generated scripts.
const TAB_TITLE_REL: &str = "hooks/airis-tab-title.sh";
const STATUSLINE_REL: &str = "statusline-command.sh";

/// Marker identifying an airis-managed tab-title hook command in settings.json.
const HOOK_MARKER: &str = "airis-tab-title.sh";
/// Markers of superseded tab-title implementations, stripped on install.
const LEGACY_HOOK_MARKERS: &[&str] = &["claude tab-title", "warp-tab-title.sh"];

/// Hook events and the tab-title state each one reports. `matcher` follows
/// Claude Code's hook schema (a regex: tool name for tool events,
/// notification type for `Notification`, empty otherwise).
const HOOK_EVENTS: &[(&str, &str, &str)] = &[
    ("SessionStart", "", "idle"),
    ("UserPromptSubmit", "", "running"),
    ("PostToolUse", "", "running"),
    ("PreToolUse", "AskUserQuestion|ExitPlanMode", "waiting"),
    // Notification matches on the notification type (official Claude Code
    // mechanism): permission / MCP-elicitation prompts mean "waiting on you",
    // an idle prompt just means idle.
    (
        "Notification",
        "permission_prompt|elicitation_dialog",
        "waiting",
    ),
    ("Notification", "idle_prompt", "idle"),
    ("Stop", "", "stop"),
];

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

// ── Public commands ─────────────────────────────────────────────────

/// Install the airis UI integrations into ~/.claude/.
pub fn install() -> Result<()> {
    println!("{}", "🎨 Installing airis UI integrations...".bright_blue());
    println!();

    let home = claude_home()?;
    let cfg = GlobalConfig::load()?;
    let tt = &cfg.claude.terminal_title;

    // 1. Write the self-contained scripts.
    write_script(&home.join(TAB_TITLE_REL), TAB_TITLE_SCRIPT)?;
    println!("   {} {}", "✓".green(), TAB_TITLE_REL.dimmed());
    write_script(&home.join(STATUSLINE_REL), STATUSLINE_SCRIPT)?;
    println!("   {} {}", "✓".green(), STATUSLINE_REL.dimmed());

    // 2. Wire settings.json to call those scripts.
    let settings_path = home.join("settings.json");
    let mut settings = load_settings(&settings_path)?;
    let hooks_n = wire_tab_title(&mut settings, &home, tt);
    wire_statusline(&mut settings, &home);
    write_settings(&settings_path, &settings)?;

    println!();
    if hooks_n > 0 {
        println!(
            "   {} Tab-title hooks wired ({} events)",
            "✓".green(),
            hooks_n
        );
    } else {
        println!(
            "   {} Tab-title disabled ([claude.terminal_title] enabled = false)",
            "–".dimmed()
        );
    }
    println!("   {} Statusline wired", "✓".green());
    println!();
    println!("{}", "✅ airis UI installed".green());
    println!(
        "   {} settings.json hot-reloads — a running Claude Code session picks it up.",
        "ℹ".dimmed()
    );

    Ok(())
}

/// Remove the airis UI integrations.
///
/// `purge`: `Some(true)` deletes the generated script files, `Some(false)`
/// keeps them, `None` asks interactively (keeps them when non-interactive).
pub fn uninstall(purge: Option<bool>) -> Result<()> {
    println!("{}", "🗑️  Removing airis UI integrations...".bright_blue());
    println!();

    let home = claude_home()?;

    // 1. Unwire settings.json.
    let settings_path = home.join("settings.json");
    if settings_path.exists() {
        let mut settings = load_settings(&settings_path)?;
        ensure_hooks_object(&mut settings);
        strip_tab_title_entries(&mut settings);
        unwire_statusline(&mut settings);
        write_settings(&settings_path, &settings)?;
        println!(
            "   {} Unwired tab-title hooks + statusline from settings.json",
            "✓".green()
        );
    }

    // 2. Decide whether to delete the generated script files.
    let scripts = [home.join(TAB_TITLE_REL), home.join(STATUSLINE_REL)];
    let existing: Vec<&PathBuf> = scripts.iter().filter(|p| p.exists()).collect();

    let remove = if existing.is_empty() {
        false
    } else {
        match purge {
            Some(v) => v,
            None => {
                if std::io::stdin().is_terminal() {
                    Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(
                            "生成したスクリプトファイル (airis-tab-title.sh, statusline-command.sh) も削除しますか？",
                        )
                        .default(false)
                        .interact()?
                } else {
                    // Non-interactive without a flag: keep the files (safe default).
                    false
                }
            }
        }
    };

    println!();
    if remove {
        for p in &existing {
            fs::remove_file(p).with_context(|| format!("Failed to remove {}", p.display()))?;
            println!(
                "   {} {} deleted",
                "✓".green(),
                p.display().to_string().dimmed()
            );
        }
    } else {
        for p in &existing {
            println!(
                "   {} {} kept",
                "ℹ".dimmed(),
                p.display().to_string().dimmed()
            );
        }
    }

    println!();
    println!("{}", "✅ airis UI removed".green());

    Ok(())
}

/// Show the airis UI install status.
pub fn status() -> Result<()> {
    println!("{}", "airis UI Status".bright_blue().bold());
    println!();

    let home = claude_home()?;
    let cfg = GlobalConfig::load()?;

    println!("  Scripts:");
    print_status(TAB_TITLE_REL, home.join(TAB_TITLE_REL).exists());
    print_status(STATUSLINE_REL, home.join(STATUSLINE_REL).exists());

    let settings = load_settings(&home.join("settings.json")).unwrap_or_else(|_| json!({}));
    let hook_count = count_tab_title_entries(&settings);
    let statusline_wired = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd.contains(STATUSLINE_REL));

    println!();
    println!("  settings.json:");
    print_status(
        &format!("tab-title hooks ({hook_count} events)"),
        hook_count > 0,
    );
    print_status("statusLine", statusline_wired);

    let tt = &cfg.claude.terminal_title;
    let idle = if tt.idle.is_empty() { "—" } else { &tt.idle };
    println!();
    println!("  Config ([claude.terminal_title]):");
    println!(
        "    enabled={} running={} waiting={} idle={}",
        tt.enabled, tt.running, tt.waiting, idle
    );

    Ok(())
}

// ── Script files ────────────────────────────────────────────────────

/// Write a generated script, creating parent dirs and marking it executable.
fn write_script(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

// ── settings.json wiring ────────────────────────────────────────────

fn load_settings(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&content).with_context(|| format!("{} is not valid JSON", path.display()))
}

fn write_settings(path: &Path, settings: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }
    let mut content = serde_json::to_string_pretty(settings)?;
    content.push('\n');
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

/// Ensure `settings` is an object with a `hooks` object.
fn ensure_hooks_object(settings: &mut Value) {
    if !settings.is_object() {
        *settings = json!({});
    }
    if !settings.get("hooks").map(Value::is_object).unwrap_or(false) {
        settings["hooks"] = json!({});
    }
}

/// Strip managed/legacy tab-title hook entries, then re-add the current set.
/// Returns the number of hook entries added (0 when tab-title is disabled).
fn wire_tab_title(settings: &mut Value, claude_home: &Path, tt: &TerminalTitleSection) -> usize {
    ensure_hooks_object(settings);
    strip_tab_title_entries(settings);

    if !tt.enabled {
        return 0;
    }

    let script = claude_home.join(TAB_TITLE_REL);
    let script = script.display().to_string();
    let hooks = settings["hooks"].as_object_mut().expect("hooks is object");
    let mut count = 0;
    for (event, matcher, state) in HOOK_EVENTS {
        // Quote the script path so an install path with spaces still works.
        // The configured emoji are baked in as positional args.
        let command = format!(
            "sh \"{}\" {} \"{}\" \"{}\" \"{}\"",
            script, state, tt.running, tt.waiting, tt.idle
        );
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
        count += 1;
    }
    count
}

/// Remove every airis-managed (current or legacy) tab-title hook entry.
fn strip_tab_title_entries(settings: &mut Value) {
    if let Some(hooks) = settings["hooks"].as_object_mut() {
        for entries in hooks.values_mut() {
            if let Some(arr) = entries.as_array_mut() {
                arr.retain(|entry| !is_tab_title_entry(entry));
            }
        }
    }
}

/// True for a hook entry whose command invokes an airis tab-title script
/// (current `airis-tab-title.sh` or a superseded implementation).
///
/// Entries without a `hooks[].command` string — e.g. `type:"agent"` hooks —
/// never match, so user hooks are preserved.
fn is_tab_title_entry(entry: &Value) -> bool {
    let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) else {
        return false;
    };
    hooks.iter().any(|hook| {
        hook.get("command")
            .and_then(|c| c.as_str())
            .is_some_and(|cmd| {
                cmd.contains(HOOK_MARKER) || LEGACY_HOOK_MARKERS.iter().any(|m| cmd.contains(m))
            })
    })
}

/// Count airis-managed tab-title hook entries in a settings value.
fn count_tab_title_entries(settings: &Value) -> usize {
    settings
        .get("hooks")
        .and_then(|h| h.as_object())
        .map(|hooks| {
            hooks
                .values()
                .filter_map(|entries| entries.as_array())
                .flatten()
                .filter(|entry| is_tab_title_entry(entry))
                .count()
        })
        .unwrap_or(0)
}

/// Point settings.json `statusLine` at the airis-managed script.
fn wire_statusline(settings: &mut Value, claude_home: &Path) {
    if !settings.is_object() {
        *settings = json!({});
    }
    let script = claude_home.join(STATUSLINE_REL);
    settings["statusLine"] = json!({
        "type": "command",
        "command": format!("sh \"{}\"", script.display()),
    });
}

/// Remove the `statusLine` key if it points at the airis-managed script.
fn unwire_statusline(settings: &mut Value) {
    let is_ours = settings
        .get("statusLine")
        .and_then(|s| s.get("command"))
        .and_then(|c| c.as_str())
        .is_some_and(|cmd| cmd.contains(STATUSLINE_REL));
    if !is_ours {
        return;
    }
    if let Some(obj) = settings.as_object_mut() {
        obj.remove("statusLine");
    }
}
