use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

use crate::manifest::Manifest;

/// Run the pre_command hook if configured and cache key has changed.
pub(super) fn ensure_pre_command(manifest: &Manifest) -> Result<()> {
    let pre_command = match &manifest.hooks.pre_command {
        Some(cmd) => cmd,
        None => return Ok(()),
    };

    if let Some(cache) = &manifest.hooks.cache {
        let key_path = Path::new(&cache.key);
        if key_path.exists() {
            let current_hash = hash_file(key_path)?;
            let hash_file_path = Path::new(".airis/hook-cache");
            if let Ok(saved) = std::fs::read_to_string(hash_file_path)
                && saved.trim() == current_hash
            {
                return Ok(());
            }
        }
    }

    println!(
        "{}",
        format!("📦 Running pre-command: {}", pre_command).bright_blue()
    );

    let success = execute_hook_command(pre_command, manifest)?;

    if success {
        if let Some(cache) = &manifest.hooks.cache {
            let key_path = Path::new(&cache.key);
            if key_path.exists() {
                let hash = hash_file(key_path)?;
                std::fs::create_dir_all(".airis").context("Failed to create .airis directory")?;
                std::fs::write(".airis/hook-cache", &hash).context("Failed to write hook cache")?;
            }
        }
        println!("{}", "  ✓ Pre-command completed".green());
    } else {
        println!("{}", "  ⚠ Pre-command failed (continuing anyway)".yellow());
    }

    Ok(())
}

/// Compute BLAKE3 hash of a file.
pub(super) fn hash_file(path: &Path) -> Result<String> {
    let content =
        std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
    Ok(blake3::hash(&content).to_hex()[..64].to_string())
}

/// Execute a hook command respecting Docker-first mode.
fn execute_hook_command(cmd: &str, manifest: &Manifest) -> Result<bool> {
    let is_docker_first = matches!(manifest.mode, crate::manifest::Mode::DockerFirst);

    if is_docker_first {
        let svc = manifest
            .docker
            .workspace
            .as_ref()
            .map(|w| w.service.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| manifest.service.keys().next().map(|s| s.as_str()));

        if let Some(svc) = svc {
            let status = Command::new("docker")
                .args(["compose", "exec", "-T", svc, "sh", "-c", cmd])
                .status();

            match status {
                Ok(s) if s.success() => return Ok(true),
                _ => {
                    println!(
                        "  {} container not running, starting temporary container...",
                        "↻".yellow()
                    );
                    let status = Command::new("docker")
                        .args([
                            "compose",
                            "run",
                            "--rm",
                            "--no-deps",
                            "-T",
                            svc,
                            "sh",
                            "-c",
                            cmd,
                        ])
                        .status()
                        .context("Failed to execute docker compose run")?;
                    return Ok(status.success());
                }
            }
        }
    }

    let status = Command::new("sh")
        .args(["-c", cmd])
        .status()
        .with_context(|| format!("Failed to execute hook: {}", cmd))?;
    Ok(status.success())
}
