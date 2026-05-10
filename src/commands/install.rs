use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

use crate::commands::run::compose::find_compose_file;
use crate::manifest::Manifest;

/// Execute package installation inside Docker container
pub fn run(extra_args: &[String]) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");
    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Create one (see docs/manifest.md) or ask Claude Code via {}.",
            "/airis:init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path).with_context(|| "Failed to load manifest.toml")?;
    let pm_info = &manifest.workspace.package_manager;

    // Extract command name (e.g., "pnpm@10.22.0" -> "pnpm")
    let pm_cmd = if pm_info.contains('@') {
        pm_info.split('@').next().unwrap_or("pnpm")
    } else {
        pm_info.as_str()
    };

    println!(
        "{} Running {} inside Docker container...",
        "📦".cyan(),
        format!("{} install", pm_cmd).bold()
    );

    let compose_file = find_compose_file().ok_or_else(|| {
        anyhow::anyhow!("No compose file found. Run 'airis gen' to generate one.")
    })?;

    // We use 'exec' if the container is running, otherwise 'run --rm'
    // But for a workspace, we expect it to be running after 'airis up'

    // Build the install command string
    // If no args, just run 'install'
    // If args are provided, check if they already include a verb like 'add', 'install', 'remove'
    let mut full_args = Vec::new();

    let has_verb = extra_args.iter().any(|arg| {
        matches!(
            arg.as_str(),
            "install" | "add" | "remove" | "uninstall" | "update" | "i"
        )
    });

    if !has_verb {
        // Default to 'install' if no packages specified, or 'add' if packages provided
        if extra_args.is_empty() {
            full_args.push("install");
        } else if pm_cmd == "pnpm" || pm_cmd == "yarn" {
            full_args.push("add");
        } else {
            full_args.push("install");
        }
    }

    for arg in extra_args {
        full_args.push(arg.as_str());
    }

    // Try 'docker compose exec'. If it fails because container is not running,
    // it's a hint that the user should run 'airis up' first.
    let mut cmd = Command::new("docker");
    cmd.args(["compose", "-f", compose_file, "exec", "workspace", pm_cmd]);
    cmd.args(&full_args);

    let status = cmd
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to start docker process: {}", e))?;

    if !status.success() {
        // If exec fails, it's often because the container is not running.
        // Check container status to provide a better message.
        let is_running = Command::new("docker")
            .args([
                "compose",
                "-f",
                compose_file,
                "ps",
                "--format",
                "json",
                "workspace",
            ])
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        if !is_running {
            println!(
                "\n{}",
                "⚠️  Workspace container is not running.".yellow().bold()
            );
            println!("   To install dependencies, you must start the environment first:");
            println!("   {}", "airis up".cyan().bold());
            println!();
            bail!("Installation skipped: environment not ready.");
        }

        bail!(
            "Installation failed inside container (exit code: {:?})",
            status.code()
        );
    }

    println!(
        "\n{}",
        "✅ Dependencies installed successfully inside Docker!"
            .green()
            .bold()
    );
    Ok(())
}
