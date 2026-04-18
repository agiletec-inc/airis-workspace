use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::manifest::MANIFEST_FILE;

use super::discover;
use super::migrate;

/// Initialize a new airis workspace
///
/// Workflow:
/// 1. If manifest.toml exists, show guidance
/// 2. If manifest.toml doesn't exist:
///    a. Run project discovery (apps, libs, compose files, catalog)
///    b. Create migration plan
///    c. Show plan (dry-run by default)
///    d. Execute plan if --write is passed
pub fn run(
    _force_snapshot: bool,
    _no_snapshot: bool,
    write: bool,
    _skip_discovery: bool,
) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if manifest_path.exists() {
        // manifest.toml already exists - show guidance
        println!(
            "{} {} already exists",
            "✓".green(),
            MANIFEST_FILE.bright_cyan()
        );
        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!("  1. Edit {} to configure your workspace", MANIFEST_FILE);
        println!(
            "  2. Run {} to generate workspace files",
            "airis gen".bright_cyan()
        );
        println!();
        println!(
            "{}",
            "Or use Claude Code for intelligent configuration:".bright_yellow()
        );
        println!("  Ask Claude to analyze your repo and update manifest.toml");
        return Ok(());
    }

    // Run discovery workflow
    run_discovery_mode(write)
}

/// Run the discovery-based initialization workflow
fn run_discovery_mode(write: bool) -> Result<()> {
    // Phase 1: Discovery
    let discovery = discover::run()?;

    // Phase 2: Create migration plan
    let plan = migrate::plan(discovery)?;

    // Phase 3: Show the plan
    migrate::print_plan(&plan);

    // Phase 4: Execute or show dry-run message
    if write {
        println!("{}", "🚀 Executing migration plan...".bright_blue());
        println!();

        let report = migrate::execute(&plan, false)?;

        println!();
        if report.has_errors() {
            println!(
                "{} Migration completed with {} error(s)",
                "⚠️".yellow(),
                report.errors.len()
            );
            for err in &report.errors {
                println!("   {} {}", "✗".red(), err);
            }
        } else {
            println!("{} Migration completed successfully!", "✅".green());
        }

        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!(
            "  1. Review {} and adjust as needed",
            MANIFEST_FILE.bright_cyan()
        );
        println!(
            "  2. Run {} to generate workspace files",
            "airis gen".bright_cyan()
        );
    } else {
        // Dry-run: show what would happen
        let _report = migrate::execute(&plan, true)?;

        println!();
        println!("{}", "Options:".dimmed());
        println!(
            "  {} - Analyze and write generated files",
            "--write".dimmed()
        );
    }

    Ok(())
}
