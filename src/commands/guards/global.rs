use super::scripts::{install_global_guard, is_global_guard};
use crate::manifest::{GlobalConfig, GuardPreset};
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;

/// Install global guards based on preset
pub fn install_global(preset: Option<GuardPreset>) -> Result<()> {
    println!("{}", "🌍 Installing global guards...".bright_blue());

    let mut config = GlobalConfig::load()?;
    if let Some(p) = preset {
        config.guards.preset = p;
    }
    config.save()?;

    let bin_dir = GlobalConfig::bin_dir()?;
    fs::create_dir_all(&bin_dir)?;

    let mut installed_count = 0;
    let commands = config.guards.active_commands();

    for cmd in &commands {
        let level = config.guards.get_level(cmd);
        install_global_guard(&bin_dir, cmd, level)?;
        installed_count += 1;
        println!(
            "   {} {} ({:?})",
            "✓".green(),
            cmd.dimmed(),
            config.guards.get_level(cmd)
        );
    }

    // PATH check and setup
    setup_shell_path()?;

    println!(
        "\n{} {} global guard(s) installed (Preset: {:?})",
        "✅".green(),
        installed_count,
        config.guards.preset
    );

    Ok(())
}

fn setup_shell_path() -> Result<()> {
    let mut path_added = false;
    let mut prompt_added = false;

    for rc_file in &[".zshrc", ".bashrc"] {
        let home = dirs::home_dir().context("No home dir")?;
        let rc_path = home.join(rc_file);
        if !rc_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&rc_path)?;
        let shell_name = if rc_file.contains("zsh") { "zsh" } else { "bash" };

        let mut lines_to_add: Vec<String> = Vec::new();

        // 1. PATH setup
        if !content.contains(".airis/bin") {
            lines_to_add.push("export PATH=\"$HOME/.airis/bin:$PATH\"  # airis guards".to_string());
            path_added = true;
        }

        // 2. Prompt integration setup
        if !content.contains("airis init-shell") {
            use dialoguer::{Confirm, theme::ColorfulTheme};
            let question = format!("Would you like to enable the airis status line in your {} prompt?", shell_name);
            
            if Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(question)
                .default(true)
                .interact()
                .unwrap_or(false) 
            {
                lines_to_add.push(format!("source <(airis init-shell {}) # airis prompt", shell_name));
                prompt_added = true;
            }
        }

        if !lines_to_add.is_empty() {
            use std::io::Write;
            let mut file = fs::OpenOptions::new().append(true).open(&rc_path)?;
            for line in lines_to_add {
                writeln!(file, "{}", line)?;
            }
            println!("   {} Updated ~/{}", "✓".green(), rc_file.cyan());
        }
    }

    if path_added || prompt_added {
        println!(
            "\n{} Shell integration complete. Reload your shell: {}",
            "🔧".yellow(),
            "source ~/.zshrc".cyan()
        );
    }
    Ok(())
}

pub fn status_global() -> Result<()> {
    let config = GlobalConfig::load()?;
    let bin_dir = GlobalConfig::bin_dir()?;

    println!("{}", "🌍 Global Guard Status".bright_blue());
    println!("   Preset: {:?}", config.guards.preset);
    println!();

    if !bin_dir.exists() {
        println!("{}", "✗ Guards not installed in ~/.airis/bin".red());
        return Ok(());
    }

    println!("{}", "Installed guards:".bright_yellow());
    let commands = config.guards.active_commands();
    for cmd in &commands {
        let status = if bin_dir.join(cmd).exists() {
            "✓".green()
        } else {
            "✗".red()
        };
        println!("   {} {} ({:?})", status, cmd, config.guards.get_level(cmd));
    }
    Ok(())
}

pub fn uninstall_global() -> Result<()> {
    let bin_dir = GlobalConfig::bin_dir()?;
    if !bin_dir.exists() {
        println!("{}", "ℹ️  No global guards found to uninstall.".yellow());
        return Ok(());
    }

    println!("{}", "🗑️  Uninstalling global guards...".bright_blue());
    let mut removed = 0;
    for entry in fs::read_dir(&bin_dir)? {
        let path = entry?.path();
        if is_global_guard(&path)? {
            fs::remove_file(path)?;
            removed += 1;
        }
    }

    // Try to remove the bin directory if empty
    if fs::read_dir(&bin_dir)?.next().is_none() {
        fs::remove_dir(&bin_dir)?;
    }

    // Clean up PATH from shell config files
    let mut path_removed = false;
    for rc_file in &[".zshrc", ".bashrc"] {
        let home = dirs::home_dir().context("No home dir")?;
        let rc_path = home.join(rc_file);
        if !rc_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&rc_path)?;
        if content.contains(".airis/bin") {
            let lines: Vec<&str> = content
                .lines()
                .filter(|line| !line.contains(".airis/bin") && !line.contains("# airis guards"))
                .collect();

            let new_content = lines.join("\n") + "\n";
            fs::write(&rc_path, new_content)?;
            println!("   {} Removed PATH from ~/{}", "✓".green(), rc_file.cyan());
            path_removed = true;
        }
    }

    println!(
        "{} Removed {} guards and cleaned up PATH",
        "✅".green(),
        removed
    );

    if path_removed {
        println!(
            "\n{} Your shell config has been updated. Reload your shell: {}",
            "🔧".yellow(),
            "source ~/.zshrc".cyan()
        );
    }

    Ok(())
}

pub fn verify_global() -> Result<()> {
    status_global() // Simplified for now
}
