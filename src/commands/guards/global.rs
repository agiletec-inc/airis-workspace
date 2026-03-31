use std::fs;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::GlobalConfig;

use super::scripts::{install_global_guard, is_global_guard};

/// Install global guards to ~/.airis/bin
pub fn install_global() -> Result<()> {
    println!("{}", "🌍 Installing global guards...".bright_blue());
    println!();

    // Load or create global config
    let config = GlobalConfig::load()?;
    let config_path = GlobalConfig::config_path()?;
    let bin_dir = GlobalConfig::bin_dir()?;

    // Create directories
    fs::create_dir_all(&bin_dir).with_context(|| format!("Failed to create {:?}", bin_dir))?;

    // Save default config if it doesn't exist
    if !config_path.exists() {
        config.save()?;
        println!(
            "   {} {}",
            "✓".green(),
            format!("Created {}", config_path.display()).dimmed()
        );
    }

    let mut installed_count = 0;

    // Install guard scripts for each denied command
    for cmd in &config.guards.deny {
        install_global_guard(&bin_dir, cmd)?;
        installed_count += 1;
        println!(
            "   {} {}",
            "✓".green(),
            format!("{} (global guard)", cmd).dimmed()
        );
    }

    // Auto-add PATH to shell profiles
    let path_line = "export PATH=\"$HOME/.airis/bin:$PATH\"  # airis guards";
    let mut path_added = false;

    for rc_file in &[".zshrc", ".bashrc"] {
        let home = dirs::home_dir().context("Failed to detect home directory")?;
        let rc_path = home.join(rc_file);

        if !rc_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&rc_path)
            .with_context(|| format!("Failed to read ~/{}", rc_file))?;

        if content.contains(".airis/bin") {
            println!(
                "   {} {}",
                "✓".green(),
                format!("~/{} already has PATH entry", rc_file).dimmed()
            );
            continue;
        }

        // Append PATH entry
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&rc_path)
            .with_context(|| format!("Failed to open ~/{} for writing", rc_file))?;

        use std::io::Write;
        writeln!(file)?;
        writeln!(file, "{}", path_line)?;

        println!(
            "   {} {}",
            "✓".green(),
            format!("Added PATH to ~/{}", rc_file).cyan()
        );
        path_added = true;
    }

    println!();
    println!(
        "{}",
        format!("✅ {} global guard(s) installed", installed_count).green()
    );
    println!();
    println!("{}", "📁 Config:".bright_yellow());
    println!("   {}", config_path.display());

    if path_added {
        println!();
        println!("{}", "🔧 Reload your shell to activate:".bright_yellow());
        println!("   {}", "source ~/.zshrc".cyan());
    }

    Ok(())
}

/// Show global guards status
pub fn status_global() -> Result<()> {
    let config_path = GlobalConfig::config_path()?;
    let bin_dir = GlobalConfig::bin_dir()?;

    println!("{}", "🌍 Global Guard Status".bright_blue());
    println!();

    // Check config file
    println!("{}", "Config file:".bright_yellow());
    if config_path.exists() {
        println!("   {} {}", "✓".green(), config_path.display());
    } else {
        println!(
            "   {} {} (will use defaults)",
            "✗".yellow(),
            config_path.display()
        );
    }
    println!();

    // Check bin directory
    println!("{}", "Guard directory:".bright_yellow());
    if bin_dir.exists() {
        println!("   {} {}", "✓".green(), bin_dir.display());
    } else {
        println!("   {} {} (not created)", "✗".yellow(), bin_dir.display());
        println!();
        println!(
            "{}",
            "Run 'airis guards --global install' to install guards".yellow()
        );
        return Ok(());
    }
    println!();

    // Load config and show guards
    let config = GlobalConfig::load()?;

    println!("{}", "Installed guards:".bright_yellow());
    for cmd in &config.guards.deny {
        let guard_path = bin_dir.join(cmd);
        let status = if guard_path.exists() && is_global_guard(&guard_path)? {
            "✓".green()
        } else if guard_path.exists() {
            "?".yellow() // File exists but not our guard
        } else {
            "✗".red()
        };
        println!("   {} {}", status, cmd);
    }
    println!();

    // Check PATH
    println!("{}", "PATH check:".bright_yellow());
    let path_var = std::env::var("PATH").unwrap_or_default();
    let bin_dir_str = bin_dir.to_string_lossy();
    if path_var.contains(&*bin_dir_str) {
        println!("   {} ~/.airis/bin is in PATH", "✓".green());
    } else {
        println!("   {} ~/.airis/bin is NOT in PATH", "✗".red());
        println!();
        println!("{}", "Add to your ~/.zshrc or ~/.bashrc:".bright_yellow());
        println!("   {}", "export PATH=\"$HOME/.airis/bin:$PATH\"".cyan());
    }

    Ok(())
}

