use std::fs;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

use crate::manifest::MANIFEST_FILE;

use super::discover;
use super::migrate;

/// Default manifest.toml template (embedded at compile time)
const MANIFEST_TEMPLATE: &str = include_str!("../../examples/manifest.toml");

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
    skip_discovery: bool,
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

    // manifest.toml doesn't exist - run auto-migration workflow
    if skip_discovery {
        // Skip discovery, just create from template (legacy behavior)
        return run_template_mode(write);
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
        println!(
            "Run {} to execute this plan",
            "airis init --write".bright_cyan()
        );
        println!();
        println!("{}", "Options:".dimmed());
        println!(
            "  {} - Use empty template instead of discovery",
            "--skip-discovery".dimmed()
        );
    }

    Ok(())
}

/// Run the template-based initialization (legacy mode)
fn run_template_mode(write: bool) -> Result<()> {
    let manifest_path = Path::new(MANIFEST_FILE);

    if write {
        fs::write(manifest_path, MANIFEST_TEMPLATE)?;
        println!("{} Created {}", "✓".green(), MANIFEST_FILE.bright_cyan());
        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!("  1. Edit {} to configure your workspace:", MANIFEST_FILE);
        println!("     - Set [workspace].name to your project name");
        println!("     - Add your apps under [apps.*]");
        println!("     - Add your libs under [libs.*]");
        println!("     - Configure [packages.catalog] for shared dependencies");
        println!();
        println!(
            "  2. Run {} to generate workspace files",
            "airis gen".bright_cyan()
        );
        println!();
        println!("{}", "Pro tip:".bright_yellow());
        println!("  Use Claude Code to intelligently configure manifest.toml");
        println!("  based on your existing project structure.");
    } else {
        // Dry-run mode - show what would be created
        println!(
            "{} Would create {}",
            "→".bright_blue(),
            MANIFEST_FILE.bright_cyan()
        );
        println!();
        println!("{}", "Preview (first 50 lines):".bright_yellow());
        println!("{}", "─".repeat(60));
        for line in MANIFEST_TEMPLATE.lines().take(50) {
            println!("{}", line);
        }
        println!("{}", "─".repeat(60));
        println!(
            "... ({} more lines)",
            MANIFEST_TEMPLATE.lines().count() - 50
        );
        println!();
        println!(
            "Run {} to actually create the file",
            "airis init --write --skip-discovery".bright_cyan()
        );
    }

    Ok(())
}
