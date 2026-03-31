use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::manifest::Manifest;

/// Sync pnpm-lock.yaml after package.json updates.
/// Uses `--lockfile-only` to avoid installing into node_modules (fast).
/// Runs via Docker if mode is docker-first, otherwise directly.
///
/// In docker-first mode: tries `docker compose exec` first (fast, uses running container).
/// If the container is not running, falls back to `docker compose run --rm` (starts a
/// temporary container, slower but always works without requiring `airis up` first).
pub(super) fn sync_lockfile(manifest: &Manifest) -> Result<()> {
    use std::process::Command;

    // Only sync if pnpm-lock.yaml exists (skip for fresh projects)
    if !Path::new("pnpm-lock.yaml").exists() {
        return Ok(());
    }

    println!();
    println!("{}", "🔒 Syncing pnpm-lock.yaml...".bright_blue());

    let is_docker_first = matches!(manifest.mode, crate::manifest::Mode::DockerFirst);

    // Find a service to use
    let docker_service = manifest
        .docker
        .workspace
        .as_ref()
        .map(|w| w.service.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| manifest.service.keys().next().map(|s| s.as_str()));

    let status = if is_docker_first {
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
            .args([
                "compose",
                "exec",
                "-T",
                svc,
                "pnpm",
                "install",
                "--lockfile-only",
            ])
            .status();

        match exec_status {
            Ok(s) if s.success() => Ok(s),
            _ => {
                // Container not running — use doppler + docker compose run to inject env vars
                println!(
                    "   {} container not running, trying with doppler...",
                    "↻".yellow()
                );
                let doppler_status = Command::new("doppler")
                    .args([
                        "run",
                        "--",
                        "docker",
                        "compose",
                        "run",
                        "--rm",
                        "--no-deps",
                        "-T",
                        svc,
                        "pnpm",
                        "install",
                        "--lockfile-only",
                    ])
                    .status();

                match doppler_status {
                    Ok(s) if s.success() => Ok(s),
                    _ => {
                        // Doppler not available — use lightweight docker run with base image
                        println!(
                            "   {} doppler unavailable, using docker run...",
                            "↻".yellow()
                        );
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
                }
            }
        }
    } else {
        // Non-docker mode: run directly on host
        Command::new("pnpm")
            .args(["install", "--lockfile-only"])
            .status()
    };

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
