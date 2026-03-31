//! Port validation: ensure no `ports:` mapping in application compose files

use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::process::Command;

/// Validate that no ports: mapping exists in application docker-compose files
pub fn validate_ports() -> Result<()> {
    validate_ports_impl(false)
}

pub fn validate_ports_impl(quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "{}",
            "🔍 Checking for ports: mapping in application docker-compose files...".bright_blue()
        );
    }

    // Use ripgrep to find ports: mappings
    let output = Command::new("rg")
        .args([
            "-n",
            r"^\s*ports\s*:",
            "--glob",
            "apps/*/compose*.yml",
            "--glob",
            "apps/*/docker-compose*.yml",
            "--glob",
            "!apps/*/compose.override*.yml",
            "--glob",
            "!apps/*/docker-compose.override*.yml",
            ".",
        ])
        .output()
        .context("Failed to run ripgrep")?;

    let matches = String::from_utf8_lossy(&output.stdout);

    if !matches.is_empty() {
        if !quiet {
            println!();
            println!(
                "{}",
                "❌ ERROR: Found ports: mapping in application docker-compose.".red()
            );
            println!();
            println!("Found:");
            for line in matches.lines() {
                println!("  {}", line);
            }
            println!();
            println!("   {} Wrong:", "❌".red());
            println!("   ports:");
            println!("     - \"4010:3000\"");
            println!();
            println!("   {} Right:", "✅".green());
            println!("   expose:");
            println!("     - \"3000\"");
            println!("   labels:");
            println!("     - traefik.enable=true");
            println!("     - traefik.http.routers.app.rule=Host(`app.localhost`)");
            println!("     - traefik.http.services.app.loadbalancer.server.port=3000");
            println!();
            println!("   Exception: Only allowed in:");
            println!("   - Infrastructure (traefik/, supabase/)");
            println!("   - Override files (compose.*.override.yml, compose.dev.yml)");
        }

        bail!("Found ports: mapping in application docker-compose files");
    }

    if !quiet {
        println!(
            "{}",
            "✅ No ports: mapping found in application docker-compose.".green()
        );
    }
    Ok(())
}