/// Uninstall global guards
pub fn uninstall_global() -> Result<()> {
    let bin_dir = GlobalConfig::bin_dir()?;

    if !bin_dir.exists() {
        println!(
            "{}",
            "⚠️  Global guards not installed (no ~/.airis/bin directory)".yellow()
        );
        return Ok(());
    }

    println!("{}", "🗑️  Uninstalling global guards...".bright_blue());
    println!();

    let mut removed_count = 0;

    // Remove only files that have the global guard marker
    for entry in fs::read_dir(&bin_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_global_guard(&path)? {
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
    if bin_dir.read_dir()?.next().is_none() {
        fs::remove_dir(&bin_dir)?;
        println!(
            "   {} {}",
            "✓".green(),
            "Removed ~/.airis/bin directory".dimmed()
        );
    }

    println!();
    println!(
        "{}",
        format!("✅ {} global guard(s) uninstalled", removed_count).green()
    );

    Ok(())
}

/// Verify global guards are properly installed and active
pub fn verify_global() -> Result<()> {
    let bin_dir = GlobalConfig::bin_dir()?;
    let config = GlobalConfig::load()?;

    println!("{}", "🔍 Guard Verification".bright_blue());
    println!();

    let mut all_ok = true;

    // 1. Check ~/.airis/bin/ directory exists
    if bin_dir.exists() {
        println!("  {} ~/.airis/bin/ exists", "✓".green());
    } else {
        println!("  {} ~/.airis/bin/ does not exist", "✗".red());
        all_ok = false;
    }

    // 2. Check each guard script exists, is not a symlink, and has correct marker
    for cmd in &config.guards.deny {
        let guard_path = bin_dir.join(cmd);
        if guard_path.is_symlink() {
            let target = fs::read_link(&guard_path)
                .map(|t| t.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            println!(
                "  {} {} is a SYMLINK → {} (security risk!)",
                "🚨".red(),
                cmd,
                target.red()
            );
            all_ok = false;
        } else if guard_path.exists() {
            if is_global_guard(&guard_path)? {
                println!("  {} {} guard installed", "✓".green(), cmd);
            } else {
                println!("  {} {} exists but is NOT an airis guard", "✗".red(), cmd);
                all_ok = false;
            }
        } else {
            println!("  {} {} guard not installed", "✗".red(), cmd);
            all_ok = false;
        }
    }

    // 3. Check PATH contains ~/.airis/bin
    let path_var = std::env::var("PATH").unwrap_or_default();
    let bin_dir_str = bin_dir.to_string_lossy();
    if path_var.contains(&*bin_dir_str) {
        println!("  {} ~/.airis/bin is in PATH", "✓".green());
    } else {
        println!("  {} ~/.airis/bin is NOT in PATH", "✗".red());
        all_ok = false;
    }

    // 4. Check which <cmd> points to guard script
    for cmd in &config.guards.deny {
        let output = std::process::Command::new("which").arg(cmd).output();

        match output {
            Ok(out) if out.status.success() => {
                let resolved = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if resolved.contains(".airis/bin") {
                    println!(
                        "  {} which {} → {} (guarded)",
                        "✓".green(),
                        cmd,
                        resolved.dimmed()
                    );
                } else {
                    println!(
                        "  {} which {} → {} (NOT guarded)",
                        "✗".red(),
                        cmd,
                        resolved.yellow()
                    );
                    all_ok = false;
                }
            }
            _ => {
                println!("  {} which {} → not found", "?".yellow(), cmd);
            }
        }
    }

    println!();
    if all_ok {
        println!("{}", "✅ All guards are active!".green().bold());
    } else {
        println!(
            "{}",
            "⚠️  Some guards are not properly configured.".yellow()
        );
        println!();
        println!("Run to fix:");
        println!("  {}", "airis guards --global install".cyan());
        println!("  {}", "source ~/.zshrc".cyan());
    }

    Ok(())
}
