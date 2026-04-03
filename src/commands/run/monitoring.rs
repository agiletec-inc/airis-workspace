use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::manifest::Manifest;

use super::build_compose_command;
use super::compose::{collect_all_compose_files, find_compose_file};
use super::services::{condense_status, discover_compose_port_urls};

/// Show running services with status and workspace health summary
pub fn run_ps() -> Result<()> {
    let manifest_path = Path::new("manifest.toml");
    let manifest = if manifest_path.exists() {
        Some(Manifest::load(manifest_path)?)
    } else {
        None
    };

    // 1. Docker Services Status
    println!();
    println!(
        "{}",
        "── 🚀 Services ──────────────────────────────────────────".dimmed()
    );

    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}\t{{.Status}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .with_context(|| "Failed to execute docker ps")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let containers: Vec<(&str, &str)> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.splitn(2, '\t').collect();
            if parts.len() == 2 {
                Some((parts[0], parts[1]))
            } else {
                None
            }
        })
        .collect();

    if containers.is_empty() {
        println!("   {}", "No services are currently running.".yellow());
    } else {
        let url_map = {
            let port_services = if let Some(ref m) = manifest {
                let compose_files = collect_all_compose_files(m);
                discover_compose_port_urls(&compose_files)
            } else if let Some(compose_file) = find_compose_file() {
                discover_compose_port_urls(&[compose_file.to_string()])
            } else {
                Vec::new()
            };

            let mut map = std::collections::HashMap::new();
            for svc in &port_services {
                map.insert(svc.name.clone(), svc.url.clone());
            }
            map
        };

        for (name, status) in &containers {
            let condensed = condense_status(status);
            let icon = if status.contains("Up") {
                "✔".green()
            } else {
                "⚠".yellow()
            };
            if let Some(url) = url_map.get(*name) {
                println!(
                    "   {} {:<24} {:<12} {}",
                    icon,
                    name.bold(),
                    condensed.dimmed(),
                    url.cyan()
                );
            } else {
                println!("   {} {:<24} {:<12}", icon, name.bold(), condensed.dimmed());
            }
        }
    }

    // 2. Workspace Health Summary (Guardrails)
    if let Some(_m) = manifest {
        println!();
        println!(
            "{}",
            "── 🛡️  Workspace Health ─────────────────────────────────".dimmed()
        );

        // Host Contamination Check
        let has_node_modules = Path::new("node_modules").exists();
        let host_status = if has_node_modules {
            "⚠ CONTAMINATED (host node_modules detected)"
                .yellow()
                .bold()
        } else {
            "✔ CLEAN (host is protected)".green()
        };
        println!("   {:<12} {}", "Host:".dimmed(), host_status);

        // Guard Check
        let has_guards = Path::new(".airis/bin").exists();
        let guard_status = if has_guards {
            "✔ ACTIVE (command guards installed)".green()
        } else {
            "⚠ UNPROTECTED (guards missing)".red().bold()
        };
        println!("   {:<12} {}", "Guards:".dimmed(), guard_status);

        // Lock/Manifest Sync Check
        let lock_exists = Path::new(crate::manifest::LOCK_FILE).exists();
        let sync_status = if lock_exists {
            "✔ LOCKED (versions are pinned)".green()
        } else {
            "⚠ UNLOCKED (run 'airis gen' to lock)".yellow()
        };
        println!("   {:<12} {}", "Sync:".dimmed(), sync_status);

        if has_node_modules || !has_guards || !lock_exists {
            println!();
            println!(
                "   {} Run {} to heal your workspace.",
                "💡".yellow(),
                "airis doctor --fix".bold().cyan()
            );
        }
    }

    println!();
    Ok(())
}

/// Execute logs command with options
pub fn run_logs(service: Option<&str>, follow: bool, tail: Option<u32>) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path).with_context(|| "Failed to load manifest.toml")?;

    let mut args = vec!["logs".to_string()];

    if follow {
        args.push("-f".to_string());
    }

    if let Some(n) = tail {
        args.push(format!("--tail={}", n));
    }

    if let Some(svc) = service {
        args.push(svc.to_string());
    }

    let cmd = build_compose_command(&manifest, &args.join(" "))?;

    println!("🚀 Running: {}", cmd.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", &cmd]).status()
    } else {
        Command::new("sh").arg("-c").arg(&cmd).status()
    }
    .with_context(|| format!("Failed to execute: {}", cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Execute command in a service container
pub fn run_exec(service: &str, cmd: &[String]) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path).with_context(|| "Failed to load manifest.toml")?;

    if cmd.is_empty() {
        bail!("❌ No command specified. Usage: airis exec <service> <cmd>");
    }

    let exec_cmd = format!("exec {} {}", service, cmd.join(" "));
    let full_cmd = build_compose_command(&manifest, &exec_cmd)?;

    println!("🚀 Running: {}", full_cmd.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", &full_cmd]).status()
    } else {
        Command::new("sh").arg("-c").arg(&full_cmd).status()
    }
    .with_context(|| format!("Failed to execute: {}", full_cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Restart Docker services
pub fn run_restart(service: Option<&str>) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path).with_context(|| "Failed to load manifest.toml")?;

    let restart_cmd = match service {
        Some(svc) => format!("restart {}", svc),
        None => "restart".to_string(),
    };

    let full_cmd = build_compose_command(&manifest, &restart_cmd)?;

    println!("🚀 Running: {}", full_cmd.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", &full_cmd]).status()
    } else {
        Command::new("sh").arg("-c").arg(&full_cmd).status()
    }
    .with_context(|| format!("Failed to execute: {}", full_cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

    Ok(())
}
