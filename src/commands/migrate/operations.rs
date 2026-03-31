//! Directory creation, file move, and backup operations

use anyhow::{Context, Result, bail};
use chrono::Local;
use colored::Colorize;
use std::fs;
use std::path::Path;

use crate::commands::discover::DiscoveryResult;

use super::MigrationReport;
use super::manifest_gen::generate_manifest_content;

/// Execute directory creation
pub(super) fn execute_create_directory(
    dir: &Path,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<()> {
    let path_str = dir.display().to_string();

    if dir.exists() {
        let msg = format!("Directory already exists: {}", path_str);
        println!("   {} {}", "⏭️".yellow(), msg);
        report.skipped.push(msg);
        return Ok(());
    }

    if dry_run {
        println!(
            "   {} Would create directory: {}",
            "→".bright_blue(),
            path_str
        );
        report.completed.push(format!("Would create: {}", path_str));
    } else {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", path_str))?;
        println!("   {} Created directory: {}", "✓".green(), path_str);
        report.completed.push(format!("Created: {}", path_str));
    }

    Ok(())
}

/// Execute file move with backup
pub(super) fn execute_move_file(
    from_path: &Path,
    to_path: &Path,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<()> {
    let from_str = from_path.display().to_string();
    let to_str = to_path.display().to_string();

    // Check source exists
    if !from_path.exists() {
        let msg = format!("Source file not found: {}", from_str);
        println!("   {} {}", "⏭️".yellow(), msg);
        report.skipped.push(msg);
        return Ok(());
    }

    // Check target doesn't exist
    if to_path.exists() {
        let msg = format!("Target already exists, skipping: {}", to_str);
        println!("   {} {}", "⚠️".yellow(), msg);
        report.skipped.push(msg);
        return Ok(());
    }

    if dry_run {
        println!(
            "   {} Would move: {} → {}",
            "→".bright_blue(),
            from_str,
            to_str
        );
        report
            .completed
            .push(format!("Would move: {} → {}", from_str, to_str));
    } else {
        // Create backup before move
        let backup_path = create_backup(from_path)?;
        println!(
            "   {} Backup created: {}",
            "📦".dimmed(),
            backup_path.display()
        );

        // Ensure target directory exists
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Move the file (rename if on same filesystem, copy+delete otherwise)
        if fs::rename(from_path, to_path).is_err() {
            // Cross-filesystem move: copy then delete
            fs::copy(from_path, to_path)?;
            fs::remove_file(from_path)?;
        }

        println!("   {} Moved: {} → {}", "✓".green(), from_str, to_str);
        report
            .completed
            .push(format!("Moved: {} → {}", from_str, to_str));
    }

    Ok(())
}

/// Create a backup of a file
fn create_backup(path: &Path) -> Result<std::path::PathBuf> {
    let backup_dir = Path::new(".airis/backups");
    fs::create_dir_all(backup_dir)?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let backup_name = format!("{}.{}.bak", filename, timestamp);
    let backup_path = backup_dir.join(backup_name);

    fs::copy(path, &backup_path)?;
    Ok(backup_path)
}

/// Generate manifest.toml from discovery results
pub(super) fn execute_generate_manifest(
    discovery: &DiscoveryResult,
    dry_run: bool,
    base_dir: &Path,
    report: &mut MigrationReport,
) -> Result<()> {
    let manifest_path = base_dir.join("manifest.toml");

    // CRITICAL: Never overwrite existing manifest.toml
    if manifest_path.exists() {
        bail!("manifest.toml already exists. This should not happen in migration flow.");
    }

    let content = generate_manifest_content(discovery)?;

    if dry_run {
        println!("   {} Would generate manifest.toml:", "→".bright_blue());
        println!();
        // Show preview (first 30 lines)
        for line in content.lines().take(30) {
            println!("   {}", line.dimmed());
        }
        if content.lines().count() > 30 {
            println!("   {}", "... (truncated)".dimmed());
        }
        println!();
        report
            .completed
            .push("Would generate: manifest.toml".to_string());
    } else {
        fs::write(&manifest_path, &content)?;
        println!("   {} Generated manifest.toml", "✓".green());
        report
            .completed
            .push("Generated: manifest.toml".to_string());
    }

    Ok(())
}
