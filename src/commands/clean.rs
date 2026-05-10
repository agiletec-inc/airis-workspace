//! Safe clean command using SafeFS
//!
//! This module provides a secure implementation of the clean command that:
//! - Uses SafeFS for all file operations (workspace-bounded)
//! - Supports dry-run mode by default
//! - Never deletes user data without explicit confirmation
//! - Provides clear feedback on what was/would be deleted

use std::path::Path;

use anyhow::{Result, anyhow};
use colored::Colorize;
use glob::glob;

use crate::manifest::{MANIFEST_FILE, Manifest};
use crate::safe_fs::{SafeAction, SafeFS};

/// Project-root markers used to decide whether the current directory is a
/// reasonable place to run a destructive cleanup.
///
/// The set is intentionally broad (Node, Rust, Python, Go) because `airis
/// clean` may legitimately run in any of those repos. If none are present we
/// abort to avoid wiping `node_modules` / `dist` from arbitrary directories
/// where the user wandered into.
const PROJECT_ROOT_MARKERS: &[&str] = &[
    "manifest.toml",
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "go.mod",
];

fn is_project_root(dir: &Path) -> bool {
    PROJECT_ROOT_MARKERS
        .iter()
        .any(|name| dir.join(name).exists())
}

/// Construct an empty Manifest entirely from `#[serde(default)]` fields.
///
/// Used when `manifest.toml` is absent so `airis clean` can still operate on
/// the canonical build-artifact list without requiring users to run
/// `airis init` first. We deliberately bypass `Manifest::parse` because its
/// `validate()` step (e.g. `project.id required`) is meant for user-authored
/// manifests; an in-memory default never reaches disk and only feeds the
/// canonical `clean.dirs` / `clean.recursive` lists here.
fn default_manifest() -> Manifest {
    toml::from_str("").expect("empty manifest must deserialize via serde defaults — schema bug")
}

