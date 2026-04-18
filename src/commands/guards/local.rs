use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::manifest::{GlobalConfig, GuardLevel, Manifest};

use super::GUARDS_DIR;
use super::scripts::{install_deny_guard, install_wrap_guard, is_airis_guard};

/// Install guards in the current repository based on manifest.toml
pub fn install() -> Result<()> {
    let manifest = Manifest::load("manifest.toml")?;
    let guards_dir = Path::new(GUARDS_DIR);

    if !guards_dir.exists() {
        fs::create_dir_all(guards_dir).with_context(|| "Failed to create guards directory")?;
    }

    let global = GlobalConfig::load()?;
    let mut installed_count = 0;

    // 1. Get default commands from global config preset
    let base_commands = global.guards.active_commands();

    // 2. Combine with manifest-specific denies, excluding global allows
    for cmd in &base_commands {
        if manifest.guards.allow.contains(cmd) {
            continue;
        }
        install_deny_guard(guards_dir, cmd, None)?;
        installed_count += 1;
    }

    // 3. Local manifest-specific guards
    for cmd in &manifest.guards.deny {
        if !base_commands.contains(cmd) {
            install_deny_guard(guards_dir, cmd, None)?;
            installed_count += 1;
        }
    }

    for (cmd, wrapper) in &manifest.guards.wrap {
        install_wrap_guard(guards_dir, cmd, wrapper)?;
        installed_count += 1;
    }

    println!(
        "{} Installed {} command guard(s) to {}",
        "✓".green(),
        installed_count,
        GUARDS_DIR.cyan()
    );

    Ok(())
}

pub fn status() -> Result<()> {
    let guards_dir = Path::new(GUARDS_DIR);
    if !guards_dir.exists() {
        println!("{}", "✗ No command guards installed locally".red());
        return Ok(());
    }

    println!("{}", "🛡️  Local Command Guards".bright_blue());
    for entry in fs::read_dir(guards_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_airis_guard(&path)? {
            let name = path.file_name().unwrap().to_string_lossy();
            println!("   {} {}", "✓".green(), name);
        }
    }
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let guards_dir = Path::new(GUARDS_DIR);
    if !guards_dir.exists() {
        return Ok(());
    }

    let mut count = 0;
    for entry in fs::read_dir(guards_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_airis_guard(&path)? {
            fs::remove_file(path)?;
            count += 1;
        }
    }

    if guards_dir.read_dir()?.next().is_none() {
        fs::remove_dir(guards_dir)?;
    }

    println!("{} Removed {} local guard(s)", "✓".green(), count);
    Ok(())
}

pub fn check_allow(cmd: &str) -> Result<bool> {
    let manifest_path = Path::new("manifest.toml");
    if !manifest_path.exists() {
        return Ok(false);
    }

    let manifest = Manifest::load(manifest_path)?;
    
    // Check local allow list
    if manifest.guards.allow.iter().any(|c| c == cmd) {
        return Ok(true);
    }

    // Check if the command is not even in the deny/wrap lists
    let is_guarded = manifest.guards.deny.contains(&cmd.to_string()) || 
                    manifest.guards.wrap.contains_key(cmd);

    // Also check global config if not explicitly guarded locally
    if !is_guarded {
        let global_config = GlobalConfig::load()?;
        if global_config.guards.get_level(cmd) == GuardLevel::Off {
            return Ok(true);
        }
    }

    Ok(false)
}

pub fn check_docker() -> Result<()> {
    if is_inside_docker() {
        println!("{} Running inside Docker", "✓".green());
    } else {
        println!("{} Running on host", "✗".yellow());
    }
    Ok(())
}

fn is_inside_docker() -> bool {
    Path::new("/.dockerenv").exists() || 
    fs::read_to_string("/proc/1/cgroup").map(|s| s.contains("docker")).unwrap_or(false)
}
