//! Network validation: Traefik network wiring in application compose files

use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Validate Traefik network wiring in application docker-compose files
pub fn validate_networks() -> Result<()> {
    validate_networks_impl(false)
}

pub fn validate_networks_impl(quiet: bool) -> Result<()> {
    if !quiet {
        println!(
            "{}",
            "🔍 Checking Traefik network wiring in apps/*/compose.yml...".bright_blue()
        );
    }

    let apps_dir = Path::new("apps");
    if !apps_dir.exists() {
        if !quiet {
            println!("  {} No apps directory found", "⏭️".dimmed());
        }
        return Ok(());
    }

    let mut failures = 0;
    // Resolve proxy network from manifest > env var
    let manifest_proxy =
        crate::manifest::Manifest::load(std::path::Path::new(crate::manifest::MANIFEST_FILE))
            .ok()
            .and_then(|m| {
                m.orchestration
                    .networks
                    .as_ref()
                    .and_then(|n| n.proxy.clone())
            });
    let proxy_network = manifest_proxy
        .or_else(|| std::env::var("EXTERNAL_PROXY_NETWORK").ok())
        .unwrap_or_default();

    for entry in fs::read_dir(apps_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Check for compose file (modern + legacy naming)
        let compose_file = [
            "compose.yml",
            "compose.yaml",
            "docker-compose.yml",
            "docker-compose.yaml",
        ]
        .iter()
        .map(|name| path.join(name))
        .find(|p| p.exists());
        let Some(compose_file) = compose_file else {
            continue;
        };

        let project = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Read and parse the compose file
        let content = fs::read_to_string(&compose_file)
            .with_context(|| format!("Failed to read {}", compose_file.display()))?;

        // Check for required network configurations using simple string matching
        // A more robust solution would use a YAML parser

        // Check for workspace default network (derived from manifest workspace name)
        let workspace_network =
            crate::manifest::Manifest::load(std::path::Path::new(crate::manifest::MANIFEST_FILE))
                .map(|m| format!("{}_default", m.workspace.name))
                .unwrap_or_else(|_| "default".to_string());
        if !content.contains(&workspace_network) {
            if !quiet {
                println!(
                    "  {} {}: networks.default should reference '{}'",
                    "❌".red(),
                    project,
                    workspace_network
                );
            }
            failures += 1;
        }

        // Check for proxy network
        if !content.contains(&proxy_network) && !content.contains("EXTERNAL_PROXY_NETWORK") {
            if !quiet {
                println!(
                    "  {} {}: networks.proxy should reference '{}' or EXTERNAL_PROXY_NETWORK",
                    "❌".red(),
                    project,
                    proxy_network
                );
            }
            failures += 1;
        }

        // Check for traefik.docker.network label
        if content.contains("traefik.enable=true") && !content.contains("traefik.docker.network=") {
            if !quiet {
                println!(
                    "  {} {}: Traefik-enabled services need traefik.docker.network label",
                    "❌".red(),
                    project
                );
            }
            failures += 1;
        }
    }

    if failures > 0 {
        if !quiet {
            println!();
            println!("⚠️  Traefik ネットワーク設定を修正してください");
        }
        bail!("Found {} network configuration issues", failures);
    }

    if !quiet {
        println!("{}", "✅ Traefik network wiring looks good.".green());
    }
    Ok(())
}
