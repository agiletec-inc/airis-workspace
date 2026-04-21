//! (DEPRECATED) Local command guards.
//! Modern airis uses global shims in ~/.airis/bin.

use anyhow::Result;
use colored::Colorize;

pub fn install() -> Result<()> {
    println!("{}", "⚠️  Local guards are deprecated.".yellow());
    println!("   Use {} instead.", "airis guards install --global".bright_cyan());
    Ok(())
}

pub fn status() -> Result<()> {
    println!("Local guards are no longer recommended. Use 'airis guards status --global'.");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    crate::commands::workspace::uninstall()
}

pub fn check_allow(_cmd: &str) -> Result<bool> {
    // Legacy allow-check: always allow for now to avoid breaking existing flows 
    // before users migrate to global shims.
    Ok(true)
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
    std::path::Path::new("/.dockerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .map(|s| s.contains("docker"))
            .unwrap_or(false)
}
