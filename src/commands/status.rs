use crate::manifest::{MANIFEST_FILE, Manifest};
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

pub fn run(short: bool) -> Result<()> {
    if short {
        return run_short();
    }

    // Detailed status (not implemented yet, fallback to a nice summary)
    println!("{}", "🌀 Airis Status".bright_blue().bold());
    println!("{}", "━".repeat(40).dimmed());

    // Project Info
    let manifest_path = Path::new(MANIFEST_FILE);
    if manifest_path.exists() {
        if let Ok(manifest) = Manifest::load(manifest_path) {
            println!(
                "{:<15} {}",
                "Project:".bright_yellow(),
                manifest.workspace.name.cyan()
            );
        }
    } else {
        let current_dir = std::env::current_dir()?;
        let dir_name = current_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        println!(
            "{:<15} {} (no manifest)",
            "Directory:".bright_yellow(),
            dir_name.dimmed()
        );
    }

    // Docker Context
    let has_compose = Path::new("compose.yaml").exists()
        || Path::new("compose.yml").exists()
        || Path::new("docker-compose.yaml").exists()
        || Path::new("docker-compose.yml").exists();

    println!(
        "{:<15} {}",
        "Docker context:".bright_yellow(),
        if has_compose {
            "✓".green()
        } else {
            "✗".red()
        }
    );

    Ok(())
}

fn run_short() -> Result<()> {
    let mut parts = Vec::new();

    let manifest_path = Path::new(MANIFEST_FILE);
    let has_project_context = manifest_path.exists()
        || Path::new("compose.yaml").exists()
        || Path::new("compose.yml").exists()
        || Path::new("docker-compose.yaml").exists()
        || Path::new("docker-compose.yml").exists();

    // 1. Airis Icon
    parts.push("🌀".into());

    if has_project_context {
        // --- Project Context ---
        let name = if manifest_path.exists() {
            Manifest::load(manifest_path)
                .map(|m| m.workspace.name)
                .unwrap_or_else(|_| "unknown".into())
        } else {
            let current_dir = std::env::current_dir()?;
            current_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };

        // POLLUTION CHECK: check for node_modules, etc. on host
        let mut polluted = false;
        for artifact in &["node_modules", ".next", "dist", "build", "target", ".venv"] {
            if Path::new(artifact).exists() {
                polluted = true;
                break;
            }
        }

        if polluted {
            parts.push(format!(
                "💀{}({})",
                name.red().bold(),
                "POLLUTED".on_red().white().bold()
            ));
        } else {
            parts.push(name.bright_cyan().bold().to_string());
        }

        // Docker indicator
        parts.push("🐳".into());
    }

    print!("{}", parts.join(" "));
    Ok(())
}
