//! Validate command: workspace configuration validation
//!
//! Validates Traefik ports, networks, and environment variables.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Validate action types
pub enum ValidateAction {
    Ports,
    Networks,
    Env,
    All,
}

/// Run validation
pub fn run(action: ValidateAction) -> Result<()> {
    match action {
        ValidateAction::Ports => validate_ports(),
        ValidateAction::Networks => validate_networks(),
        ValidateAction::Env => validate_env(),
        ValidateAction::All => {
            let mut failures = 0;

            println!("{}", "🔍 Running all validations...".bright_blue());
            println!();

            if let Err(e) = validate_ports() {
                eprintln!("  {} Ports validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if let Err(e) = validate_networks() {
                eprintln!("  {} Networks validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if let Err(e) = validate_env() {
                eprintln!("  {} Env validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if failures > 0 {
                bail!("Validation failed with {} errors", failures);
            }

            println!();
            println!("{}", "✅ All validations passed!".green());
            Ok(())
        }
    }
}

/// Validate that no ports: mapping exists in application docker-compose files
fn validate_ports() -> Result<()> {
    println!("{}", "🔍 Checking for ports: mapping in application docker-compose files...".bright_blue());

    // Use ripgrep to find ports: mappings
    let output = Command::new("rg")
        .args([
            "-n",
            r"^\s*ports\s*:",
            "--glob", "apps/*/docker-compose*.yml",
            "--glob", "!apps/*/docker-compose.override*.yml",
            ".",
        ])
        .output()
        .context("Failed to run ripgrep")?;

    let matches = String::from_utf8_lossy(&output.stdout);

    if !matches.is_empty() {
        println!();
        println!("{}", "❌ ERROR: Found ports: mapping in application docker-compose.".red());
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

        bail!("Found ports: mapping in application docker-compose files");
    }

    println!("{}", "✅ No ports: mapping found in application docker-compose.".green());
    Ok(())
}

/// Validate Traefik network wiring in application docker-compose files
fn validate_networks() -> Result<()> {
    println!("{}", "🔍 Checking Traefik network wiring in apps/*/docker-compose.yml...".bright_blue());

    let apps_dir = Path::new("apps");
    if !apps_dir.exists() {
        println!("  {} No apps directory found", "⏭️".dimmed());
        return Ok(());
    }

    let mut failures = 0;
    let proxy_network = std::env::var("EXTERNAL_PROXY_NETWORK").unwrap_or_else(|_| "coolify".to_string());

    for entry in fs::read_dir(apps_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let compose_file = path.join("docker-compose.yml");
        if !compose_file.exists() {
            continue;
        }

        let project = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Read and parse the compose file
        let content = fs::read_to_string(&compose_file)
            .with_context(|| format!("Failed to read {}", compose_file.display()))?;

        // Check for required network configurations using simple string matching
        // A more robust solution would use a YAML parser

        // Check for agiletec_default network
        if !content.contains("agiletec_default") {
            println!("  {} {}: networks.default should reference 'agiletec_default'", "❌".red(), project);
            failures += 1;
        }

        // Check for proxy network
        if !content.contains(&proxy_network) && !content.contains("EXTERNAL_PROXY_NETWORK") {
            println!("  {} {}: networks.proxy should reference '{}' or EXTERNAL_PROXY_NETWORK", "❌".red(), project, proxy_network);
            failures += 1;
        }

        // Check for traefik.docker.network label
        if content.contains("traefik.enable=true") {
            if !content.contains("traefik.docker.network=") {
                println!("  {} {}: Traefik-enabled services need traefik.docker.network label", "❌".red(), project);
                failures += 1;
            }
        }
    }

    if failures > 0 {
        println!();
        println!("⚠️  Traefik ネットワーク設定を修正してください");
        bail!("Found {} network configuration issues", failures);
    }

    println!("{}", "✅ Traefik network wiring looks good.".green());
    Ok(())
}

/// Validate frontend environment variables
fn validate_env() -> Result<()> {
    println!("{}", "🔍 Checking frontend environment variables...".bright_blue());

    let allowed_keys = vec![
        "NEXT_PUBLIC_SUPABASE_URL",
        "NEXT_PUBLIC_SUPABASE_ANON_KEY",
        "EXPO_PUBLIC_SUPABASE_URL",
        "EXPO_PUBLIC_SUPABASE_ANON_KEY",
    ];

    let mut disallowed = Vec::new();

    // Check .env files in apps
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        for entry in fs::read_dir(apps_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check various .env files
            for env_file in &[".env", ".env.local", ".env.development"] {
                let env_path = path.join(env_file);
                if env_path.exists() {
                    check_env_file(&env_path, &allowed_keys, &mut disallowed)?;
                }
            }
        }
    }

    if !disallowed.is_empty() {
        println!();
        println!("{}", "Disallowed public environment keys detected:".red());
        for item in &disallowed {
            println!("  - {}", item);
        }
        println!();
        println!("Allowed keys: {}", allowed_keys.join(", "));
        bail!("Found {} disallowed public environment keys", disallowed.len());
    }

    println!("{}", "✅ Environment variables look good.".green());
    Ok(())
}

/// Check a single .env file for disallowed public keys
fn check_env_file(path: &Path, allowed: &[&str], disallowed: &mut Vec<String>) -> Result<()> {
    let content = fs::read_to_string(path)?;

    for line in content.lines() {
        // Skip comments and empty lines
        if line.trim().starts_with('#') || line.trim().is_empty() {
            continue;
        }

        // Extract key
        if let Some(key) = line.split('=').next() {
            let key = key.trim();

            // Check if it's a public key
            if key.starts_with("NEXT_PUBLIC_") || key.starts_with("EXPO_PUBLIC_") {
                if !allowed.contains(&key) {
                    disallowed.push(format!("{}: {}", path.display(), key));
                }
            }
        }
    }

    Ok(())
}
