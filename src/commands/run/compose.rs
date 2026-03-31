use anyhow::{Context, Result, bail};
use colored::Colorize;
use glob::glob;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::manifest::Manifest;

use super::services::display_service_urls;
use super::{DB_HEALTH_RETRIES, DB_HEALTH_SLEEP_SECS};

/// Find the compose file in the current directory.
/// Checks in Docker's official priority order:
/// compose.yaml > compose.yml > docker-compose.yaml > docker-compose.yml
pub(super) fn find_compose_file() -> Option<&'static str> {
    if Path::new("compose.yaml").exists() {
        return Some("compose.yaml");
    }
    if Path::new("compose.yml").exists() {
        return Some("compose.yml");
    }
    // Legacy naming (backwards compatibility)
    if Path::new("docker-compose.yaml").exists() {
        return Some("docker-compose.yaml");
    }
    if Path::new("docker-compose.yml").exists() {
        return Some("docker-compose.yml");
    }
    None
}

/// Copy `.env.example` to `.env` if `.env` does not exist (idempotent)
pub(super) fn ensure_env_file() {
    let env_path = Path::new(".env");
    let example_path = Path::new(".env.example");

    if !env_path.exists() && example_path.exists() {
        match std::fs::copy(example_path, env_path) {
            Ok(_) => println!(
                "   {} Copied {} → {}",
                "📋".dimmed(),
                ".env.example".dimmed(),
                ".env".bold()
            ),
            Err(e) => println!("   {} Failed to copy .env.example: {}", "⚠️".yellow(), e),
        }
    }
}

/// Run post_up hooks from manifest (idempotent, warns on failure)
pub(super) fn run_post_up(manifest: &Manifest) {
    let hooks = &manifest.dev.post_up;
    if hooks.is_empty() {
        return;
    }

    println!("\n{}", "🔧 Running post_up hooks...".cyan().bold());
    for hook in hooks {
        println!("   {} {}", "→".dimmed(), hook.dimmed());
        let status = Command::new("sh").arg("-c").arg(hook).status();

        match status {
            Ok(s) if s.success() => {
                println!("   {} Done", "✅".green());
            }
            Ok(s) => {
                println!(
                    "   {} post_up hook failed (exit {}): {}",
                    "⚠️".yellow(),
                    s.code().unwrap_or(-1),
                    hook
                );
            }
            Err(e) => {
                println!("   {} post_up hook error: {} — {}", "⚠️".yellow(), hook, e);
            }
        }
    }
}

/// Execute a shell command and return success status
pub(super) fn exec_command(cmd: &str) -> Result<bool> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", cmd]).status()
    } else {
        Command::new("sh").arg("-c").arg(cmd).status()
    }
    .with_context(|| format!("Failed to execute: {}", cmd))?;

    Ok(status.success())
}

