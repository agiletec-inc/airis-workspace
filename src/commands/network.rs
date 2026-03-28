use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

use crate::manifest::Manifest;

/// Network types to create
#[derive(Debug, Clone)]
struct NetworkConfig {
    /// Suffix for network name (e.g., "_default", "-services", "-proxy")
    suffix: &'static str,
    /// Description for user output
    description: &'static str,
}

/// Default networks to create for a workspace
fn default_networks() -> Vec<NetworkConfig> {
    vec![
        NetworkConfig {
            suffix: "_default",
            description: "Main application network",
        },
        NetworkConfig {
            suffix: "-services",
            description: "Internal services network (Kong, Supabase, etc.)",
        },
        NetworkConfig {
            suffix: "-proxy",
            description: "Reverse proxy network (Traefik, etc.)",
        },
    ]
}

/// Check if a Docker network exists
fn network_exists(name: &str) -> Result<bool> {
    let output = Command::new("docker")
        .args(["network", "ls", "--format", "{{.Name}}"])
        .output()
        .with_context(|| "Failed to list Docker networks")?;

    if !output.status.success() {
        bail!("Failed to list Docker networks");
    }

    let networks = String::from_utf8_lossy(&output.stdout);
    Ok(networks.lines().any(|n| n == name))
}

/// Create a Docker network
fn create_network(name: &str) -> Result<()> {
    let status = Command::new("docker")
        .args(["network", "create", name])
        .status()
        .with_context(|| format!("Failed to create network: {}", name))?;

    if !status.success() {
        bail!("Failed to create network: {}", name);
    }

    Ok(())
}

/// Initialize Docker networks for the workspace
pub fn init() -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    let project_name = &manifest.workspace.name;

    println!("🌐 Initializing Docker networks for project: {}", project_name.cyan());

    let networks = default_networks();
    let mut created = 0;
    let mut skipped = 0;

    for network in &networks {
        let network_name = format!("{}{}", project_name, network.suffix);

        if network_exists(&network_name)? {
            println!("  {} {} (already exists)", "⏭".yellow(), network_name);
            skipped += 1;
        } else {
            create_network(&network_name)?;
            println!("  {} {} - {}", "✓".green(), network_name, network.description);
            created += 1;
        }
    }

    println!();
    if created > 0 {
        println!("✅ Created {} network(s), skipped {} existing", created, skipped);
    } else {
        println!("✅ All networks already exist ({} total)", skipped);
    }

    Ok(())
}

/// List Docker networks for the workspace
pub fn list() -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    let project_name = &manifest.workspace.name;

    // Get all networks
    let output = Command::new("docker")
        .args(["network", "ls", "--format", "{{.Name}}\t{{.Driver}}\t{{.Scope}}"])
        .output()
        .with_context(|| "Failed to list Docker networks")?;

    if !output.status.success() {
        bail!("Failed to list Docker networks");
    }

    let networks = String::from_utf8_lossy(&output.stdout);

    println!("🌐 Networks for project: {}", project_name.cyan());
    println!();
    println!("{:<40} {:<10} {}", "NAME".bold(), "DRIVER".bold(), "SCOPE".bold());

    let mut found = 0;
    for line in networks.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 3 {
            let name = parts[0];
            // Show networks that start with project name
            if name.starts_with(project_name) {
                println!("{:<40} {:<10} {}", name.green(), parts[1], parts[2]);
                found += 1;
            }
        }
    }

    if found == 0 {
        println!("  {} No networks found. Run {} to create them.", "⚠".yellow(), "airis network init".bold());
    }

    Ok(())
}

/// Setup development networks and Traefik
/// This is equivalent to bootstrap-mac-dev.sh
pub fn setup() -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    let project_name = &manifest.workspace.name;
    // Resolve proxy network: manifest > env var > skip
    let proxy_network = manifest.orchestration.networks.as_ref()
        .and_then(|n| n.proxy.clone())
        .or_else(|| std::env::var("EXTERNAL_PROXY_NETWORK").ok());

    println!("🚀 Setting up development environment...");
    println!();

    // 1. Create proxy network (from manifest or env var, skip if not configured)
    println!("{}", "Creating proxy network...".bright_blue());
    if let Some(ref proxy) = proxy_network {
        if network_exists(proxy)? {
            println!("  {} {} (already exists)", "✓".green(), proxy);
        } else {
            create_network(proxy)?;
            println!("  {} {} (created)", "✓".green(), proxy);
        }
    } else {
        println!("  {} skipped (no proxy network configured in manifest or EXTERNAL_PROXY_NETWORK)", "⏭️".dimmed());
    }

    // 2. Create project networks
    println!();
    println!("{}", "Creating project networks...".bright_blue());

    let networks = default_networks();
    for network in &networks {
        let network_name = format!("{}{}", project_name, network.suffix);

        if network_exists(&network_name)? {
            println!("  {} {} (already exists)", "✓".green(), network_name);
        } else {
            create_network(&network_name)?;
            println!("  {} {} (created)", "✓".green(), network_name);
        }
    }

    // 3. Start Traefik (check modern + legacy naming)
    let traefik_compose = ["traefik/compose.yml", "traefik/compose.yaml",
        "traefik/docker-compose.yml", "traefik/docker-compose.yaml"]
        .iter().find(|p| Path::new(p).exists());
    if let Some(&traefik_path) = traefik_compose {
        let _traefik_compose = Path::new(traefik_path);
        println!();
        println!("{}", "Starting Traefik...".bright_blue());

        let proxy_env = proxy_network.as_deref().unwrap_or("bridge");
        let cmd = format!(
            "EXTERNAL_PROXY_NETWORK={} docker compose -f {} up -d",
            proxy_env, traefik_path
        );

        let status = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .status()
            .with_context(|| "Failed to start Traefik")?;

        if status.success() {
            println!("  {} Traefik started", "✓".green());
        } else {
            println!("  {} Traefik failed to start", "⚠".yellow());
        }
    }

    println!();
    println!("{}", "✅ Development environment ready!".green().bold());
    println!();
    println!("Dashboard: http://traefik.localhost");

    Ok(())
}

/// Remove Docker networks for the workspace
pub fn remove() -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    let project_name = &manifest.workspace.name;

    println!("🌐 Removing Docker networks for project: {}", project_name.cyan());

    let networks = default_networks();
    let mut removed = 0;
    let mut skipped = 0;

    for network in &networks {
        let network_name = format!("{}{}", project_name, network.suffix);

        if !network_exists(&network_name)? {
            println!("  {} {} (not found)", "⏭".yellow(), network_name);
            skipped += 1;
        } else {
            let status = Command::new("docker")
                .args(["network", "rm", &network_name])
                .status()
                .with_context(|| format!("Failed to remove network: {}", network_name))?;

            if status.success() {
                println!("  {} {}", "✓".green(), network_name);
                removed += 1;
            } else {
                println!("  {} {} (in use or failed)", "✗".red(), network_name);
            }
        }
    }

    println!();
    if removed > 0 {
        println!("✅ Removed {} network(s), skipped {}", removed, skipped);
    } else {
        println!("✅ No networks to remove");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_networks() {
        let networks = default_networks();
        assert_eq!(networks.len(), 3);
        assert_eq!(networks[0].suffix, "_default");
        assert_eq!(networks[1].suffix, "-services");
        assert_eq!(networks[2].suffix, "-proxy");
    }
}
