use anyhow::{Context, Result};
use colored::Colorize;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

const PRE_COMMIT_HOOK: &str = include_str!("../../hooks/pre-commit");
const PRE_PUSH_HOOK: &str = include_str!("../../hooks/pre-push");

/// Marker to identify airis-managed hook entries
const AIRIS_MARKER: &str = "# airis-managed";

/// Install a single Git hook file with executable permissions
fn install_hook(hooks_dir: &Path, name: &str, content: &str) -> Result<()> {
    let hook_path = hooks_dir.join(name);

    fs::write(&hook_path, content)
        .with_context(|| format!("Failed to write {} hook", name))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)?;
    }

    Ok(())
}

/// Remove airis-managed entries from a hook array, keeping other entries intact
fn remove_airis_entries(arr: &mut Vec<Value>) {
    arr.retain(|entry| {
        let dominated_by_airis = entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|c| c.contains(AIRIS_MARKER))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        !dominated_by_airis
    });
}

/// Install Claude Code hooks into ~/.claude/settings.json
fn install_claude_hooks() -> Result<()> {
    let claude_dir = dirs::home_dir()
        .context("Could not determine home directory")?
        .join(".claude");
    let settings_path = claude_dir.join("settings.json");

    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)
            .with_context(|| "Failed to read ~/.claude/settings.json")?;
        serde_json::from_str(&content)
            .with_context(|| "Failed to parse ~/.claude/settings.json")?
    } else {
        fs::create_dir_all(&claude_dir)
            .with_context(|| "Failed to create ~/.claude directory")?;
        json!({})
    };

    let hooks = settings
        .as_object_mut()
        .context("settings.json is not an object")?
        .entry("hooks")
        .or_insert_with(|| json!({}));

    // Stop hook: run tests when Claude finishes a task
    let stop_arr = hooks
        .as_object_mut()
        .context("hooks is not an object")?
        .entry("Stop")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .context("Stop is not an array")?;
    remove_airis_entries(stop_arr);
    stop_arr.push(json!({
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": format!(
                "cd $CLAUDE_PROJECT_DIR && if [ -f manifest.toml ]; then airis test 2>&1 | tail -20; fi {AIRIS_MARKER}"
            )
        }]
    }));

    // PreToolUse hook: block git push if tests fail
    let pre_tool_arr = hooks
        .as_object_mut()
        .context("hooks is not an object")?
        .entry("PreToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .context("PreToolUse is not an array")?;
    remove_airis_entries(pre_tool_arr);
    pre_tool_arr.push(json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": format!(
                r#"bash -c 'input=$(cat); cmd=$(echo "$input" | jq -r ".tool_input.command // empty"); if echo "$cmd" | grep -qE "git\s+push"; then push_repo=$(echo "$cmd" | sed -n "s/.*cd \([^ ;]*\).*/\1/p"); push_root=$(cd "${{push_repo:-.}}" 2>/dev/null && git rev-parse --show-toplevel 2>/dev/null || echo "."); project_root=$(cd "$CLAUDE_PROJECT_DIR" && git rev-parse --show-toplevel 2>/dev/null || echo "$CLAUDE_PROJECT_DIR"); if [ "$push_root" != "$project_root" ]; then echo "Push target differs from project — skipping airis test"; exit 0; fi; cd $CLAUDE_PROJECT_DIR && if [ -f manifest.toml ]; then changed=$(git diff --name-only origin/stg...HEAD 2>/dev/null || git diff --name-only HEAD~1 HEAD); if [ -z "$changed" ]; then echo "No changes to test" && exit 0; fi; has_turbo_pkg=$(echo "$changed" | grep -cE "^(apps|libs|products)/" || true); if [ "$has_turbo_pkg" -eq 0 ]; then echo "Changes only in non-turbo paths — skipping airis test"; exit 0; fi; if ! docker ps --format "{{{{.Names}}}}" 2>/dev/null | grep -q agiletec; then echo "⚠️ No agiletec containers running — skipping local test. CI will validate."; exit 0; fi; airis test || {{ echo "{{\"decision\": \"block\", \"reason\": \"テスト失敗。push 前に修正してください\"}}" >&2; exit 2; }}; fi; fi' {AIRIS_MARKER}"#
            )
        }]
    }));

    let formatted = serde_json::to_string_pretty(&settings)
        .context("Failed to serialize settings")?;
    fs::write(&settings_path, format!("{formatted}\n"))
        .with_context(|| "Failed to write ~/.claude/settings.json")?;

    Ok(())
}

/// Install Git hooks (pre-commit + pre-push) and Claude Code hooks
pub fn install() -> Result<()> {
    // Git hooks
    let git_dir = Path::new(".git");

    if !git_dir.exists() {
        eprintln!("⚠️  Not a git repository. Skipping Git hook installation.");
    } else {
        let hooks_dir = git_dir.join("hooks");
        fs::create_dir_all(&hooks_dir)
            .with_context(|| "Failed to create .git/hooks directory")?;

        install_hook(&hooks_dir, "pre-commit", PRE_COMMIT_HOOK)?;
        install_hook(&hooks_dir, "pre-push", PRE_PUSH_HOOK)?;

        println!("✅ Git hooks installed:");
        println!(
            "   {} → {}",
            "pre-commit".green(),
            hooks_dir.join("pre-commit").display()
        );
        println!(
            "   {} → {}",
            "pre-push".green(),
            hooks_dir.join("pre-push").display()
        );
        println!("   💡 pre-commit: version auto-bump + .env / node_modules guard");
        println!("   💡 pre-push:   lint / typecheck / build (commented out — enable as needed)");
    }

    // Claude Code hooks
    install_claude_hooks()?;
    println!("\n✅ Claude Code hooks installed (~/.claude/settings.json):");
    println!(
        "   {} → タスク完了時に airis test --quick を自動実行",
        "Stop".green()
    );
    println!(
        "   {} → git push 前にテスト通過を強制",
        "PreToolUse (Bash)".green()
    );
    println!("\n💡 Claude Code を再起動すると hooks が有効になります");

    Ok(())
}
