//! Doctor command: diagnose and heal workspace configuration issues
//!
//! Detects drift between manifest.toml and generated files,
//! then automatically repairs them.
//!
//! Also provides "truth" output for LLM consumption via --truth and --truth-json flags.

use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use std::fs;
use std::path::Path;

use crate::commands::manifest_cmd::WorkspaceTruth;
use crate::manifest::{CatalogEntry, MANIFEST_FILE, Manifest};
use crate::ownership::{Ownership, get_ownership};
use crate::templates::TemplateEngine;
use crate::version_resolver::resolve_version;

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

/// Run the doctor --truth command
///
/// Outputs workspace startup truth for LLM consumption.
/// Uses manifest_cmd::WorkspaceTruth internally (single source of truth).
pub fn run_truth(json_output: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        anyhow::bail!(
            "manifest.toml not found.\n\n\
             Hint: Create one (see docs/manifest.md) or ask Claude Code via /airis:init.\n\
             This command requires an airis workspace."
        );
    }

    let manifest = Manifest::load(manifest_path).context("Failed to load manifest.toml")?;

    let truth = WorkspaceTruth::from_manifest(&manifest)?;

    if json_output {
        // JSON output for automation/LLM
        println!("{}", truth.to_json()?);
    } else {
        // Human-readable output
        println!("{}", "📋 Startup Truth".bright_blue().bold());
        println!("{}", "━".repeat(44).dimmed());
        println!();
        println!("{:<12} {}", "Root:".bright_yellow(), truth.workspace_root);
        println!(
            "{:<12} {}",
            "Compose:".bright_yellow(),
            truth.compose_command
        );
        println!("{:<12} {}", "Service:".bright_yellow(), truth.service);
        println!("{:<12} {}", "Workdir:".bright_yellow(), truth.workdir);
        println!("{:<12} {}", "PM:".bright_yellow(), truth.package_manager);
        println!();
        println!("{}", "Commands:".bright_yellow());
        for (name, cmd) in &truth.recommended_commands {
            println!("  {:<10} {}", format!("{}:", name), cmd.cyan());
        }
        println!();
        println!("{}", "━".repeat(44).dimmed());
        println!(
            "{}",
            "Use `airis doctor --truth-json` for machine-readable output.".dimmed()
        );
    }

    Ok(())
}

/// Run the doctor command
pub fn run(fix: bool) -> Result<()> {
    println!("{}", "🔍 Diagnosing workspace health...".bright_blue());
    println!();

    // Check if manifest.toml exists
    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        println!("{}", "❌ manifest.toml not found".red());
        println!("   Create one (see docs/manifest.md) or ask Claude Code via /airis:init.");
        return Ok(());
    }

    // Load manifest
    let manifest = Manifest::load(manifest_path).context("Failed to load manifest.toml")?;

    // Collect issues
    let mut issues: Vec<Issue> = Vec::new();

    // Check each generated file
    check_generated_files(&manifest, &mut issues)?;

    // Check for command guards
    check_command_guards(&manifest, &mut issues)?;

    // Check for orphaned packages (not in manifest)
    check_orphaned_packages(&manifest, &mut issues)?;

    // Check for leaked host artifacts (node_modules, .pnpm, build outputs, etc.)
    check_host_artifacts(&mut issues)?;

    // Report results
    if issues.is_empty() {
        println!("{}", "✅ Workspace is healthy!".green());
        println!("   All generated files are in sync and host is clean.");
        return Ok(());
    }

    // Report issues
    println!("{}", "⚠️  Detected workspace issues:".yellow());
    for issue in &issues {
        let icon = match issue.severity {
            Severity::Error => "❌",
            Severity::Warning => "⚠️ ",
        };
        println!("   {} {} - {}", icon, issue.file.bold(), issue.description);
    }
    println!();

    if fix {
        // Auto-fix detected issues
        println!("{}", "🔧 Healing workspace...".bright_blue());
        println!();

        // 1. Regenerate files
        crate::commands::generate::sync_from_manifest(&manifest)?;

        // 2. Install guards if missing
        for issue in &issues {
            if issue.file == "guards" {
                println!("   {} Installing command guards...", "→".dimmed());
                crate::commands::guards::install()?;
            }
        }

        // 3. Remove host artifacts (physical enforcement)
        for issue in &issues {
            if issue.description.contains("leaked from container") {
                let path = Path::new(&issue.file);
                if path.exists() {
                    println!(
                        "   {} Removing host artifact: {}...",
                        "→".dimmed(),
                        issue.file
                    );
                    if path.is_dir() {
                        let _ = fs::remove_dir_all(path);
                    } else {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }

        println!();
        println!("{}", "✨ Workspace healed successfully!".green().bold());
    } else {
        println!(
            "{}",
            "💡 Run `airis doctor --fix` to auto-repair issues and enforce boundaries."
                .bright_yellow()
        );
    }

    Ok(())
}

/// Check if command guards are installed
fn check_command_guards(manifest: &Manifest, issues: &mut Vec<Issue>) -> Result<()> {
    let guards_dir = Path::new(".airis/bin");

    // Check if guards directory exists
    if !guards_dir.exists() {
        issues.push(Issue {
            file: "guards".to_string(),
            description: "Command guards are not installed. Host commands are unprotected."
                .to_string(),
            severity: Severity::Error,
        });
        return Ok(());
    }

    // Check if expected deny guards exist
    for cmd in &manifest.guards.deny {
        let guard_path = guards_dir.join(cmd);
        if !guard_path.exists() {
            issues.push(Issue {
                file: "guards".to_string(),
                description: format!("Guard for `{}` is missing.", cmd),
                severity: Severity::Warning,
            });
        }
    }

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
        || engine.render_package_json(manifest, &resolved_catalog),
        issues,
    )?;

    // Check pnpm-workspace.yaml (minimal file for pnpm compatibility)
    if !manifest.packages.workspaces.is_empty() {
        check_file(
            "pnpm-workspace.yaml",
            || engine.render_pnpm_workspace(manifest),
            issues,
        )?;
    }

    // compose.yml and CI/CD workflows are project-owned — not checked by airis doctor

    Ok(())
}

/// Check a single file for drift
fn check_file<F>(filename: &str, generate: F, issues: &mut Vec<Issue>) -> Result<()>
where
    F: FnOnce() -> Result<String>,
{
    let path = Path::new(filename);
    let ownership = get_ownership(path);

    if !path.exists() {
        // Only report missing for tool-owned files
        if matches!(ownership, Ownership::Tool) {
            issues.push(Issue {
                file: filename.to_string(),
                description: "Missing (will be created)".to_string(),
                severity: Severity::Error,
            });
        }
        return Ok(());
    }

    // Read current file
    let current =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", filename))?;

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
            format!(
                "Content mismatch ({} lines differ, {} lines added/removed)",
                diff_count, line_diff
            )
        } else {
            format!("Content mismatch ({} lines differ)", diff_count.max(1))
        };

        // Severity depends on ownership
        let severity = match ownership {
            Ownership::Tool => Severity::Error,   // Tool files must match
            Ownership::User => Severity::Warning, // User files are their responsibility
        };

        issues.push(Issue {
            file: filename.to_string(),
            description,
            severity,
        });
    }

    Ok(())
}

