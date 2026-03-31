//! Dependency architecture validation: ensure apps only depend on libs

use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

/// Validate dependency architecture rules
/// Checks that apps only depend on libs (public API), and no cross-app dependencies exist
pub fn validate_dependencies() -> Result<()> {
    validate_dependencies_impl(false)
}

pub fn validate_dependencies_impl(quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "{}",
            "🔍 Validating dependency architecture...".bright_blue()
        );
    }

    // Check if dependency-cruiser config exists
    let config_path = Path::new("tools/dependency-cruiser.cjs");
    if !config_path.exists() {
        if !quiet {
            println!(
                "  {} dependency-cruiser config not found, skipping",
                "⏭️".yellow()
            );
        }
        return Ok(());
    }

    // Check if dependency-cruiser is installed
    let check = Command::new("npx")
        .args(["dependency-cruiser", "--version"])
        .output();

    if check.is_err() {
        if !quiet {
            println!(
                "  {} dependency-cruiser not installed, skipping",
                "⏭️".yellow()
            );
            println!(
                "  {} Install with: pnpm add -D dependency-cruiser",
                "💡".dimmed()
            );
        }
        return Ok(());
    }

    // Run dependency-cruiser
    if !quiet {
        println!("  {} Running dependency-cruiser...", "⚙️".dimmed());
    }
    let output = Command::new("npx")
        .args([
            "dependency-cruiser",
            "--config",
            "tools/dependency-cruiser.cjs",
            "--output-type",
            "err",
            "apps",
            "libs",
        ])
        .output()
        .context("Failed to run dependency-cruiser")?;

    if !output.status.success() {
        bail!("Dependency architecture validation failed. Fix violations above.");
    }

    if !quiet {
        println!("  {} No architecture violations found", "✅".green());
    }
    Ok(())
}
