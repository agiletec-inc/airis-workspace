//! Diff command: preview changes between manifest.toml and generated files
//!
//! Shows what `airis generate files` would change without actually writing files.
//! Useful for reviewing manifest changes before applying them.

use anyhow::{Context, Result};
use colored::Colorize;
use indexmap::IndexMap;
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::commands::sync_deps::resolve_version;
use crate::manifest::{CatalogEntry, Manifest, MANIFEST_FILE};
use crate::templates::TemplateEngine;

/// Diff output format
#[derive(Debug, Clone)]
pub enum DiffFormat {
    /// Human-readable unified diff
    Unified,
    /// JSON output for automation/CI
    Json,
    /// Statistics only (file count, line changes)
    Stat,
}

/// A single file diff
#[derive(Debug, Serialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// File status in diff
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    /// File would be created (doesn't exist)
    Created,
    /// File would be modified (content differs)
    Modified,
    /// File is unchanged
    Unchanged,
}

/// Overall diff result
#[derive(Debug, Serialize)]
pub struct DiffResult {
    pub files: Vec<FileDiff>,
    pub summary: DiffSummary,
}

/// Diff summary statistics
#[derive(Debug, Serialize)]
pub struct DiffSummary {
    pub files_changed: usize,
    pub files_created: usize,
    pub files_unchanged: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
}

/// Run the diff command
pub fn run(format: DiffFormat) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if !manifest_path.exists() {
        anyhow::bail!(
            "manifest.toml not found.\n\n\
             Hint: Run `airis init` to create one.\n\
             This command requires an airis workspace."
        );
    }

    let manifest = Manifest::load(manifest_path).context("Failed to load manifest.toml")?;

    // Generate all files in memory and compare
    let result = compute_diff(&manifest)?;

    match format {
        DiffFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        DiffFormat::Stat => {
            print_stat(&result);
        }
        DiffFormat::Unified => {
            print_unified(&result);
        }
    }

    Ok(())
}

/// Compute diff between manifest-generated content and current files
fn compute_diff(manifest: &Manifest) -> Result<DiffResult> {
    let engine = TemplateEngine::new()?;
    let resolved_catalog = resolve_catalog_versions_quiet(&manifest.packages.catalog)?;

    let mut files = Vec::new();

    // Check package.json
    files.push(check_file_with_content(
        "package.json",
        engine.render_package_json(manifest, &resolved_catalog)?,
    )?);

    // Check compose.yml (modern) or docker-compose.yml (legacy)
    let compose_file = if Path::new("compose.yml").exists() {
        "compose.yml"
    } else {
        "docker-compose.yml"
    };
    files.push(check_file_with_content(
        compose_file,
        engine.render_docker_compose(manifest)?,
    )?);

    // Check Dockerfile
    files.push(check_file_with_content(
        "Dockerfile",
        engine.render_dockerfile_dev(manifest)?,
    )?);

    // Check pnpm-workspace.yaml if workspaces exist
    if !manifest.packages.workspaces.is_empty() {
        files.push(check_file_with_content(
            "pnpm-workspace.yaml",
            engine.render_pnpm_workspace(manifest)?,
        )?);
    }

    // Check GitHub workflows if CI is enabled
    if manifest.ci.enabled {
        files.push(check_file_with_content(
            ".github/workflows/ci.yml",
            engine.render_ci_yml(manifest)?,
        )?);
        files.push(check_file_with_content(
            ".github/workflows/release.yml",
            engine.render_release_yml(manifest)?,
        )?);
    }

    // Calculate summary
    let summary = DiffSummary {
        files_changed: files
            .iter()
            .filter(|f| f.status == FileStatus::Modified)
            .count(),
        files_created: files
            .iter()
            .filter(|f| f.status == FileStatus::Created)
            .count(),
        files_unchanged: files
            .iter()
            .filter(|f| f.status == FileStatus::Unchanged)
            .count(),
        total_additions: files.iter().map(|f| f.additions).sum(),
        total_deletions: files.iter().map(|f| f.deletions).sum(),
    };

    Ok(DiffResult { files, summary })
}

