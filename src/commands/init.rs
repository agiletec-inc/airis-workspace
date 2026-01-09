use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use anyhow::Result;
use chrono::Local;
use colored::Colorize;

use crate::manifest::MANIFEST_FILE;

use super::discover;
use super::migrate;

/// Create a backup of a file before replacing it
/// Returns the backup path if successful
fn backup_file(path: &Path) -> Result<std::path::PathBuf> {
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let backup_name = format!(
        "{}.bak.{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        timestamp
    );
    let backup_path = path.parent().unwrap_or(Path::new(".")).join(&backup_name);

    fs::copy(path, &backup_path)?;
    Ok(backup_path)
}

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
pub fn run(_force_snapshot: bool, _no_snapshot: bool, write: bool, skip_discovery: bool) -> Result<()> {
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
        println!("  2. Run {} to generate workspace files", "airis generate files".bright_cyan());
        println!();
        println!("{}", "Or use Claude Code for intelligent configuration:".bright_yellow());
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
            println!(
                "{} Migration completed successfully!",
                "✅".green()
            );
        }

        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!("  1. Review {} and adjust as needed", MANIFEST_FILE.bright_cyan());
        println!("  2. Run {} to generate workspace files", "airis generate files".bright_cyan());
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
        println!(
            "{} Created {}",
            "✓".green(),
            MANIFEST_FILE.bright_cyan()
        );
        println!();
        println!("{}", "Next steps:".bright_yellow());
        println!("  1. Edit {} to configure your workspace:", MANIFEST_FILE);
        println!("     - Set [workspace].name to your project name");
        println!("     - Add your apps under [apps.*]");
        println!("     - Add your libs under [libs.*]");
        println!("     - Configure [packages.catalog] for shared dependencies");
        println!();
        println!("  2. Run {} to generate workspace files", "airis generate files".bright_cyan());
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
        println!("... ({} more lines)", MANIFEST_TEMPLATE.lines().count() - 50);
        println!();
        println!(
            "Run {} to actually create the file",
            "airis init --write --skip-discovery".bright_cyan()
        );
    }

    Ok(())
}

/// Setup .npmrc symlinks for Docker-First enforcement
/// This creates symlinks in apps/* and libs/* pointing to root .npmrc
pub fn setup_npmrc() -> Result<()> {
    println!("{}", "🔗 Setting up .npmrc symlinks...".bright_blue());
    println!();

    let root_npmrc = Path::new(".npmrc");
    if !root_npmrc.exists() {
        anyhow::bail!("Root .npmrc not found. Create it first.");
    }

    let mut created = 0;
    let mut skipped = 0;

    // Process apps directory
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        for entry in fs::read_dir(apps_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check if package.json exists (valid app)
            if !path.join("package.json").exists() {
                continue;
            }

            let npmrc_path = path.join(".npmrc");
            let relative_root = "../../.npmrc";

            if npmrc_path.exists() {
                // Check if it's already a symlink to root
                if npmrc_path.is_symlink() {
                    println!(
                        "  {} {} (already linked)",
                        "⏭️".yellow(),
                        npmrc_path.display()
                    );
                    skipped += 1;
                } else {
                    // Backup existing file before replacing
                    let backup_path = backup_file(&npmrc_path)?;
                    println!(
                        "  {} {} → {}",
                        "📦".cyan(),
                        npmrc_path.display(),
                        backup_path.display()
                    );
                    fs::remove_file(&npmrc_path)?;
                    symlink(relative_root, &npmrc_path)?;
                    println!("  {} {} (replaced)", "✓".green(), npmrc_path.display());
                    created += 1;
                }
            } else {
                // Create new symlink
                symlink(relative_root, &npmrc_path)?;
                println!("  {} {}", "✓".green(), npmrc_path.display());
                created += 1;
            }
        }
    }

    // Process libs directory
    let libs_dir = Path::new("libs");
    if libs_dir.exists() {
        for entry in fs::read_dir(libs_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check if package.json exists (valid lib)
            if !path.join("package.json").exists() {
                continue;
            }

            let npmrc_path = path.join(".npmrc");
            let relative_root = "../../.npmrc";

            if npmrc_path.exists() {
                if npmrc_path.is_symlink() {
                    println!(
                        "  {} {} (already linked)",
                        "⏭️".yellow(),
                        npmrc_path.display()
                    );
                    skipped += 1;
                } else {
                    // Backup existing file before replacing
                    let backup_path = backup_file(&npmrc_path)?;
                    println!(
                        "  {} {} → {}",
                        "📦".cyan(),
                        npmrc_path.display(),
                        backup_path.display()
                    );
                    fs::remove_file(&npmrc_path)?;
                    symlink(relative_root, &npmrc_path)?;
                    println!("  {} {} (replaced)", "✓".green(), npmrc_path.display());
                    created += 1;
                }
            } else {
                symlink(relative_root, &npmrc_path)?;
                println!("  {} {}", "✓".green(), npmrc_path.display());
                created += 1;
            }
        }
    }

    println!();
    println!(
        "{} Created {} symlinks, skipped {} existing",
        "✅".green(),
        created,
        skipped
    );
    println!();
    println!("{}", "🛡️  Triple-layer defense active:".bright_yellow());
    println!("  1. .npmrc symlinks (primary)");
    println!("  2. preinstall hooks (backup)");
    println!("  3. Root preinstall + monorepo check (fallback)");

    Ok(())
}
