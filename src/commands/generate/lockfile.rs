use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::manifest::Manifest;

/// Sync pnpm-lock.yaml after package.json updates.
/// Uses `--lockfile-only` to avoid installing into node_modules (fast).
/// Runs via Docker if mode is docker-first, otherwise directly.
///
/// In docker-first mode: tries `docker compose exec` first (fast, uses running container).
/// If the container is not running, falls back to secret-provider-wrapped `docker compose run`
/// or a lightweight `docker run` as a last resort.
pub(super) fn sync_lockfile(manifest: &Manifest) -> Result<()> {
    use std::process::Command;

    // Only sync if pnpm-lock.yaml exists (skip for fresh projects)
    if !Path::new("pnpm-lock.yaml").exists() {
        return Ok(());
    }

    println!();
    println!("{}", "🔒 Syncing pnpm-lock.yaml...".bright_blue());

    // Find a service to use
    let docker_service = manifest
        .docker
        .workspace
        .as_ref()
        .map(|w| w.service.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| manifest.service.keys().next().map(|s| s.as_str()));

    let svc = match docker_service {
        Some(s) => s,
        None => {
            println!(
                "   {} no service found for lockfile sync (run `docker compose exec <service> pnpm install --lockfile-only`)",
                "⚠".yellow()
            );
            return Ok(());
        }
    };

    // Try exec first (fast, uses running container)
    let exec_status = Command::new("docker")
        .args(["compose", "exec", svc, "pnpm", "install", "--lockfile-only"])
        .status();

    let status = match exec_status {
        Ok(s) if s.success() => Ok(s),
        _ => {
            // Container not running — try with secret provider if configured
            let provider = manifest
                .secrets
                .as_ref()
                .and_then(|s| crate::secrets::create_provider(s).ok());

            if let Some(ref provider) = provider
                && provider.is_available()
            {
                println!(
                    "   {} container not running, trying with {}...",
                    "↻".yellow(),
                    provider.name()
                );
                let base_args = [
                    "compose",
                    "run",
                    "--rm",
                    "--no-deps",
                    svc,
                    "pnpm",
                    "install",
                    "--lockfile-only",
                ];
                let (program, wrapped_args) = provider.wrap_command("docker", &base_args);
                let provider_status = Command::new(&program).args(&wrapped_args).status();

                match provider_status {
                    Ok(s) if s.success() => return report_status(Ok(s)),
                    _ => {} // fall through to docker run
                }
            }

            // Last resort — use lightweight docker run with base image
            println!("   {} using docker run fallback...", "↻".yellow());
            let pm = &manifest.workspace.package_manager;
            let image = &manifest.workspace.image;
            Command::new("docker")
                .args([
                    "run",
                    "--rm",
                    "-v",
                    &format!("{}:/app", std::env::current_dir()?.display()),
                    "-w",
                    "/app",
                    image,
                    "sh",
                    "-c",
                    &format!("npm install -g {} && pnpm install --lockfile-only", pm),
                ])
                .status()
        }
    };

    report_status(status)
}

fn report_status(status: std::io::Result<std::process::ExitStatus>) -> Result<()> {
    match status {
        Ok(s) if s.success() => {
            println!("   {} pnpm-lock.yaml synced", "✓".green());
        }
        Ok(_) => {
            println!(
                "   {} pnpm-lock.yaml sync failed (run `docker compose run --rm <service> pnpm install --lockfile-only`)",
                "⚠".yellow()
            );
        }
        Err(e) => {
            println!("   {} pnpm-lock.yaml sync skipped: {}", "⚠".yellow(), e);
        }
    }

    Ok(())
}
