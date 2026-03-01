//! Safe clean command using SafeFS
//!
//! This module provides a secure implementation of the clean command that:
//! - Uses SafeFS for all file operations (workspace-bounded)
//! - Supports dry-run mode by default
//! - Never deletes user data without explicit confirmation
//! - Provides clear feedback on what was/would be deleted

use anyhow::{Context, Result};
use colored::Colorize;
use glob::glob;

use crate::manifest::{Manifest, MANIFEST_FILE};
use crate::safe_fs::{SafeAction, SafeFS};

/// Run the clean command
///
/// # Arguments
/// * `dry_run` - If true, only show what would be deleted without deleting
pub fn run(dry_run: bool) -> Result<()> {
    let manifest = Manifest::load(MANIFEST_FILE)
        .with_context(|| "Failed to load manifest.toml. Run 'airis init' first.")?;

    let safe_fs = SafeFS::current(dry_run)?;

    if dry_run {
        println!("{}", "🔍 Dry-run mode: showing what would be cleaned...".bright_blue());
    } else {
        println!("{}", "🧹 Cleaning host build artifacts...".bright_blue());
    }
    println!();

    let clean = &manifest.workspace.clean;
    let mut cleaned = 0;
    let mut skipped = 0;
    let mut errors = 0;

    // Clean root directories specified in manifest
    for dir in &clean.dirs {
        match safe_fs.clean_artifact(dir) {
            Ok(result) => {
                print_result(&result.action, dir, &mut cleaned, &mut skipped);
            }
            Err(e) => {
                println!("   {} {} - {}", "✗".red(), dir, e);
                errors += 1;
            }
        }
    }

    // Clean recursive patterns (e.g., node_modules in subdirectories)
    for pattern in &clean.recursive {
        // Validate pattern is safe
        if pattern.contains("..") || pattern.starts_with('/') {
            println!(
                "   {} {} (unsafe pattern skipped)",
                "⏭️".yellow(),
                pattern
            );
            skipped += 1;
            continue;
        }

        // Find matching directories up to depth 3
        let glob_pattern = format!("**/{}",pattern);
        match glob(&glob_pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    // Skip if path is too deep (more than 3 levels)
                    if entry.components().count() > 4 {
                        continue;
                    }

                    // Skip protected directories
                    let path_str = entry.to_string_lossy();
                    if path_str.starts_with("supabase/")
                        || path_str.starts_with("infra/")
                        || path_str.starts_with(".git/")
                        || path_str.starts_with(".airis/")
                    {
                        continue;
                    }

                    match safe_fs.clean_artifact(&entry) {
                        Ok(result) => {
                            print_result(
                                &result.action,
                                &entry.to_string_lossy(),
                                &mut cleaned,
                                &mut skipped,
                            );
                        }
                        Err(e) => {
                            println!("   {} {} - {}", "✗".red(), entry.display(), e);
                            errors += 1;
                        }
                    }
                }
            }
            Err(e) => {
                println!("   {} {} - {}", "✗".red(), pattern, e);
                errors += 1;
            }
        }
    }

    // Clean .DS_Store files (macOS artifacts)
    if let Ok(paths) = glob("**/.DS_Store") {
        for entry in paths.flatten() {
            if entry.components().count() <= 5 {
                match safe_fs.clean_artifact(&entry) {
                    Ok(result) => {
                        if matches!(result.action, SafeAction::Deleted | SafeAction::WouldDelete) {
                            // Don't count .DS_Store in main stats, just clean silently
                        }
                    }
                    Err(_) => {
                        // Ignore errors for .DS_Store
                    }
                }
            }
        }
    }

    println!();

    if dry_run {
        println!(
            "{} Would clean {} item(s), {} skipped",
            "📋".cyan(),
            cleaned,
            skipped
        );
        println!();
        println!(
            "Run {} to actually clean.",
            "airis clean".bright_cyan()
        );
    } else {
        println!(
            "{} Cleaned {} item(s), {} skipped, {} errors",
            if errors == 0 { "✅".green() } else { "⚠️".yellow() },
            cleaned,
            skipped,
            errors
        );
        println!(
            "{}",
            "(container cache preserved)".dimmed()
        );
    }

    if errors > 0 {
        println!();
        println!(
            "{}",
            "Some items could not be cleaned. Check permissions or if files are in use.".yellow()
        );
    }

    Ok(())
}

fn print_result(action: &SafeAction, path: &str, cleaned: &mut usize, skipped: &mut usize) {
    match action {
        SafeAction::Deleted => {
            println!("   {} {}", "✓".green(), path);
            *cleaned += 1;
        }
        SafeAction::WouldDelete => {
            println!("   {} {} (would delete)", "→".bright_blue(), path);
            *cleaned += 1;
        }
        SafeAction::Skipped(reason) => {
            println!("   {} {} ({})", "⏭️".yellow(), path, reason);
            *skipped += 1;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Check if a path should be protected from cleaning (test helper)
    fn is_protected_path(path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        // Never clean these
        let protected = [
            "manifest.toml",
            "package.json",
            "pnpm-lock.yaml",
            "Cargo.toml",
            "Cargo.lock",
            ".git",
            ".env",
            ".envrc",
            "src",
            "apps",
            "libs",
            "supabase",
            "infra",
        ];

        for p in protected {
            if path_str == p || path_str.starts_with(&format!("{}/", p)) {
                return true;
            }
        }

        false
    }

    #[test]
    fn test_protected_paths() {
        assert!(is_protected_path(Path::new("manifest.toml")));
        assert!(is_protected_path(Path::new("src/main.rs")));
        assert!(is_protected_path(Path::new(".git/config")));
        assert!(is_protected_path(Path::new("apps/dashboard")));

        assert!(!is_protected_path(Path::new("node_modules")));
        assert!(!is_protected_path(Path::new(".next")));
        assert!(!is_protected_path(Path::new("dist")));
    }
}
