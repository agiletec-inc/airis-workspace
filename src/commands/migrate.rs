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

use anyhow::{bail, Context, Result};
use chrono::Local;
use colored::Colorize;
use indexmap::IndexMap;
use std::fs;
use std::path::Path;

use super::discover::{ComposeLocation, DiscoveryResult};

/// Format an IndexMap as a TOML inline table
/// e.g., { "dev" = "next dev", "build" = "next build" }
fn format_inline_table(map: &IndexMap<String, String>) -> String {
    let pairs: Vec<String> = map
        .iter()
        .map(|(k, v)| format!("\"{}\" = \"{}\"", escape_toml_string(k), escape_toml_string(v)))
        .collect();
    format!("{{ {} }}", pairs.join(", "))
}

/// Escape special characters in TOML strings
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// A single migration task
#[derive(Debug, Clone)]
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
    fn new() -> Self {
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
pub fn execute_in_dir(plan: &MigrationPlan, dry_run: bool, base_dir: &Path) -> Result<MigrationReport> {
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
                execute_move_file(&base_dir.join(from), &base_dir.join(to), dry_run, &mut report)?;
            }
            MigrationTask::GenerateManifest => {
                execute_generate_manifest(&plan.discovery, dry_run, base_dir, &mut report)?;
            }
        }
    }

    Ok(report)
}

/// Execute directory creation
fn execute_create_directory(dir: &Path, dry_run: bool, report: &mut MigrationReport) -> Result<()> {
    let path_str = dir.display().to_string();

    if dir.exists() {
        let msg = format!("Directory already exists: {}", path_str);
        println!("   {} {}", "⏭️".yellow(), msg);
        report.skipped.push(msg);
        return Ok(());
    }

    if dry_run {
        println!("   {} Would create directory: {}", "→".bright_blue(), path_str);
        report.completed.push(format!("Would create: {}", path_str));
    } else {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create directory: {}", path_str))?;
        println!("   {} Created directory: {}", "✓".green(), path_str);
        report.completed.push(format!("Created: {}", path_str));
    }

    Ok(())
}

