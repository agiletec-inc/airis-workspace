//! Verify command: quality and health checks for Docker-first environments
//!
//! Executes verification rules from manifest.toml inside the Docker workspace.
//! Supports global [rule.verify] and app-specific stack-based verify commands.

use crate::manifest::Manifest;
use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;

/// Run the verify command
pub fn run() -> Result<()> {
    let manifest = Manifest::load("manifest.toml")
        .with_context(|| "Failed to load manifest.toml for verification")?;

    println!(
        "{}",
        "🛡️  Running workspace quality verification...".bright_blue()
    );
    println!();

    // 1. Check if workspace container is running
    let container = find_workspace_container();
    if container.is_none() {
        println!(
            "{}",
            "⚠️  Workspace container not running. Starting it...".yellow()
        );
        // Auto-up if not running
        crate::commands::run::run("up", &[])?;
    }

    let container =
        find_workspace_container().context("Failed to find workspace container after start")?;

    // 2. Execute verification rules from manifest
    let mut failures = 0;

    // A. Global [rule.verify]
    if let Some(verify_rule) = manifest.rule.get("verify") {
        println!("{}", "🌍 Global Checks".bold());
        for cmd in &verify_rule.commands {
            if !run_verify_command(&container, cmd)? {
                failures += 1;
            }
        }
    }

    // B. App-specific verification (derived from stack)
    for app in &manifest.app {
        let mut commands = Vec::new();

        // Get framework defaults (legacy fallback)
        let framework = app.framework.as_deref().unwrap_or("node");
        let defaults = crate::conventions::framework_defaults(framework);
        for (name, cmd) in defaults.default_scripts {
            if *name == "lint" || *name == "typecheck" || *name == "test" {
                commands.push(cmd.to_string());
            }
        }

        // Add from user-defined stack (overrides/extends)
        if let Some(ref stack_name) = app.use_stack {
            if let Some(stack_def) = manifest.stack.get(stack_name) {
                if !stack_def.verify.is_empty() {
                    // If stack defines verify, it takes precedence over conventions
                    commands = stack_def.verify.clone();
                }
            }
        }

        if !commands.is_empty() {
            println!("\n{} Verifying app: {}", "📦".cyan(), app.name.bold());
            let app_path = app.path.as_deref().unwrap_or(".");
            for cmd in commands {
                // Execute in the app's directory
                let full_cmd = format!("cd {} && {}", app_path, cmd);
                if !run_verify_command(&container, &full_cmd)? {
                    failures += 1;
                }
            }
        }
    }

    println!();

    // 3. Final result
    if failures > 0 {
        println!("{} VERIFICATION FAILED ({})", "✗".red(), failures);
        println!(
            "   {}",
            "Fix the errors above before committing or finishing the task.".yellow()
        );
        std::process::exit(1);
    }

    println!(
        "{}",
        "✓ All quality checks passed. Workspace is healthy."
            .green()
            .bold()
    );
    Ok(())
}

/// Helper to run a command inside a container and return success status
fn run_verify_command(container: &str, cmd: &str) -> Result<bool> {
    println!("{} Running: {}", "→".dimmed(), cmd.cyan());

    // Execute inside Docker using `docker exec`
    // We use -T to avoid TTY issues in CI/Agents
    let status = Command::new("docker")
        .args(["exec", "-T", container, "sh", "-c", cmd])
        .status()
        .with_context(|| format!("Failed to execute verification command: {}", cmd))?;

    if status.success() {
        println!("   {} Command passed", "✅".green());
        Ok(true)
    } else {
        println!(
            "   {} Command failed with exit code: {:?}",
            "✗".red(),
            status.code()
        );
        Ok(false)
    }
}

/// Find the workspace container name (e.g., airis-workspace-workspace-1)
fn find_workspace_container() -> Option<String> {
    let output = Command::new("docker")
        .args([
            "ps",
            "--filter",
            "label=com.docker.compose.service=workspace",
            "--format",
            "{{.Names}}",
        ])
        .output()
        .ok()?;

    let names = String::from_utf8_lossy(&output.stdout);
    names.lines().next().map(|s| s.to_string())
}