/// Check a single file and compute its diff using pre-generated content
fn check_file_with_content(path: &str, expected: String) -> Result<FileDiff> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        // File would be created
        let lines = expected.lines().count();
        return Ok(FileDiff {
            path: path.to_string(),
            status: FileStatus::Created,
            additions: lines,
            deletions: 0,
            diff: Some(format_new_file_diff(path, &expected)),
        });
    }

    let current =
        fs::read_to_string(file_path).with_context(|| format!("Failed to read {}", path))?;

    // Normalize line endings
    let current_normalized = current.replace("\r\n", "\n");
    let expected_normalized = expected.replace("\r\n", "\n");

    if current_normalized == expected_normalized {
        return Ok(FileDiff {
            path: path.to_string(),
            status: FileStatus::Unchanged,
            additions: 0,
            deletions: 0,
            diff: None,
        });
    }

    // Compute unified diff
    let (additions, deletions, diff_text) =
        compute_unified_diff(path, &current_normalized, &expected_normalized);

    Ok(FileDiff {
        path: path.to_string(),
        status: FileStatus::Modified,
        additions,
        deletions,
        diff: Some(diff_text),
    })
}

/// Format diff for a new file (all additions)
fn format_new_file_diff(path: &str, content: &str) -> String {
    let mut output = String::new();
    output.push_str("--- /dev/null\n");
    output.push_str(&format!("+++ {} (generated)\n", path));
    output.push_str("@@ -0,0 +1 @@\n");
    for line in content.lines() {
        output.push_str(&format!("+{}\n", line));
    }
    output
}

/// Compute unified diff between two strings
fn compute_unified_diff(path: &str, current: &str, expected: &str) -> (usize, usize, String) {
    let current_lines: Vec<&str> = current.lines().collect();
    let expected_lines: Vec<&str> = expected.lines().collect();

    let mut output = String::new();
    output.push_str(&format!("--- {}\n", path));
    output.push_str(&format!("+++ {} (generated)\n", path));

    let mut additions = 0;
    let mut deletions = 0;

    // Simple line-by-line diff (not a full Myers diff, but good enough for display)
    let max_len = current_lines.len().max(expected_lines.len());
    let mut hunks: Vec<(usize, Vec<String>)> = Vec::new();
    let mut current_hunk: Vec<String> = Vec::new();
    let mut hunk_start: Option<usize> = None;
    let context_lines = 3;

    for i in 0..max_len {
        let current_line = current_lines.get(i).copied();
        let expected_line = expected_lines.get(i).copied();

        match (current_line, expected_line) {
            (Some(c), Some(e)) if c == e => {
                // Context line
                if !current_hunk.is_empty() {
                    current_hunk.push(format!(" {}", c));
                }
            }
            (Some(c), Some(e)) => {
                // Modified line
                if hunk_start.is_none() {
                    hunk_start = Some(i.saturating_sub(context_lines));
                    // Add context before
                    for j in i.saturating_sub(context_lines)..i {
                        if let Some(ctx) = current_lines.get(j) {
                            current_hunk.push(format!(" {}", ctx));
                        }
                    }
                }
                current_hunk.push(format!("-{}", c));
                current_hunk.push(format!("+{}", e));
                deletions += 1;
                additions += 1;
            }
            (Some(c), None) => {
                // Deleted line
                if hunk_start.is_none() {
                    hunk_start = Some(i.saturating_sub(context_lines));
                    for j in i.saturating_sub(context_lines)..i {
                        if let Some(ctx) = current_lines.get(j) {
                            current_hunk.push(format!(" {}", ctx));
                        }
                    }
                }
                current_hunk.push(format!("-{}", c));
                deletions += 1;
            }
            (None, Some(e)) => {
                // Added line
                if hunk_start.is_none() {
                    hunk_start = Some(i.saturating_sub(context_lines));
                    for j in i.saturating_sub(context_lines)..i {
                        if let Some(ctx) = current_lines.get(j) {
                            current_hunk.push(format!(" {}", ctx));
                        }
                    }
                }
                current_hunk.push(format!("+{}", e));
                additions += 1;
            }
            (None, None) => unreachable!(),
        }

        // Check if we should close the current hunk
        if !current_hunk.is_empty() {
            let last_change_idx = current_hunk
                .iter()
                .rposition(|l| l.starts_with('+') || l.starts_with('-'));
            if let Some(last_idx) = last_change_idx {
                let context_after = current_hunk.len() - last_idx - 1;
                if context_after >= context_lines
                    && let Some(start) = hunk_start.take()
                {
                    hunks.push((start, std::mem::take(&mut current_hunk)));
                }
            }
        }
    }

    // Push remaining hunk
    if !current_hunk.is_empty()
        && let Some(start) = hunk_start
    {
        hunks.push((start, current_hunk));
    }

    // Format hunks
    for (start, hunk) in hunks {
        let hunk_len = hunk.len();
        output.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            start + 1,
            hunk_len,
            start + 1,
            hunk_len
        ));
        for line in hunk {
            output.push_str(&line);
            output.push('\n');
        }
    }

    (additions, deletions, output)
}

