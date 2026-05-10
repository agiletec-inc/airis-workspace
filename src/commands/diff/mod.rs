//! Diff command: preview changes between manifest.toml and generated files
//!
//! Shows what `airis gen` would change without actually writing files.
//! Useful for reviewing manifest changes before applying them.

mod compute;
mod display;

#[cfg(test)]
mod tests;

use anyhow::Context;
use anyhow::Result;
use std::path::Path;

use crate::manifest::{MANIFEST_FILE, Manifest};

use compute::compute_diff;
use display::{print_stat, print_unified};

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
#[derive(Debug, serde::Serialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// File status in diff
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
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
#[derive(Debug, serde::Serialize)]
pub struct DiffResult {
    pub files: Vec<FileDiff>,
    pub summary: DiffSummary,
}

/// Diff summary statistics
#[derive(Debug, serde::Serialize)]
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
             Hint: Create one (see docs/manifest.md) or ask Claude Code via /airis:init.\n\
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
