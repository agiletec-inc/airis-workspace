//! Doctor command: diagnose and heal workspace configuration issues
//!
//! Detects drift between manifest.toml and generated files,
//! then automatically repairs them.

use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::fs;
use std::path::Path;

use crate::commands::snapshot::{compare_with_snapshots, DiffType, Snapshots};
use crate::commands::sync_deps::resolve_version;
use crate::manifest::{CatalogEntry, Manifest, MANIFEST_FILE};
use crate::templates::TemplateEngine;

/// Issue severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Warning,
    Error,
}

/// A detected issue in the workspace
#[derive(Debug)]
pub struct Issue {
    pub file: String,
    pub description: String,
    pub severity: Severity,
}

/// Run the doctor command
pub fn run(fix: bool) -> Result<()> {
    println!("{}", "🔍 Diagnosing workspace health...".bright_blue());
    println!();

    // Check if manifest.toml exists
    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        println!("{}", "❌ manifest.toml not found".red());
        println!("   Run `airis init` to create one.");
        return Ok(());
    }

    // Load manifest
    let manifest = Manifest::load(manifest_path)
        .context("Failed to load manifest.toml")?;

    // Collect issues
    let mut issues: Vec<Issue> = Vec::new();

    // Check snapshots if available
    check_snapshots(&mut issues)?;

    // Check each generated file
    check_generated_files(&manifest, &mut issues)?;

    // Report results
    if issues.is_empty() {
        println!("{}", "✅ Workspace is healthy!".green());
        println!("   All generated files are in sync with manifest.toml");
        return Ok(());
    }

    // Report issues
    println!("{}", "⚠️  Detected inconsistencies:".yellow());
    for issue in &issues {
        let icon = match issue.severity {
            Severity::Error => "❌",
            Severity::Warning => "⚠️ ",
        };
        println!("   {} {} - {}", icon, issue.file, issue.description);
    }
    println!();

    if fix {
        // Auto-fix by regenerating
        println!("{}", "🔧 Fixing...".bright_blue());
        println!();

        crate::commands::generate::sync_from_manifest(&manifest)?;

        println!();
        println!("{}", "✨ Workspace healed successfully!".green().bold());
    } else {
        println!("{}", "💡 Run `airis doctor --fix` to auto-repair".bright_yellow());
    }

    Ok(())
}

/// Check snapshots for differences from current state
fn check_snapshots(issues: &mut Vec<Issue>) -> Result<()> {
    // Load snapshots if available
    let snapshots = match Snapshots::load()? {
        Some(s) => s,
        None => {
            // No snapshots - that's fine, skip this check
            return Ok(());
        }
    };

    if snapshots.snapshot.is_empty() {
        return Ok(());
    }

    println!("{}", "📸 Checking snapshots...".bright_blue());

    // Compare with current state
    let diffs = compare_with_snapshots(&snapshots)?;

    for diff in diffs {
        let (description, severity) = match diff.diff_type {
            DiffType::Modified => (
                format!("Changed since snapshot: {}", diff.details),
                Severity::Warning,
            ),
            DiffType::Deleted => (
                "File deleted since snapshot".to_string(),
                Severity::Error,
            ),
        };

        issues.push(Issue {
            file: diff.path,
            description,
            severity,
        });
    }

    println!();
    Ok(())
}

/// Check all generated files for drift
fn check_generated_files(manifest: &Manifest, issues: &mut Vec<Issue>) -> Result<()> {
    let engine = TemplateEngine::new()?;

    // Resolve catalog versions for comparison
    let resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;

    // Check package.json
    check_file(
        "package.json",
        || engine.render_package_json(manifest),
        issues,
    )?;

    // Check pnpm-workspace.yaml
    check_file(
        "pnpm-workspace.yaml",
        || engine.render_pnpm_workspace(manifest, &resolved_catalog),
        issues,
    )?;

    // Check docker-compose.yml
    check_file(
        "docker-compose.yml",
        || engine.render_docker_compose(manifest),
        issues,
    )?;

    // Check GitHub workflows if CI is enabled
    if manifest.ci.enabled {
        check_file(
            ".github/workflows/ci.yml",
            || engine.render_ci_yml(manifest),
            issues,
        )?;

        check_file(
            ".github/workflows/release.yml",
            || engine.render_release_yml(manifest),
            issues,
        )?;
    }

    Ok(())
}

/// Check a single file for drift
fn check_file<F>(filename: &str, generate: F, issues: &mut Vec<Issue>) -> Result<()>
where
    F: FnOnce() -> Result<String>,
{
    let path = Path::new(filename);

    if !path.exists() {
        issues.push(Issue {
            file: filename.to_string(),
            description: "Missing (will be created)".to_string(),
            severity: Severity::Error,
        });
        return Ok(());
    }

    // Read current file
    let current = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", filename))?;

    // Generate expected content
    let expected = generate()?;

    // Compare (normalize line endings)
    let current_normalized = current.replace("\r\n", "\n");
    let expected_normalized = expected.replace("\r\n", "\n");

    if current_normalized != expected_normalized {
        // Count differences for more helpful message
        let current_lines: Vec<&str> = current_normalized.lines().collect();
        let expected_lines: Vec<&str> = expected_normalized.lines().collect();

        let diff_count = current_lines
            .iter()
            .zip(expected_lines.iter())
            .filter(|(a, b)| a != b)
            .count();

        let line_diff = (current_lines.len() as i32 - expected_lines.len() as i32).abs();

        let description = if line_diff > 0 {
            format!("Content mismatch ({} lines differ, {} lines added/removed)", diff_count, line_diff)
        } else {
            format!("Content mismatch ({} lines differ)", diff_count.max(1))
        };

        issues.push(Issue {
            file: filename.to_string(),
            description,
            severity: Severity::Error,
        });
    }

    Ok(())
}

/// Resolve catalog version policies (copied from generate.rs to avoid circular deps)
fn resolve_catalog_versions(
    catalog: &IndexMap<String, CatalogEntry>,
) -> Result<IndexMap<String, String>> {
    if catalog.is_empty() {
        return Ok(IndexMap::new());
    }

    let mut resolved: IndexMap<String, String> = IndexMap::new();

    for (package, entry) in catalog {
        let version = match entry {
            CatalogEntry::Policy(policy) => {
                let policy_str = policy.as_str();
                resolve_version(package, policy_str)?
            }
            CatalogEntry::Version(version) => version.clone(),
            CatalogEntry::Follow(follow_config) => {
                let target = &follow_config.follow;
                if let Some(target_version) = resolved.get(target) {
                    target_version.clone()
                } else {
                    anyhow::bail!(
                        "Cannot resolve '{}': follow target '{}' not found",
                        package,
                        target
                    );
                }
            }
        };

        resolved.insert(package.clone(), version);
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_issue_severity() {
        let issue = Issue {
            file: "test.txt".to_string(),
            description: "Test issue".to_string(),
            severity: Severity::Error,
        };
        assert_eq!(issue.severity, Severity::Error);
    }
}