/// Print unified diff output
fn print_unified(result: &DiffResult) {
    if result.summary.files_changed == 0 && result.summary.files_created == 0 {
        println!("{}", "✅ No changes detected".green());
        println!("   All generated files are in sync with manifest.toml");
        return;
    }

    println!(
        "{}",
        "📝 Diff Preview (manifest.toml → generated files)"
            .bright_blue()
            .bold()
    );
    println!();

    for file in &result.files {
        match file.status {
            FileStatus::Unchanged => continue,
            FileStatus::Created => {
                println!(
                    "{}",
                    format!("=== {} (new file) ===", file.path).green().bold()
                );
            }
            FileStatus::Modified => {
                println!("{}", format!("=== {} ===", file.path).yellow().bold());
            }
        }

        if let Some(ref diff) = file.diff {
            for line in diff.lines() {
                if line.starts_with('+') && !line.starts_with("+++") {
                    println!("{}", line.green());
                } else if line.starts_with('-') && !line.starts_with("---") {
                    println!("{}", line.red());
                } else if line.starts_with("@@") {
                    println!("{}", line.cyan());
                } else {
                    println!("{}", line);
                }
            }
        }
        println!();
    }

    // Summary
    println!("{}", "─".repeat(50).dimmed());
    print_summary(&result.summary);
}

/// Print statistics only
fn print_stat(result: &DiffResult) {
    if result.summary.files_changed == 0 && result.summary.files_created == 0 {
        println!("{}", "✅ No changes".green());
        return;
    }

    println!("{}", "📊 Diff Statistics".bright_blue().bold());
    println!();

    for file in &result.files {
        if file.status == FileStatus::Unchanged {
            continue;
        }

        let status_indicator = match file.status {
            FileStatus::Created => "A".green(),
            FileStatus::Modified => "M".yellow(),
            FileStatus::Unchanged => " ".normal(),
        };

        let changes = format!("+{} -{}", file.additions, file.deletions);
        println!(
            " {} {:40} {}",
            status_indicator,
            file.path,
            changes.dimmed()
        );
    }

    println!();
    print_summary(&result.summary);
}

/// Print summary line
fn print_summary(summary: &DiffSummary) {
    let mut parts = Vec::new();

    if summary.files_created > 0 {
        parts.push(
            format!("{} file(s) created", summary.files_created)
                .green()
                .to_string(),
        );
    }
    if summary.files_changed > 0 {
        parts.push(
            format!("{} file(s) modified", summary.files_changed)
                .yellow()
                .to_string(),
        );
    }

    if parts.is_empty() {
        println!("{}", "No changes".dimmed());
    } else {
        let summary_text = format!(
            "{}, {} insertions(+), {} deletions(-)",
            parts.join(", "),
            summary.total_additions,
            summary.total_deletions
        );
        println!("{}", summary_text);
    }
}

/// Resolve catalog versions without printing (for diff command)
fn resolve_catalog_versions_quiet(
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
    fn test_file_status_serialize() {
        assert_eq!(
            serde_json::to_string(&FileStatus::Created).unwrap(),
            "\"created\""
        );
        assert_eq!(
            serde_json::to_string(&FileStatus::Modified).unwrap(),
            "\"modified\""
        );
    }

    #[test]
    fn test_format_new_file_diff() {
        let diff = format_new_file_diff("test.txt", "line1\nline2");
        assert!(diff.contains("+++ test.txt"));
        assert!(diff.contains("+line1"));
        assert!(diff.contains("+line2"));
    }

    #[test]
    fn test_compute_unified_diff_no_changes() {
        let content = "line1\nline2\nline3";
        let (adds, dels, _) = compute_unified_diff("test.txt", content, content);
        assert_eq!(adds, 0);
        assert_eq!(dels, 0);
    }

    #[test]
    fn test_compute_unified_diff_with_changes() {
        let current = "line1\nold\nline3";
        let expected = "line1\nnew\nline3";
        let (adds, dels, diff) = compute_unified_diff("test.txt", current, expected);
        assert_eq!(adds, 1);
        assert_eq!(dels, 1);
        assert!(diff.contains("-old"));
        assert!(diff.contains("+new"));
    }
}