/// Smart compose up: reuses existing containers if already running
/// Based on compose_up.py logic
pub(super) fn smart_compose_up(
    project: Option<&str>,
    compose_files: &[&str],
    extra_args: &[String],
) -> Result<bool> {
    // Validate that all compose files exist first
    for file in compose_files {
        let path = Path::new(file);
        if !path.exists() {
            eprintln!(
                "{}\n\n\n{}\n",
                format!("❌ Docker Compose file not found: {}", file)
                    .red()
                    .bold(),
                "💡 Tip: Check your manifest.toml [dev] section or ensure the file exists."
                    .yellow()
            );
            return Ok(false);
        }
    }

    // Build file arguments
    let file_args: Vec<String> = compose_files
        .iter()
        .flat_map(|f| vec!["-f".to_string(), f.to_string()])
        .collect();

    // Build project argument
    let mut cmd_args = vec!["compose"];
    if let Some(proj) = project {
        cmd_args.extend(&["-p", proj]);
    }
    cmd_args.extend(file_args.iter().map(|s| s.as_str()));

    // Get config to extract container names
    let mut config_args = cmd_args.clone();
    config_args.extend(&["config", "--format", "json"]);

    let output = Command::new("docker")
        .args(&config_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    // Check existing containers
    if let Ok(output) = output {
        if !output.status.success() {
            // Docker compose config failed - likely invalid YAML
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!(
                "{}\n\n{}\n{}\n\n{}\n",
                format!(
                    "❌ Invalid Docker Compose file(s): {}",
                    compose_files.join(", ")
                )
                .red()
                .bold(),
                "Docker error:".yellow(),
                stderr,
                "💡 Check your compose file syntax and network configurations.".yellow()
            );
            return Ok(false);
        }
        if output.status.success()
            && let Ok(config) = serde_json::from_slice::<Value>(&output.stdout)
            && let Some(services) = config.get("services").and_then(|s| s.as_object())
        {
            let mut not_running = Vec::new();

            for service in services.values() {
                if let Some(container_name) = service.get("container_name").and_then(|c| c.as_str())
                {
                    // Check if container is running
                    let inspect = Command::new("docker")
                        .args(["inspect", "-f", "{{.State.Running}}", container_name])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output();

                    let is_running = if let Ok(inspect_output) = inspect {
                        inspect_output.status.success()
                            && String::from_utf8_lossy(&inspect_output.stdout).trim() == "true"
                    } else {
                        false
                    };

                    if !is_running {
                        not_running.push(container_name.to_string());
                    }
                }
            }

            if !not_running.is_empty() {
                for name in &not_running {
                    println!(
                        "     {} detected stopped/missing container: {}",
                        "🔍".dimmed(),
                        name.dimmed()
                    );
                }
            } else {
                println!(
                    "     {} containers already running; refreshing...",
                    "✓".dimmed()
                );
            }
        }
    }

    // Execute docker compose up -d --remove-orphans (+ extra args)
    let mut up_args = cmd_args.clone();
    up_args.extend(&["up", "-d", "--build", "--remove-orphans"]);
    for arg in extra_args {
        up_args.push(arg.as_str());
    }

    let output = Command::new("docker")
        .args(&up_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| "Failed to execute docker compose up".to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", "❌ Docker Compose up failed:".red().bold());
        eprintln!("{}", stderr);
        return Ok(false);
    }

    Ok(true)
}

/// Collect all compose files from manifest configuration
pub(super) fn collect_all_compose_files(manifest: &Manifest) -> Vec<String> {
    let mut files = Vec::new();

    // Check orchestration.dev first
    if let Some(dev) = &manifest.orchestration.dev {
        if let Some(workspace) = &dev.workspace
            && Path::new(workspace.as_str()).exists()
        {
            files.push(workspace.clone());
        }
        if let Some(supabase_files) = &dev.supabase {
            for f in supabase_files {
                if Path::new(f.as_str()).exists() && !files.contains(f) {
                    files.push(f.clone());
                }
            }
        }
        if let Some(traefik) = &dev.traefik
            && Path::new(traefik.as_str()).exists()
            && !files.contains(traefik)
        {
            files.push(traefik.clone());
        }
    }

    // Root compose file (if not already added)
    if let Some(compose_file) = find_compose_file() {
        let cf = compose_file.to_string();
        if !files.contains(&cf) {
            files.push(cf);
        }
    }

    // Dev section compose files
    if let Some(supabase_files) = &manifest.dev.supabase {
        for f in supabase_files {
            if Path::new(f.as_str()).exists() && !files.contains(f) {
                files.push(f.clone());
            }
        }
    }
    if let Some(traefik_file) = &manifest.dev.traefik
        && Path::new(traefik_file.as_str()).exists()
        && !files.contains(traefik_file)
    {
        files.push(traefik_file.clone());
    }

    // Apps pattern glob
    if let Ok(entries) = glob(&manifest.dev.apps_pattern) {
        for entry in entries.flatten() {
            if let Some(path_str) = entry.to_str() {
                let path_string = path_str.to_string();
                if !files.contains(&path_string) {
                    files.push(path_string);
                }
            }
        }
    }

    files
}

/// Orchestrated startup: supabase -> workspace -> apps
pub(super) fn orchestrated_up(manifest: &Manifest, extra_args: &[String]) -> Result<()> {
    ensure_env_file();
    let dev = &manifest.dev;

    // 1. Start Supabase (if configured)
    if let Some(supabase_files) = &dev.supabase {
        println!("{}", "📦 Starting Supabase...".cyan().bold());
        let files: Vec<&str> = supabase_files.iter().map(|s| s.as_str()).collect();

        if !smart_compose_up(None, &files, &[])? {
            bail!("❌ Failed to start Supabase");
        }

        // Wait for Supabase DB to be healthy
        println!(
            "   {} Waiting for Supabase DB to be healthy...",
            "⏳".dimmed()
        );
        let compose_file = supabase_files
            .first()
            .map(|s| s.as_str())
            .unwrap_or("supabase/compose.yml");
        let health_check = format!(
            "docker compose -f {} exec -T db pg_isready -U postgres -h localhost",
            compose_file
        );
        let mut retries = DB_HEALTH_RETRIES;
        while retries > 0 {
            if exec_command(&health_check)? {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(DB_HEALTH_SLEEP_SECS));
            retries -= 1;
        }
        if retries == 0 {
            bail!("Supabase DB health check timed out after 60s. Check `docker compose logs db`");
        }
        println!("   {} Supabase DB is healthy", "✅".green());
    }

    // 2. Start Traefik (if configured)
    if let Some(traefik) = &dev.traefik {
        println!("{}", "🔀 Starting Traefik...".cyan().bold());

        if !smart_compose_up(None, &[traefik.as_str()], &[])? {
            bail!(
                "Traefik failed to start. Check `docker compose -f {} logs`",
                traefik
            );
        }
    }

    // 3. Start workspace container (root compose.yml or compose.yml)
    if let Some(compose_file) = find_compose_file() {
        println!("{}", "🛠️  Starting workspace...".cyan().bold());

        if !smart_compose_up(None, &[compose_file], extra_args)? {
            bail!("Workspace failed to start. Check `docker compose logs`");
        }
    }

    // 4. Start apps using autodiscovery (apps_pattern)
    let apps_pattern = &dev.apps_pattern;

    // Collect compose files via glob
    let mut compose_files: Vec<String> = Vec::new();

    if let Ok(entries) = glob(apps_pattern) {
        for entry in entries.flatten() {
            if let Some(path_str) = entry.to_str() {
                compose_files.push(path_str.to_string());
            }
        }
    }

    // Sort for consistent ordering
    compose_files.sort();

    if !compose_files.is_empty() {
        println!("{}", "🚀 Starting apps...".cyan().bold());
        println!(
            "   {} Found {} apps via pattern: {}",
            "🔍".dimmed(),
            compose_files.len(),
            apps_pattern.dimmed()
        );

        for compose_path in &compose_files {
            // Extract app name from path (apps/foo/compose.yml -> foo)
            let app_name = Path::new(compose_path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            println!("   {} Starting {}...", "→".dimmed(), app_name.bold());

            if smart_compose_up(None, &[compose_path.as_str()], extra_args)? {
                println!("   {} {} started", "✅".green(), app_name);
            } else {
                println!("   {} {} failed to start", "⚠️".yellow(), app_name);
            }
        }
    }

    println!("\n{}", "✅ All services started!".green().bold());

    // Run post_up hooks (e.g., DB migration)
    run_post_up(manifest);

    // Display URLs - either from manifest config or auto-discover from compose files
    display_service_urls(manifest)?;

    Ok(())
}

/// Orchestrated shutdown: apps -> workspace -> supabase
pub(super) fn orchestrated_down(manifest: &Manifest) -> Result<()> {
    let dev = &manifest.dev;

    // 1. Stop apps (autodiscovery, reverse order)
    let apps_pattern = &dev.apps_pattern;

    let mut compose_files: Vec<String> = Vec::new();
    if let Ok(entries) = glob(apps_pattern) {
        for entry in entries.flatten() {
            if let Some(path_str) = entry.to_str() {
                compose_files.push(path_str.to_string());
            }
        }
    }
    compose_files.sort();
    compose_files.reverse(); // Stop in reverse order

    if !compose_files.is_empty() {
        println!("{}", "🛑 Stopping apps...".cyan().bold());

        for compose_path in &compose_files {
            let app_name = Path::new(compose_path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let cmd = format!("docker compose -f {} down --remove-orphans", compose_path);
            let _ = exec_command(&cmd);
            println!("   {} {} stopped", "✅".green(), app_name);
        }
    }

    // 2. Stop workspace (root compose.yml or compose.yml)
    if let Some(compose_file) = find_compose_file() {
        println!("{}", "🛑 Stopping workspace...".cyan().bold());
        let cmd = format!("docker compose -f {} down --remove-orphans", compose_file);
        let _ = exec_command(&cmd);
    }

    // 3. Stop Traefik
    if let Some(traefik) = &dev.traefik {
        println!("{}", "🛑 Stopping Traefik...".cyan().bold());
        let cmd = format!("docker compose -f {} down --remove-orphans", traefik);
        let _ = exec_command(&cmd);
    }

    // 4. Stop Supabase
    if let Some(supabase_files) = &dev.supabase {
        println!("{}", "🛑 Stopping Supabase...".cyan().bold());
        let files: Vec<String> = supabase_files.iter().map(|f| format!("-f {}", f)).collect();
        let cmd = format!("docker compose {} down --remove-orphans", files.join(" "));
        let _ = exec_command(&cmd);
    }

    println!("\n{}", "✅ All services stopped!".green().bold());
    Ok(())
}
