use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::{MANIFEST_FILE, Manifest, Mode};

use super::scripts::{install_deny_guard, install_wrap_guard, is_airis_guard};
use super::{GUARDS_DIR, HYBRID_MODE_ALLOW, STRICT_MODE_DENY};

/// Install command guards from manifest.toml
pub fn install() -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        anyhow::bail!("manifest.toml not found. Run `airis init` first.");
    }

    let manifest = Manifest::load(manifest_path)?;

    // Collect effective deny list based on mode
    let mut effective_deny: Vec<String> = manifest.guards.deny.clone();

    // Apply mode-specific guards
    match &manifest.mode {
        Mode::DockerFirst => {
            // Standard mode: use guards as configured
            println!("{}", "Mode: docker-first (standard guards)".dimmed());
        }
        Mode::Hybrid => {
            // Hybrid mode: allow local toolchains (cargo, python, etc.)
            println!("{}", "Mode: hybrid (allowing local toolchains)".yellow());
            effective_deny.retain(|cmd| !HYBRID_MODE_ALLOW.contains(&cmd.as_str()));
        }
        Mode::Strict => {
            // Strict mode: block additional commands
            println!("{}", "Mode: strict (maximum restrictions)".bright_red());
            for cmd in STRICT_MODE_DENY {
                if !effective_deny.contains(&cmd.to_string()) {
                    effective_deny.push(cmd.to_string());
                }
            }
        }
    }

    if effective_deny.is_empty()
        && manifest.guards.wrap.is_empty()
        && manifest.guards.deny_with_message.is_empty()
    {
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

    // Install deny guards (mode-adjusted)
    for cmd in &effective_deny {
        install_deny_guard(&guards_dir, cmd, None)?;
        installed_count += 1;
        println!("   {} {}", "✓".green(), format!("{} (deny)", cmd).dimmed());
    }

    // Install wrap guards
    for (cmd, wrapper) in &manifest.guards.wrap {
        install_wrap_guard(&guards_dir, cmd, wrapper)?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} → {}", cmd, wrapper).dimmed()
        );
    }

    // Install deny with message guards
    for (cmd, message) in &manifest.guards.deny_with_message {
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
    println!();
    println!("{}", "To activate guards:".bright_yellow());
    println!("  export PATH=\"$PWD/{}:$PATH\"", GUARDS_DIR);
    println!();
    println!("{}", "Or use airis shell:".bright_yellow());
    println!("  airis shell");

    Ok(())
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

    // Check DOCKER_CONTAINER environment variable
    if std::env::var("DOCKER_CONTAINER").unwrap_or_default() == "true" {
        println!(
            "{}",
            "✅ Running inside Docker container (DOCKER_CONTAINER=true)".green()
        );
        return Ok(());
    }

    // Check for /.dockerenv file
    if Path::new("/.dockerenv").exists() {
        println!(
            "{}",
            "✅ Running inside Docker container (/.dockerenv exists)".green()
        );
        return Ok(());
    }

    // Check /proc/1/cgroup for Docker
    if let Ok(content) = fs::read_to_string("/proc/1/cgroup")
        && (content.contains("docker") || content.contains("containerd"))
    {
        println!(
            "{}",
            "✅ Running inside Docker container (cgroup detected)".green()
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

    if !manifest_path.exists() {
        println!("{}", "⚠️  manifest.toml not found".yellow());
        return Ok(());
    }

    let manifest = Manifest::load(manifest_path)?;
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
