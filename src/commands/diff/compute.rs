//! Diff computation: compare manifest-generated content with current files

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::manifest::Manifest;
use crate::templates::TemplateEngine;

use super::{DiffResult, DiffSummary, FileDiff, FileStatus};

/// Compute diff between manifest-generated content and current files
pub(super) fn compute_diff(manifest: &Manifest) -> Result<DiffResult> {
    let engine = TemplateEngine::new()?;
    let resolved_catalog = crate::pnpm::read_workspace_catalog();

    let mut files = Vec::new();

    // Check package.json
    files.push(check_file_with_content(
        "package.json",
        engine.render_package_json(manifest, &resolved_catalog)?,
    )?);

    // pnpm-workspace.yaml is user-owned — not checked by airis diff

    // CI/CD workflows are project-owned — not checked by airis diff

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
pub(super) fn check_file_with_content(path: &str, expected: String) -> Result<FileDiff> {
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
pub(super) fn format_new_file_diff(path: &str, content: &str) -> String {
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
pub(super) fn compute_unified_diff(
    path: &str,
    current: &str,
    expected: &str,
) -> (usize, usize, String) {
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
