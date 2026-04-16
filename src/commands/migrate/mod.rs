//! Safe migration module for auto-migration workflow.
//!
//! Handles:
//! - Creating workspace/ directory if needed
//! - Moving docker-compose.yml to correct locations
//! - Generating manifest.toml from discovery results
//!
//! Safety rules:
//! - NEVER overwrite existing files without user confirmation
//! - ALWAYS create backups before moving files
//! - ALWAYS warn if target file already exists

mod manifest_gen;
mod operations;

#[cfg(test)]
mod tests;

use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use super::discover::{ComposeLocation, DiscoveryResult};
use operations::{execute_create_directory, execute_generate_manifest, execute_move_file};

use serde::{Deserialize, Serialize};

/// A single migration task
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MigrationTask {
    /// Create a new directory
    CreateDirectory { path: String },
    /// Move a file from one location to another
    MoveFile { from: String, to: String },
    /// Generate manifest.toml from discovery
    GenerateManifest,
}

impl std::fmt::Display for MigrationTask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationTask::CreateDirectory { path } => {
                write!(f, "Create directory: {}", path)
            }
            MigrationTask::MoveFile { from, to } => {
                write!(f, "Move {} → {}", from, to)
            }
            MigrationTask::GenerateManifest => {
                write!(f, "Generate manifest.toml")
            }
        }
    }
}

/// Complete migration plan
#[derive(Debug, Clone)]
pub struct MigrationPlan {
    pub tasks: Vec<MigrationTask>,
    pub discovery: DiscoveryResult,
}

impl MigrationPlan {
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

/// Result of migration execution
#[derive(Debug)]
pub struct MigrationReport {
    pub completed: Vec<String>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

impl MigrationReport {
    pub(super) fn new() -> Self {
        Self {
            completed: Vec::new(),
            skipped: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Create a migration plan from discovery results
pub fn plan(discovery: DiscoveryResult) -> Result<MigrationPlan> {
    let mut tasks = Vec::new();

    // Check if workspace/ directory needs to be created
    let workspace_dir = Path::new("workspace");
    let need_workspace_dir = !workspace_dir.exists()
        && discovery
            .compose_files
            .iter()
            .any(|c| c.location == ComposeLocation::Root);

    if need_workspace_dir {
        tasks.push(MigrationTask::CreateDirectory {
            path: "workspace".to_string(),
        });
    }

    // Plan moves for root docker-compose.yml
    for compose in &discovery.compose_files {
        if compose.location == ComposeLocation::Root {
            let target = "workspace/docker-compose.yml";
            // Only plan the move if target doesn't already exist
            if !Path::new(target).exists() {
                tasks.push(MigrationTask::MoveFile {
                    from: compose.path.clone(),
                    to: target.to_string(),
                });
            }
        }
    }

    // Always generate manifest.toml (this is the main goal)
    tasks.push(MigrationTask::GenerateManifest);

    Ok(MigrationPlan { tasks, discovery })
}

/// Execute the migration plan
pub fn execute(plan: &MigrationPlan, dry_run: bool) -> Result<MigrationReport> {
    execute_in_dir(plan, dry_run, Path::new("."))
}

/// Execute the migration plan in a specific directory
pub fn execute_in_dir(
    plan: &MigrationPlan,
    dry_run: bool,
    base_dir: &Path,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::new();

    if dry_run {
        println!(
            "{}",
            "🔍 Dry-run mode: showing what would be done...".bright_blue()
        );
        println!();
    }

    for task in &plan.tasks {
        match task {
            MigrationTask::CreateDirectory { path } => {
                execute_create_directory(&base_dir.join(path), dry_run, &mut report)?;
            }
            MigrationTask::MoveFile { from, to } => {
                execute_move_file(
                    &base_dir.join(from),
                    &base_dir.join(to),
                    dry_run,
                    &mut report,
                )?;
            }
            MigrationTask::GenerateManifest => {
                execute_generate_manifest(&plan.discovery, dry_run, base_dir, &mut report)?;
            }
        }
    }

    Ok(report)
}

/// Print the migration plan
pub fn print_plan(plan: &MigrationPlan) {
    if plan.is_empty() {
        println!(
            "{}",
            "✅ No migration tasks needed. Workspace is already configured.".green()
        );
        return;
    }

    println!("{}", "📄 Migration Plan:".green());
    for (i, task) in plan.tasks.iter().enumerate() {
        println!("   {}. {}", i + 1, task);
    }
    println!();
}