/// Check for orphaned packages (exist on disk but not in manifest)
fn check_orphaned_packages(manifest: &Manifest, issues: &mut Vec<Issue>) -> Result<()> {
    // Get declared apps from manifest.apps keys
    let declared_apps: std::collections::HashSet<String> = manifest.apps.keys().cloned().collect();

    // Check apps directory
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        for entry in fs::read_dir(apps_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let app_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Check if this app has a package.json but isn't in manifest
                let pkg_json = path.join("package.json");
                if pkg_json.exists() && !declared_apps.contains(app_name) {
                    issues.push(Issue {
                        file: format!("apps/{}", app_name),
                        description: "Not declared in manifest.toml [dev.apps]".to_string(),
                        severity: Severity::Warning,
                    });
                }
            }
        }
    }

    // Get declared libs from manifest
    let declared_libs: std::collections::HashSet<String> = manifest.libs.keys().cloned().collect();

    // Check libs directory
    let libs_dir = Path::new("libs");
    if libs_dir.exists() {
        for entry in fs::read_dir(libs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let lib_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Check if this lib has a package.json but isn't in manifest
                let pkg_json = path.join("package.json");
                if pkg_json.exists() && !declared_libs.contains(lib_name) {
                    issues.push(Issue {
                        file: format!("libs/{}", lib_name),
                        description: "Not declared in manifest.toml [libs]".to_string(),
                        severity: Severity::Warning,
                    });
                }
            }
        }
    }

    Ok(())
}

/// Determine severity for a host artifact based on its name.
///
/// Dependency directories (`node_modules`, `.pnpm`) are errors because they
/// indicate the bind mount is leaking container-installed packages.
/// Build outputs (`.turbo`, `.next`, `dist`, `build`, `coverage`) are warnings.
fn artifact_severity(name: &str) -> Severity {
    match name {
        "node_modules" | ".pnpm" | ".pnpm-store" => Severity::Error,
        _ => Severity::Warning,
    }
}

/// Check for host artifacts that should only exist inside containers.
///
/// In Docker-first mode, dependencies and build outputs should stay in
/// container volumes. If they appear on the host, the bind mount is leaking.
fn check_host_artifacts(issues: &mut Vec<Issue>) -> Result<()> {
    let artifact_names = [
        "node_modules",
        ".pnpm",
        ".pnpm-store",
        ".turbo",
        ".next",
        "dist",
        "build",
        "coverage",
    ];

    use walkdir::WalkDir;

    for entry in WalkDir::new(".")
        .max_depth(5)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            // Skip .git and nested node_modules to keep it fast
            name != ".git" && (name != "node_modules" || e.depth() == 0)
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if artifact_names.contains(&name.as_ref()) {
            let path_str = path.to_string_lossy();

            // Skip root-level matches for artifacts that might legitimately exist at root
            // (e.g., ./node_modules is the root volume mount)
            if (path_str == "./node_modules" || path_str == "node_modules")
                && name == "node_modules"
            {
                continue;
            }

            let severity = artifact_severity(&name);
            let hint = match severity {
                Severity::Error => "run `airis clean && airis install`",
                Severity::Warning => "run `airis clean`",
            };

            issues.push(Issue {
                file: path_str.to_string(),
                description: format!("Host artifact `{}` leaked from container ({})", name, hint),
                severity,
            });
        }
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
            CatalogEntry::Empty(_) => resolve_version(package, "latest")?,
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

    #[test]
    fn test_artifact_severity_dependencies_are_errors() {
        assert_eq!(artifact_severity("node_modules"), Severity::Error);
        assert_eq!(artifact_severity(".pnpm"), Severity::Error);
        assert_eq!(artifact_severity(".pnpm-store"), Severity::Error);
    }

    #[test]
    fn test_artifact_severity_build_outputs_are_warnings() {
        assert_eq!(artifact_severity(".turbo"), Severity::Warning);
        assert_eq!(artifact_severity(".next"), Severity::Warning);
        assert_eq!(artifact_severity("dist"), Severity::Warning);
        assert_eq!(artifact_severity("build"), Severity::Warning);
        assert_eq!(artifact_severity("coverage"), Severity::Warning);
    }
}
