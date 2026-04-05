use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use serde::Deserialize;

use crate::manifest::{GlobalConfig, MANIFEST_FILE, Manifest, Mode};

use super::scripts::{install_deny_guard, install_wrap_guard, is_airis_guard};
use super::{GUARDS_DIR, HYBRID_MODE_ALLOW, STRICT_MODE_DENY};

/// Lightweight guard config for repos without manifest.toml
const REPO_GUARDS_FILE: &str = ".airis/guards.toml";

/// Guard config loaded from .airis/guards.toml
#[derive(Debug, Deserialize, Default)]
struct RepoGuardsFile {
    #[serde(default)]
    guards: RepoGuards,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct RepoGuards {
    #[serde(default)]
    pub(super) deny: Vec<String>,
    #[serde(default)]
    pub(super) allow: Vec<String>,
}

/// Load guard config from .airis/guards.toml (for non-manifest repos)
fn load_repo_guards() -> Result<RepoGuards> {
    let path = Path::new(REPO_GUARDS_FILE);
    if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", REPO_GUARDS_FILE))?;
        let config: RepoGuardsFile = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", REPO_GUARDS_FILE))?;
        Ok(config.guards)
    } else {
        Ok(RepoGuards::default())
    }
}

/// Merge global + repo config into effective deny list
///
/// Algorithm:
///   effective = (global.deny - global.allow)
///             + repo.deny
///             - repo.allow
///             ± mode adjustment (hybrid/strict, only if manifest exists)
pub(super) fn merge_deny_list(
    global: &GlobalConfig,
    repo: &RepoGuards,
    manifest_deny: &[String],
    manifest_allow: &[String],
    mode: &Mode,
) -> Vec<String> {
    let mut effective: Vec<String> = global
        .guards
        .deny
        .iter()
        .filter(|cmd| !global.guards.allow.contains(cmd))
        .cloned()
        .collect();

    // Add manifest deny list
    for cmd in manifest_deny {
        if !effective.contains(cmd) {
            effective.push(cmd.clone());
        }
    }

    // Add repo-level deny
    for cmd in &repo.deny {
        if !effective.contains(cmd) {
            effective.push(cmd.clone());
        }
    }

    // Remove manifest-level allow
    for cmd in manifest_allow {
        effective.retain(|c| c != cmd);
    }

    // Remove repo-level allow
    for cmd in &repo.allow {
        effective.retain(|c| c != cmd);
    }

    // Apply mode adjustment
    match mode {
        Mode::DockerFirst => {}
        Mode::Hybrid => {
            effective.retain(|cmd| !HYBRID_MODE_ALLOW.contains(&cmd.as_str()));
        }
        Mode::Strict => {
            for cmd in STRICT_MODE_DENY {
                if !effective.contains(&cmd.to_string()) {
                    effective.push(cmd.to_string());
                }
            }
        }
    }

    effective
}

/// Install command guards (works with or without manifest.toml)
pub fn install() -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    let global_config = GlobalConfig::load()?;
    let repo_guards = load_repo_guards()?;

    let (effective_deny, wrap, deny_with_message, mode) = if manifest_path.exists() {
        let manifest = Manifest::load(manifest_path)?;
        let mode = manifest.mode.clone();

        let effective = merge_deny_list(
            &global_config,
            &repo_guards,
            &manifest.guards.deny,
            &manifest.guards.allow,
            &mode,
        );

        (
            effective,
            manifest.guards.wrap,
            manifest.guards.deny_with_message,
            mode,
        )
    } else {
        let mode = Mode::default();
        let effective = merge_deny_list(&global_config, &repo_guards, &[], &[], &mode);

        (effective, IndexMap::new(), IndexMap::new(), mode)
    };

    // Show mode
    match &mode {
        Mode::DockerFirst => {
            println!("{}", "Mode: docker-first (standard guards)".dimmed());
        }
        Mode::Hybrid => {
            println!("{}", "Mode: hybrid (allowing local toolchains)".yellow());
        }
        Mode::Strict => {
            println!("{}", "Mode: strict (maximum restrictions)".bright_red());
        }
    }

    if effective_deny.is_empty() && wrap.is_empty() && deny_with_message.is_empty() {
        println!(
            "{}",
            "⚠️  No guards to install (all commands allowed in this mode)".yellow()
        );
        return Ok(());
    }

    println!("{}", "🛡️  Installing command guards...".bright_blue());
    println!();

    let guards_dir = PathBuf::from(GUARDS_DIR);
    fs::create_dir_all(&guards_dir).with_context(|| format!("Failed to create {}", GUARDS_DIR))?;

    let mut installed_count = 0;

    // Install deny guards (merged)
    for cmd in &effective_deny {
        install_deny_guard(&guards_dir, cmd, None)?;
        installed_count += 1;
        println!("   {} {}", "✓".green(), format!("{} (deny)", cmd).dimmed());
    }

    // Install wrap guards
    for (cmd, wrapper) in &wrap {
        install_wrap_guard(&guards_dir, cmd, wrapper)?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} → {}", cmd, wrapper).dimmed()
        );
    }

    // Install deny with message guards
    for (cmd, message) in &deny_with_message {
        install_deny_guard(&guards_dir, cmd, Some(message))?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} (deny with message)", cmd).dimmed()
        );
    }

    println!();
    println!(
        "{}",
        format!("✅ {} command guards installed", installed_count).green()
    );

    // Show config sources
    if !manifest_path.exists() {
        if Path::new(REPO_GUARDS_FILE).exists() {
            println!(
                "{}",
                format!("   Config: {} + global", REPO_GUARDS_FILE).dimmed()
            );
        } else {
            println!("{}", "   Config: global only".dimmed());
        }
    }

    println!();
    println!("{}", "To activate guards:".bright_yellow());
    println!("  export PATH=\"$PWD/{}:$PATH\"", GUARDS_DIR);
    println!();
    println!("{}", "Or use airis shell:".bright_yellow());
    println!("  airis shell");

    Ok(())
}

