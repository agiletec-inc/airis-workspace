use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::manifest::Manifest;

/// Background task to monitor workspace health during 'airis up'
#[allow(dead_code)]
pub fn spawn_monitoring(project_id: String) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = check_health(&project_id) {
                eprintln!("{} Health check failed: {}", "⚠".yellow(), e);
            }
        }
    });
}

#[allow(dead_code)]
fn check_health(_project_id: &str) -> Result<()> {
    let manifest_path = Path::new(crate::manifest::MANIFEST_FILE);
    if !manifest_path.exists() {
        return Ok(());
    }
    Ok(())
}

/// Print a health summary for the user
#[allow(dead_code)]
pub fn print_health_summary(_manifest: &Manifest) -> Result<()> {
    println!();
    println!("{}", "🛡️  Workspace Health:".bright_blue());

    let docker_status = if is_docker_running() {
        "✔ RUNNING".green()
    } else {
        "✘ STOPPED".red().bold()
    };
    println!("   {:<12} {}", "Docker:".dimmed(), docker_status);

    let has_node_modules = Path::new("node_modules").exists();
    let hygiene_status = if has_node_modules {
        "⚠ LEAKED (host node_modules detected)".red().bold()
    } else {
        "✔ CLEAN (container isolated)".green()
    };
    println!("   {:<12} {}", "Hygiene:".dimmed(), hygiene_status);

    let has_guards = Path::new(".airis/bin").exists();
    let guard_status = if has_guards {
        "✔ ACTIVE (command guards installed)".green()
    } else {
        "⚠ UNPROTECTED (guards missing)".red().bold()
    };
    println!("   {:<12} {}", "Guards:".dimmed(), guard_status);

    let sync_status = if Path::new("pnpm-lock.yaml").exists() {
        "✔ LOCKED (via pnpm-lock.yaml)".green()
    } else {
        "⚠ UNLOCKED (run 'airis install')".yellow()
    };
    println!("   {:<12} {}", "Sync:".dimmed(), sync_status);

    if has_node_modules || !has_guards {
        println!();
        println!(
            "   {} Run {} to heal your workspace.",
            "💡".yellow(),
            "airis doctor --fix".cyan()
        );
    }
    println!();

    Ok(())
}

#[allow(dead_code)]
fn is_docker_running() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Docker helper functions used by run::dispatch

pub fn run_ps() -> Result<()> {
    let mut child = Command::new("docker")
        .args(["compose", "ps"])
        .spawn()
        .with_context(|| "Failed to run docker compose ps")?;
    child.wait()?;
    Ok(())
}

pub fn run_logs(service: Option<&str>, follow: bool, tail: Option<u32>) -> Result<()> {
    let mut args = vec!["compose", "logs"];
    if follow {
        args.push("-f");
    }
    if let Some(t) = tail {
        args.push("--tail");
        let t_str = t.to_string();
        args.push(Box::leak(t_str.into_boxed_str())); // Hack for static lifetime
    }
    if let Some(s) = service {
        args.push(s);
    }

    let mut child = Command::new("docker")
        .args(args)
        .spawn()
        .with_context(|| "Failed to run docker compose logs")?;
    child.wait()?;
    Ok(())
}

pub fn run_exec(service: &str, cmd: &[String]) -> Result<()> {
    let mut args = vec!["compose", "exec", service];
    for c in cmd {
        args.push(c);
    }

    let mut child = Command::new("docker")
        .args(args)
        .spawn()
        .with_context(|| "Failed to run docker compose exec")?;
    child.wait()?;
    Ok(())
}

pub fn run_restart(service: Option<&str>) -> Result<()> {
    let mut args = vec!["compose", "restart"];
    if let Some(s) = service {
        args.push(s);
    }

    let mut child = Command::new("docker")
        .args(args)
        .spawn()
        .with_context(|| "Failed to run docker compose restart")?;
    child.wait()?;
    Ok(())
}
