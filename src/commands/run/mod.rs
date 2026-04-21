mod build_ops;
pub(crate) mod compose;
mod hooks;
mod monitoring;
mod services;
#[cfg(test)]
mod tests;
mod traefik;

use anyhow::{Context, Result, bail};
use colored::Colorize;
use glob::glob;
use indexmap::IndexMap;
use std::path::Path;
use std::process::Command;

use crate::manifest::Manifest;

use compose::{
    ensure_env_file, find_compose_file, orchestrated_down, orchestrated_up, run_post_up,
};
use hooks::ensure_pre_command;
use services::{display_compose_urls, display_service_urls};

// Re-export public API
pub use build_ops::{run_build_prod, run_build_quick, run_test_coverage};
pub use monitoring::{run_exec, run_logs, run_ps, run_restart};

// Internal timing constants for health probes
const TCP_CONNECT_TIMEOUT_MS: u64 = 200;
const TCP_READ_TIMEOUT_MS: u64 = 300;
const DB_HEALTH_RETRIES: u32 = 30;
const DB_HEALTH_SLEEP_SECS: u64 = 2;
// Polling interval for service reachability wait loop
const REACHABILITY_POLL_INTERVAL_SECS: u64 = 2;

/// Extract package manager command from manifest (e.g., "pnpm@latest" -> "pnpm")
#[cfg(test)]
fn get_package_manager(manifest: &Manifest) -> &str {
    let pm = &manifest.workspace.package_manager;
    if pm.starts_with("pnpm") {
        "pnpm"
    } else if pm.starts_with("bun") {
        "bun"
    } else if pm.starts_with("npm") {
        "npm"
    } else if pm.starts_with("yarn") {
        "yarn"
    } else {
        "pnpm" // default
    }
}

/// Build docker compose command with orchestration files
///
/// STRICT: Always requires explicit -f flag to prevent cwd-dependent behavior.
/// Returns Result to allow proper error handling when no compose file is found.
fn build_compose_command(manifest: &Manifest, base_cmd: &str) -> Result<String> {
    // Check if orchestration.dev is configured
    if let Some(dev) = &manifest.orchestration.dev {
        let mut compose_files = Vec::new();

        if let Some(workspace) = &dev.workspace {
            compose_files.push(format!("-f {}", workspace));
        }

        if let Some(supabase) = &dev.supabase {
            for file in supabase {
                compose_files.push(format!("-f {}", file));
            }
        }

        if let Some(traefik) = &dev.traefik {
            compose_files.push(format!("-f {}", traefik));
        }

        if !compose_files.is_empty() {
            return Ok(format!(
                "docker compose {} {}",
                compose_files.join(" "),
                base_cmd
            ));
        }
    }

    if let Some(compose_file) = find_compose_file() {
        return Ok(format!("docker compose -f {} {}", compose_file, base_cmd));
    }

    bail!(
        "No compose file found.\n\n\
         Expected: compose.yml (or compose.yml) or [orchestration.dev] config in manifest.toml\n\
         Verify:   airis manifest json\n\
         Generate: airis gen"
    );
}

/// Default commands - CLI is the source of truth, manifest can override
fn default_commands(manifest: &Manifest) -> Result<IndexMap<String, String>> {
    let mut cmds = IndexMap::new();

    let is_rust_project =
        !manifest.project.rust_edition.is_empty() || !manifest.project.binary_name.is_empty();

    if is_rust_project {
        cmds.insert("build".to_string(), "cargo build --release".to_string());
        cmds.insert("test".to_string(), "cargo test".to_string());
        cmds.insert("lint".to_string(), "cargo clippy".to_string());
        cmds.insert("format".to_string(), "cargo fmt".to_string());
    } else {
        cmds.insert(
            "up".to_string(),
            build_compose_command(manifest, "up -d --build --remove-orphans")?,
        );
        cmds.insert(
            "down".to_string(),
            build_compose_command(manifest, "down --remove-orphans")?,
        );
        cmds.insert("ps".to_string(), build_compose_command(manifest, "ps")?);
    }

    Ok(cmds)
}