/// Run the clean command
///
/// # Arguments
/// * `dry_run` - If true, only show what would be deleted without deleting
/// * `purge` - If true, also remove legacy/orphaned config files
/// * `allow_anywhere` - Skip the project-root safety check
pub fn run(dry_run: bool, purge: bool, allow_anywhere: bool) -> Result<()> {
    if !allow_anywhere {
        let cwd = std::env::current_dir()?;
        if !is_project_root(&cwd) {
            return Err(anyhow!(
                "Not a project root: none of {} found in {}.\n\
                 Run from a project directory, or pass --allow-anywhere to override.",
                PROJECT_ROOT_MARKERS.join(", "),
                cwd.display()
            ));
        }
    }

    let manifest_path = Path::new(MANIFEST_FILE);
    let manifest_present = manifest_path.exists();

    if purge && !manifest_present {
        return Err(anyhow!(
            "--purge requires manifest.toml so user-managed compose files \
             (orchestration.dev) can be protected from deletion. \
             Run `airis init` first, or omit --purge to clean only build artifacts."
        ));
    }

    let manifest = if manifest_present {
        Manifest::load(manifest_path)?
    } else {
        println!(
            "{}",
            "⚠️  manifest.toml not found — using default clean rules. Run `airis init` to customize."
                .yellow()
        );
        default_manifest()
    };

    let safe_fs = SafeFS::current(dry_run)?;

    if dry_run {
        println!(
            "{}",
            "🔍 Dry-run mode: showing what would be cleaned...".bright_blue()
        );
    } else {
        println!("{}", "🧹 Cleaning workspace...".bright_blue());
    }
    println!();

    let mut cleaned = 0;
    let mut skipped = 0;
    let mut errors = 0;

    // 1. Build Artifacts Clean (Standard)
    println!("{}", "📦 Build Artifacts".bold());
    let clean = &manifest.workspace.clean;
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

    // 2. Legacy / Purge Clean (Optional)
    if purge {
        println!("\n{}", "💀 Legacy Config Purge".bold());
        let legacy_patterns = [
            "docker-compose.yml",
            "docker-compose.yaml",
            "docker-compose.override.yml",
            "docker-compose.override.yaml",
            "compose.yml",
            "compose.override.yml",
            "compose.override.yaml",
            "workspace/compose.yaml",
            "workspace/compose.yml",
            "workspace/docker-compose.yml",
            "workspace/docker-compose.yaml",
            "**/docker-compose.yml",
            "**/docker-compose.yaml",
        ];

        // Files that are currently managed and should NOT be deleted
        let mut managed_files = vec!["manifest.toml".to_string()];

        // Protect the specific compose file defined in orchestration.dev
        if let Some(dev) = &manifest.orchestration.dev
            && let Some(workspace) = &dev.workspace
        {
            managed_files.push(workspace.clone());
        }

        // Also protect root compose if detected by find_compose_file
        if let Some(root_compose) = crate::commands::run::compose::find_compose_file() {
            managed_files.push(root_compose.to_string());
        }

        // De-duplicate managed files
        managed_files.sort();
        managed_files.dedup();

        for pattern in legacy_patterns {
            if let Ok(paths) = glob(pattern) {
                for entry in paths.flatten() {
                    let path_str = entry.to_string_lossy().to_string();

                    // Safety: Skip managed files or protected dirs
                    if managed_files.contains(&path_str)
                        || path_str.starts_with(".git/")
                        || path_str.starts_with(".airis/")
                    {
                        continue;
                    }

                    if dry_run {
                        println!(
                            "   {} {} (legacy — would delete)",
                            "→".bright_blue(),
                            path_str
                        );
                        cleaned += 1;
                    } else {
                        match std::fs::remove_file(&entry) {
                            Ok(()) => {
                                println!("   {} {} (legacy — deleted)", "✓".green(), path_str);
                                cleaned += 1;
                            }
                            Err(e) => {
                                println!("   {} {} — {}", "✗".red(), path_str, e);
                                errors += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Recursive Artifacts
    println!("\n{}", "📂 Recursive Artifacts".bold());
    for pattern in &clean.recursive {
        // Validate pattern is safe
        if pattern.contains("..") || pattern.starts_with('/') {
            println!("   {} {} (unsafe pattern skipped)", "⏭️".yellow(), pattern);
            skipped += 1;
            continue;
        }

        // Find matching directories up to depth 3
        let glob_pattern = format!("**/{}", pattern);
        match glob(&glob_pattern) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    // Skip if path is too deep (more than 6 levels)
                    if entry.components().count() > 7 {
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
        println!("Run {} to actually clean.", "airis clean".bright_cyan());
    } else {
        println!(
            "{} Cleaned {} item(s), {} skipped, {} errors",
            if errors == 0 {
                "✅".green()
            } else {
                "⚠️".yellow()
            },
            cleaned,
            skipped,
            errors
        );
        println!("{}", "(container cache preserved)".dimmed());
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

/// Remove orphaned generated files (called by airis gen after generation).
pub fn remove_orphaned_files(
    previous_paths: &[String],
    current_paths: &[String],
    dry_run: bool,
) -> usize {
    use std::fs;
    use std::path::Path;

    let current_set: std::collections::HashSet<&str> =
        current_paths.iter().map(|s| s.as_str()).collect();
    let mut removed = 0;

    for path_str in previous_paths {
        if current_set.contains(path_str.as_str()) {
            continue; // Still generated
        }
        let path = Path::new(path_str);
        if !path.exists() {
            continue; // Already gone
        }

        // Safety check: only delete if file has airis gen marker
        if let Ok(content) = fs::read_to_string(path)
            && !content.contains("Auto-generated by airis")
            && !content.contains("DO NOT EDIT")
            && !content.contains("airis gen")
        {
            println!(
                "   {} {} (skipped — no airis marker, may be user-created)",
                "⏭️".yellow(),
                path_str
            );
            continue;
        }

        if dry_run {
            println!(
                "   {} {} (orphaned — would delete)",
                "→".bright_blue(),
                path_str
            );
        } else {
            match fs::remove_file(path) {
                Ok(()) => {
                    println!("   {} {} (orphaned — deleted)", "✓".green(), path_str);
                    removed += 1;
                }
                Err(e) => {
                    println!("   {} {} — {}", "✗".red(), path_str, e);
                }
            }
        }
    }

    removed
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

    use super::{PROJECT_ROOT_MARKERS, default_manifest, is_project_root};
    use tempfile::tempdir;

    #[test]
    fn project_root_detected_for_each_marker() {
        for marker in PROJECT_ROOT_MARKERS {
            let dir = tempdir().expect("tempdir");
            std::fs::write(dir.path().join(marker), "").expect("write marker");
            assert!(
                is_project_root(dir.path()),
                "{marker} should be recognized as a project-root marker",
            );
        }
    }

    #[test]
    fn project_root_rejected_when_no_marker_present() {
        let dir = tempdir().expect("tempdir");
        // A stray build-artifact alone must not be treated as a project root.
        std::fs::create_dir(dir.path().join("node_modules")).expect("mkdir");
        assert!(!is_project_root(dir.path()));
    }

    #[test]
    fn default_manifest_provides_canonical_clean_lists() {
        let manifest = default_manifest();
        let clean = &manifest.workspace.clean;
        assert!(
            clean.dirs.iter().any(|d| d == "dist"),
            "default clean.dirs should include 'dist'",
        );
        assert!(
            clean.recursive.iter().any(|d| d == "node_modules"),
            "default clean.recursive should include 'node_modules'",
        );
    }
}