/// Check if a command is allowed in the current repo.
/// Used by global guard scripts: `airis guards check-allow <cmd>`
/// Exit 0 = allowed (opt-out), Exit 1 = not allowed (block it)
pub fn check_allow(cmd: &str) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    let global_config = GlobalConfig::load()?;
    let repo_guards = load_repo_guards()?;

    // Collect all allow sources
    let mut allowed = false;

    // Global allow
    if global_config.guards.allow.iter().any(|c| c == cmd) {
        allowed = true;
    }

    // Repo-level allow (.airis/guards.toml)
    if repo_guards.allow.iter().any(|c| c == cmd) {
        allowed = true;
    }

    // Manifest-level allow
    if manifest_path.exists()
        && let Ok(manifest) = Manifest::load(manifest_path)
        && manifest.guards.allow.iter().any(|c| c == cmd)
    {
        allowed = true;
    }

    if allowed {
        // Exit 0 — command is allowed in this repo
        Ok(())
    } else {
        anyhow::bail!("blocked");
    }
}

/// Check if running inside Docker container (respects mode)
pub fn check_docker() -> Result<()> {
    // Load manifest to check mode
    let manifest_path = Path::new(MANIFEST_FILE);
    let mode = if manifest_path.exists() {
        Manifest::load(manifest_path)
            .map(|m| m.mode)
            .unwrap_or_default()
    } else {
        Mode::default()
    };

    // Hybrid mode allows host execution
    if matches!(mode, Mode::Hybrid) {
        println!("{}", "✅ Hybrid mode: host execution allowed".green());
        return Ok(());
    }

    println!("{}", "🔍 Checking Docker environment...".bright_blue());

    // Check for /.dockerenv file (highest reliability — hard to spoof)
    if Path::new("/.dockerenv").exists() {
        println!(
            "{}",
            "✅ Running inside Docker container (/.dockerenv exists)".green()
        );
        return Ok(());
    }

    // Check /proc/1/cgroup for Docker (high reliability)
    if let Ok(content) = fs::read_to_string("/proc/1/cgroup")
        && (content.contains("docker") || content.contains("containerd"))
    {
        println!(
            "{}",
            "✅ Running inside Docker container (cgroup detected)".green()
        );
        return Ok(());
    }

    // Check DOCKER_CONTAINER environment variable (fallback — can be spoofed)
    if std::env::var("DOCKER_CONTAINER").unwrap_or_default() == "true" {
        eprintln!(
            "{}",
            "⚠️  Docker detected via environment variable only. Consider using /.dockerenv for reliable detection.".yellow()
        );
        println!(
            "{}",
            "✅ Running inside Docker container (DOCKER_CONTAINER=true)".green()
        );
        return Ok(());
    }

    // Check for CI environment
    if std::env::var("CI").unwrap_or_default() == "true"
        || std::env::var("GITHUB_ACTIONS").unwrap_or_default() == "true"
        || std::env::var("GITLAB_CI").unwrap_or_default() == "true"
    {
        println!("{}", "✅ Running in CI environment".green());
        return Ok(());
    }

    // Not in Docker - show error
    println!();
    println!("{}", "=".repeat(70).red());
    println!(
        "{}",
        "❌ CRITICAL ERROR: Not running inside Docker container"
            .red()
            .bold()
    );
    println!("{}", "=".repeat(70).red());
    println!();
    println!("{}", "【問題】".bright_yellow());
    println!("  Mac ホスト上で実行しようとしています。");
    println!("  Docker-First開発では、全てのコマンドはDocker内で実行する必要があります。");
    println!();
    println!("{}", "【正しい使用方法】".bright_yellow());
    println!(
        "  1. {} # Docker ワークスペースに入る",
        "airis shell".cyan()
    );
    println!("  2. コマンドを実行");
    println!();
    println!("{}", "【または】".bright_yellow());
    println!(
        "  {} # コンテナ内で直接実行",
        "airis exec workspace <command>".cyan()
    );
    println!();
    println!("{}", "【ヒント】".bright_yellow());
    println!(
        "  ホストでの実行を許可するには、manifest.toml で mode = \"hybrid\" を設定してください"
    );
    println!();
    println!("{}", "=".repeat(70).red());

    anyhow::bail!("Not running inside Docker container");
}