/// Check if orchestration is configured in manifest
fn has_orchestration(manifest: &Manifest) -> bool {
    let dev = &manifest.dev;
    if dev.supabase.is_some() || dev.traefik.is_some() {
        return true;
    }
    if !dev.apps_pattern.is_empty()
        && let Ok(mut entries) = glob(&dev.apps_pattern)
    {
        return entries.next().is_some();
    }
    false
}

/// Match input against [remap] table entries.
fn find_remap_match(remap: &IndexMap<String, String>, input: &str) -> Option<(String, String)> {
    let lower = input.to_lowercase();

    for (key, value) in remap {
        if lower == key.to_lowercase() {
            return Some((key.clone(), value.clone()));
        }
    }

    for (key, value) in remap {
        let key_lower = key.to_lowercase();
        if lower.starts_with(&key_lower)
            && lower[key_lower.len()..].starts_with(|c: char| c.is_whitespace())
        {
            return Some((key.clone(), value.clone()));
        }
    }

    None
}

/// Execute a command defined in manifest.toml [commands] section
pub fn run(task: &str, extra_args: &[String]) -> Result<()> {
    for arg in extra_args {
        if arg.contains(';')
            || arg.contains("&&")
            || arg.contains("||")
            || arg.contains('|')
            || arg.contains('`')
            || arg.contains("$(")
        {
            bail!(
                "❌ Shell metacharacters are not allowed in extra arguments: {}\n\
                 This restriction prevents host command injection.\n\
                 If you need complex commands, define them in manifest.toml [commands].",
                arg.bold()
            );
        }
    }

    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        if let Some(compose_file) = find_compose_file() {
            if matches!(task, "up" | "down") {
                if task == "up" {
                    ensure_env_file();
                }
                let action = if task == "up" {
                    "up -d --build --remove-orphans"
                } else {
                    "down"
                };
                let extra = if extra_args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", extra_args.join(" "))
                };
                let cmd = format!("docker compose -f {} {}{}", compose_file, action, extra);

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
                if task == "up" {
                    println!("\n{}", "✅ All services started!".green().bold());
                    display_compose_urls(&[compose_file.to_string()]);
                }
                return Ok(());
            } else {
                // Delegate to docker compose exec
                let services_output = Command::new("docker")
                    .args(["compose", "-f", compose_file, "ps", "--services"])
                    .output()?;
                
                let services = String::from_utf8_lossy(&services_output.stdout);
                let service_list: Vec<&str> = services
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .collect();
                
                if service_list.is_empty() {
                    bail!("No services found in {}", compose_file);
                }

                // Pick "workspace" if it exists, otherwise the first one
                let target_service = if service_list.contains(&"workspace") {
                    "workspace"
                } else {
                    service_list[0]
                };

                let extra = if extra_args.is_empty() {
                    String::new()
                } else {
                    format!(" {}", extra_args.join(" "))
                };

                let cmd = format!("docker compose -f {} exec -it {} {} {}", compose_file, target_service, task, extra);
                println!("🚀 Delegating to Docker: {}", cmd.cyan());

                let status = if cfg!(target_os = "windows") {
                    Command::new("cmd").args(["/C", &cmd]).status()
                } else {
                    Command::new("sh").arg("-c").arg(&cmd).status()
                }
                .with_context(|| format!("Failed to execute: {}", cmd))?;

                if !status.success() {
                    // Try 'run' if 'exec' fails (container might be stopped)
                    let run_cmd = format!("docker compose -f {} run --rm {} {} {}", compose_file, target_service, task, extra);
                    println!("⚠️  Exec failed, trying run: {}", run_cmd.yellow());
                    
                    let status = if cfg!(target_os = "windows") {
                        Command::new("cmd").args(["/C", &run_cmd]).status()
                    } else {
                        Command::new("sh").arg("-c").arg(&run_cmd).status()
                    }
                    .with_context(|| format!("Failed to execute: {}", run_cmd))?;

                    if !status.success() {
                        bail!("Command failed with exit code: {:?}", status.code());
                    }
                }
                return Ok(());
            }
        }

        bail!(
            "❌ manifest.toml not found. Create one (see docs/manifest.md) or ask Claude Code via {}.",
            "/airis:init".bold()
        );
    }

    let manifest = match Manifest::load(manifest_path) {
        Ok(m) => m,
        Err(_) => {
            // If strict load fails, try loose load and continue with a warning
            Manifest::load_loose(manifest_path).with_context(|| "Critical failure loading manifest.toml")?
        }
    };

    // Auto-converge: Ensure workspace is ready before starting Docker-First environment
    if task == "up" {
        println!(
            "{}",
            "⚓ Docker-First initialization starting..."
                .bright_cyan()
                .bold()
        );

        // 1. Sync manifest -> generated files (gen)
        println!("   {} Syncing workspace configuration...", "🔄".cyan());
        if let Err(e) = crate::commands::generate::sync_from_manifest(&manifest) {
            eprintln!("\n{} Workspace sync partially failed: {}", "⚠️".yellow(), e);
            eprintln!("   Continuing to start environment anyway...\n");
        }

        // 2. Sync dependencies inside Docker (install)
        println!(
            "   {} Syncing dependencies inside container...",
            "📦".blue()
        );
        let _ = crate::commands::install::run(&[]);
        println!();
    }

    // Create secret provider if configured
    let secret_provider: Option<Box<dyn crate::secrets::SecretProvider>> =
        if let Some(ref secrets) = manifest.secrets {
            let provider = crate::secrets::create_provider(secrets)?;
            if !provider.is_available() {
                eprintln!(
                    "   {} secrets provider '{}' is configured but not available on this system",
                    "⚠️".yellow(),
                    provider.name()
                );
                None
            } else {
                Some(provider)
            }
        } else {
            None
        };

    if !manifest.commands.contains_key(task) && has_orchestration(&manifest) {
        match task {
            "up" => {
                return orchestrated_up(
                    &manifest,
                    extra_args,
                    secret_provider.as_ref().map(|p| p.as_ref()),
                );
            }
            "down" => return orchestrated_down(&manifest),
            _ => {}
        }
    }

    let mut commands = default_commands(&manifest)?;
    for (key, value) in manifest.commands.iter() {
        commands.insert(key.clone(), value.clone());
    }

    let cmd = match commands.get(task) {
        Some(cmd) => cmd.clone(),
        None => {
            let full_input = if extra_args.is_empty() {
                task.to_string()
            } else {
                format!("{} {}", task, extra_args.join(" "))
            };

            if let Some((from, to)) = find_remap_match(&manifest.remap, &full_input) {
                eprintln!(
                    "🔄 Remapped: {} → {}",
                    from.yellow().strikethrough(),
                    to.green().bold()
                );

                if let Some(airis_cmd) = to.strip_prefix("airis ") {
                    let parts: Vec<&str> = airis_cmd.split_whitespace().collect();
                    let remapped_task = parts[0];
                    let remapped_args: Vec<String> =
                        parts[1..].iter().map(|s| s.to_string()).collect();
                    return run(remapped_task, &remapped_args);
                }

                to
            } else {
                bail!(
                    "❌ Command '{}' not found in manifest.toml [commands] section.\n\n\
                         Available commands:\n{}\n\n\
                         Hint: Check [remap] section for command translations.",
                    task.bold(),
                    commands
                        .keys()
                        .map(|k| format!("  - {}", k))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }
    };

    if task == "up" {
        ensure_env_file();
    }

    if !manifest.hooks.skip.contains(&task.to_string()) {
        ensure_pre_command(&manifest)?;
    }

    let full_cmd = if extra_args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, extra_args.join(" "))
    };

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

    if task == "up" {
        println!("\n{}", "✅ All services started!".green().bold());

        // Check health but don't force fix (let the user decide)
        println!("\n{}", "🛡️  Checking workspace boundaries...".dimmed());
        let _ = crate::commands::doctor::run(false);

        run_post_up(&manifest);
        display_service_urls(&manifest)?;
    }

    Ok(())
}
