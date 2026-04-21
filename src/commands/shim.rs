//! (DEPRECATED) Project-local shim management. 
//! Use 'airis guards install --global' for the modern smart-shim architecture.

use anyhow::Result;
use colored::Colorize;

pub fn install() -> Result<()> {
    println!("{}", "⚠️  'airis shim install' is deprecated.".yellow());
    println!("   Use {} instead for a cleaner setup.", "airis guards install --global".bright_cyan());
    println!("   This project's shims will now be managed globally in ~/.airis/bin.");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    crate::commands::workspace::uninstall()
}

pub fn list() -> Result<()> {
    println!("Use 'airis guards status --global' to see active shims.");
    Ok(())
}

/// Execute a command through the smart proxy logic
pub fn exec(cmd: &str, args: &[String]) -> Result<()> {
    // This is still used by the smart-shim to actually run the command in Docker.
    // It remains the core execution engine for Docker-First.
    crate::commands::run::run(cmd, args)
}