/// Show guard status
pub fn status() -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    let guards_dir = PathBuf::from(GUARDS_DIR);

    println!("{}", "🛡️  Guard Status".bright_blue());
    println!();

    // Check if guards directory exists
    if !guards_dir.exists() {
        println!(
            "{}",
            "Guards not installed. Run: airis guards install".yellow()
        );
        return Ok(());
    }

    if manifest_path.exists() {
        let manifest = Manifest::load(manifest_path)?;

        // Show deny guards
        if !manifest.guards.deny.is_empty() {
            println!("{}", "Deny guards:".bright_yellow());
            for cmd in &manifest.guards.deny {
                let guard_path = guards_dir.join(cmd);
                if guard_path.is_symlink() {
                    println!("  {} {} (SYMLINK — security risk!)", "🚨".red(), cmd);
                } else if guard_path.exists() {
                    println!("  {} {}", "✓".green(), cmd);
                } else {
                    println!("  {} {}", "✗".red(), cmd);
                }
            }
            println!();
        }

        // Show wrap guards
        if !manifest.guards.wrap.is_empty() {
            println!("{}", "Wrap guards:".bright_yellow());
            for (cmd, wrapper) in &manifest.guards.wrap {
                let guard_path = guards_dir.join(cmd);
                if guard_path.is_symlink() {
                    println!(
                        "  {} {} → {} (SYMLINK — security risk!)",
                        "🚨".red(),
                        cmd,
                        wrapper.dimmed()
                    );
                } else if guard_path.exists() {
                    println!("  {} {} → {}", "✓".green(), cmd, wrapper.dimmed());
                } else {
                    println!("  {} {} → {}", "✗".red(), cmd, wrapper.dimmed());
                }
            }
            println!();
        }

        // Show deny with message
        if !manifest.guards.deny_with_message.is_empty() {
            println!("{}", "Deny with message:".bright_yellow());
            for (cmd, _) in &manifest.guards.deny_with_message {
                let guard_path = guards_dir.join(cmd);
                let status = if guard_path.exists() {
                    "✓".green()
                } else {
                    "✗".red()
                };
                println!("  {} {}", status, cmd);
            }
        }
    } else {
        // No manifest — show installed guards from directory
        println!("{}", "Installed guards (no manifest.toml):".bright_yellow());
        for entry in fs::read_dir(&guards_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_airis_guard(&path)? {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                println!("  {} {}", "✓".green(), name);
            }
        }
    }

    Ok(())
}

/// Uninstall local guards
pub fn uninstall() -> Result<()> {
    let guards_dir = PathBuf::from(GUARDS_DIR);

    if !guards_dir.exists() {
        println!(
            "{}",
            "⚠️  Guards not installed (no .airis/bin directory)".yellow()
        );
        return Ok(());
    }

    println!("{}", "🗑️  Uninstalling local guards...".bright_blue());
    println!();

    let mut removed_count = 0;

    // Remove only files that have the airis guard marker
    for entry in fs::read_dir(&guards_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_airis_guard(&path)? {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            fs::remove_file(&path)?;
            println!(
                "   {} {}",
                "✓".green(),
                format!("Removed {}", name).dimmed()
            );
            removed_count += 1;
        }
    }

    // Remove directory if empty
    if guards_dir.read_dir()?.next().is_none() {
        fs::remove_dir(&guards_dir)?;
        println!(
            "   {} {}",
            "✓".green(),
            "Removed .airis/bin directory".dimmed()
        );
    }

    println!();
    println!(
        "{}",
        format!("✅ {} guard(s) uninstalled", removed_count).green()
    );

    Ok(())
}