/// Execute file move with backup
fn execute_move_file(from_path: &Path, to_path: &Path, dry_run: bool, report: &mut MigrationReport) -> Result<()> {
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
        report.completed.push(format!("Would move: {} → {}", from_str, to_str));
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
        report.completed.push(format!("Moved: {} → {}", from_str, to_str));
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
fn execute_generate_manifest(
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
        println!(
            "   {} Would generate manifest.toml:",
            "→".bright_blue()
        );
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

/// Generate manifest.toml content from discovery results
fn generate_manifest_content(discovery: &DiscoveryResult) -> Result<String> {
    let mut lines = Vec::new();

    // Header
    lines.push("# Auto-generated by airis init".to_string());
    lines.push("# Edit this file to configure your workspace".to_string());
    lines.push("".to_string());
    lines.push("version = 1".to_string());
    lines.push("mode = \"docker-first\"".to_string());
    lines.push("".to_string());

    // Project section
    lines.push("[project]".to_string());
    lines.push("name = \"workspace\"".to_string());
    lines.push("description = \"Auto-discovered workspace\"".to_string());
    lines.push("".to_string());

    // Workspace section
    lines.push("[workspace]".to_string());
    lines.push("name = \"workspace\"".to_string());
    lines.push("".to_string());

    // Packages section
    lines.push("[packages]".to_string());
    lines.push("workspaces = [\"apps/*\", \"libs/*\"]".to_string());
    lines.push("".to_string());

    // Catalog from discovery
    if !discovery.catalog.is_empty() {
        lines.push("[packages.catalog]".to_string());
        for (name, version) in &discovery.catalog {
            lines.push(format!("\"{}\" = \"{}\"", name, version));
        }
        lines.push("".to_string());
    }

    // App definitions (using [[app]] array format for package.json generation)
    if !discovery.apps.is_empty() {
        for app in &discovery.apps {
            lines.push("[[app]]".to_string());
            lines.push(format!("name = \"{}\"", app.name));
            lines.push(format!("path = \"{}\"", app.path));
            lines.push("kind = \"app\"".to_string());
            lines.push(format!("framework = \"{}\"", app.framework));

            // Scripts as inline table
            if !app.scripts.is_empty() {
                let scripts_str = format_inline_table(&app.scripts);
                lines.push(format!("scripts = {}", scripts_str));
            }

            // Dependencies as inline table
            if !app.deps.is_empty() {
                let deps_str = format_inline_table(&app.deps);
                lines.push(format!("deps = {}", deps_str));
            }

            // Dev dependencies as inline table
            if !app.dev_deps.is_empty() {
                let dev_deps_str = format_inline_table(&app.dev_deps);
                lines.push(format!("dev_deps = {}", dev_deps_str));
            }

            lines.push("".to_string());
        }
    }

    // Library definitions (using [[app]] with kind = "lib")
    if !discovery.libs.is_empty() {
        for lib in &discovery.libs {
            lines.push("[[app]]".to_string());
            lines.push(format!("name = \"{}\"", lib.name));
            lines.push(format!("path = \"{}\"", lib.path));
            lines.push("kind = \"lib\"".to_string());

            // Scripts as inline table
            if !lib.scripts.is_empty() {
                let scripts_str = format_inline_table(&lib.scripts);
                lines.push(format!("scripts = {}", scripts_str));
            }

            // Dependencies as inline table
            if !lib.deps.is_empty() {
                let deps_str = format_inline_table(&lib.deps);
                lines.push(format!("deps = {}", deps_str));
            }

            // Dev dependencies as inline table
            if !lib.dev_deps.is_empty() {
                let dev_deps_str = format_inline_table(&lib.dev_deps);
                lines.push(format!("dev_deps = {}", dev_deps_str));
            }

            lines.push("".to_string());
        }
    }

    // Orchestration section (docker-compose paths)
    let workspace_compose = discovery
        .compose_files
        .iter()
        .find(|c| c.location == ComposeLocation::Workspace || c.location == ComposeLocation::Root);
    let supabase_compose = discovery
        .compose_files
        .iter()
        .find(|c| c.location == ComposeLocation::Supabase);
    let traefik_compose = discovery
        .compose_files
        .iter()
        .find(|c| c.location == ComposeLocation::Traefik);

    if workspace_compose.is_some() || supabase_compose.is_some() || traefik_compose.is_some() {
        lines.push("[orchestration.dev]".to_string());

        if let Some(compose) = workspace_compose {
            // If it was at root, it will be moved to workspace/
            let path = if compose.location == ComposeLocation::Root {
                "workspace/docker-compose.yml".to_string()
            } else {
                compose.path.clone()
            };
            lines.push(format!("workspace = \"{}\"", path));
        }

        if let Some(compose) = supabase_compose {
            lines.push(format!("supabase = [\"{}\"]", compose.path));
        }

        if let Some(compose) = traefik_compose {
            lines.push(format!("traefik = \"{}\"", compose.path));
        }

        lines.push("".to_string());
    }

    // Guards section (docker-first defaults)
    lines.push("[guards]".to_string());
    lines.push("deny = [\"npm\", \"yarn\", \"pnpm\"]".to_string());
    lines.push("".to_string());

    // Commands section
    lines.push("[commands]".to_string());
    lines.push("install = \"docker compose run --rm node pnpm install\"".to_string());
    lines.push("dev = \"docker compose up\"".to_string());
    lines.push("build = \"docker compose run --rm node pnpm build\"".to_string());
    lines.push("test = \"docker compose run --rm node pnpm test\"".to_string());
    lines.push("".to_string());

    // Versioning section
    lines.push("[versioning]".to_string());
    lines.push("strategy = \"conventional-commits\"".to_string());

    Ok(lines.join("\n"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::discover::{DetectedApp, DetectedCompose, DetectedLib, Framework};
    use indexmap::IndexMap;
    use tempfile::tempdir;

    fn create_test_discovery() -> DiscoveryResult {
        let mut scripts = IndexMap::new();
        scripts.insert("dev".to_string(), "next dev".to_string());
        scripts.insert("build".to_string(), "next build".to_string());

        let mut deps = IndexMap::new();
        deps.insert("react".to_string(), "catalog:".to_string());
        deps.insert("next".to_string(), "catalog:".to_string());

        let mut dev_deps = IndexMap::new();
        dev_deps.insert("typescript".to_string(), "catalog:".to_string());

        let mut lib_scripts = IndexMap::new();
        lib_scripts.insert("build".to_string(), "tsup".to_string());

        DiscoveryResult {
            apps: vec![DetectedApp {
                name: "web".to_string(),
                path: "apps/web".to_string(),
                framework: Framework::NextJs,
                has_dockerfile: true,
                package_name: Some("@workspace/web".to_string()),
                scripts,
                deps,
                dev_deps,
            }],
            libs: vec![DetectedLib {
                name: "ui".to_string(),
                path: "libs/ui".to_string(),
                package_name: Some("@workspace/ui".to_string()),
                scripts: lib_scripts,
                deps: IndexMap::new(),
                dev_deps: IndexMap::new(),
            }],
            compose_files: vec![DetectedCompose {
                path: "docker-compose.yml".to_string(),
                location: ComposeLocation::Root,
            }],
            catalog: {
                let mut m = IndexMap::new();
                m.insert("typescript".to_string(), "^5.0.0".to_string());
                m
            },
        }
    }

    #[test]
    fn test_plan_creates_workspace_dir_task() {
        let discovery = create_test_discovery();
        let plan = plan(discovery).unwrap();

        // Should have CreateDirectory task for workspace/
        assert!(plan.tasks.iter().any(|t| matches!(
            t,
            MigrationTask::CreateDirectory { path } if path == "workspace"
        )));
    }

    #[test]
    fn test_plan_creates_move_task_for_root_compose() {
        let discovery = create_test_discovery();
        let plan = plan(discovery).unwrap();

        // Should have MoveFile task
        assert!(plan.tasks.iter().any(|t| matches!(
            t,
            MigrationTask::MoveFile { from, to }
            if from == "docker-compose.yml" && to == "workspace/docker-compose.yml"
        )));
    }

    #[test]
    fn test_plan_always_includes_generate_manifest() {
        let discovery = create_test_discovery();
        let plan = plan(discovery).unwrap();

        // Should always have GenerateManifest task
        assert!(plan
            .tasks
            .iter()
            .any(|t| matches!(t, MigrationTask::GenerateManifest)));
    }

    #[test]
    fn test_generate_manifest_content() {
        let discovery = create_test_discovery();
        let content = generate_manifest_content(&discovery).unwrap();

        assert!(content.contains("version = 1"));
        // New format uses [[app]] instead of [apps.name]
        assert!(content.contains("[[app]]"));
        assert!(content.contains("name = \"web\""));
        assert!(content.contains("framework = \"nextjs\""));
        assert!(content.contains("kind = \"app\""));
        // Check library is also using [[app]] with kind = "lib"
        assert!(content.contains("kind = \"lib\""));
        assert!(content.contains("name = \"ui\""));
        // Check catalog
        assert!(content.contains("[packages.catalog]"));
        assert!(content.contains("typescript"));
        // Check scripts/deps are included
        assert!(content.contains("scripts = {"));
        assert!(content.contains("deps = {"));
    }

    #[test]
    fn test_dry_run_does_not_create_files() {
        let dir = tempdir().unwrap();

        let discovery = DiscoveryResult {
            apps: vec![],
            libs: vec![],
            compose_files: vec![],
            catalog: IndexMap::new(),
        };

        let migration_plan = plan(discovery).unwrap();
        let _report = execute_in_dir(&migration_plan, true, dir.path()).unwrap();

        // manifest.toml should NOT exist after dry-run
        assert!(!dir.path().join("manifest.toml").exists());
    }

    #[test]
    fn test_execute_creates_manifest() {
        let dir = tempdir().unwrap();

        let discovery = DiscoveryResult {
            apps: vec![],
            libs: vec![],
            compose_files: vec![],
            catalog: IndexMap::new(),
        };

        let migration_plan = plan(discovery).unwrap();
        let report = execute_in_dir(&migration_plan, false, dir.path()).unwrap();

        // manifest.toml should exist
        assert!(dir.path().join("manifest.toml").exists());
        assert!(!report.has_errors());
    }
}
