use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use anyhow::Result;
use colored::Colorize;

/// DEPRECATED: airis init is now handled by MCP tool /airis:init
///
/// The init command has been moved to an MCP-based agent workflow that:
/// - Analyzes the entire repository structure intelligently
/// - Understands language/framework specifics (Next.js, Rust, Python, etc.)
/// - Generates optimized manifest.toml based on actual project needs
/// - Handles complex monorepo configurations
///
/// Use `/airis:init` in Claude Code or your MCP-enabled client instead.
pub fn run(_force_snapshot: bool, _no_snapshot: bool, _write: bool) -> Result<()> {
    println!("{}", "⛔ airis init has been deprecated".bright_red());
    println!();
    println!(
        "{}",
        "This command is now handled by the MCP tool /airis:init".yellow()
    );
    println!();
    println!("{}", "Why?".bright_yellow());
    println!("  The init process requires intelligent analysis of your repository:");
    println!("  - Detecting frameworks (Next.js, Hono, Rust, Python, etc.)");
    println!("  - Understanding monorepo structure and dependencies");
    println!("  - Generating optimized manifest.toml for your specific setup");
    println!("  - Handling edge cases that rule-based code can't anticipate");
    println!();
    println!("{}", "How to use:".bright_yellow());
    println!("  1. Open Claude Code (or any MCP-enabled client)");
    println!("  2. Run: /airis:init");
    println!("  3. The agent will analyze your repo and generate manifest.toml");
    println!();
    println!(
        "{}",
        "For simple file regeneration from existing manifest.toml:".bright_yellow()
    );
    println!("  airis generate files");
    println!();

    // Exit with error to indicate this command should not be used
    std::process::exit(1);
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
                    // Remove existing file and create symlink
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
