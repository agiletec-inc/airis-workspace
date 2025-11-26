use anyhow::{bail, Context, Result};
use chrono;
use colored::Colorize;
use glob::glob;
use indexmap::IndexMap;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::manifest::Manifest;

/// Extract package manager command from manifest (e.g., "pnpm@10.22.0" -> "pnpm")
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

/// Execute a shell command and return success status
fn exec_command(cmd: &str) -> Result<bool> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status()
    }
    .with_context(|| format!("Failed to execute: {}", cmd))?;

    Ok(status.success())
}

/// Smart compose up: reuses existing containers if already running
/// Based on compose_up.py logic
fn smart_compose_up(project: Option<&str>, compose_files: &[&str]) -> Result<bool> {
    // Validate that all compose files exist first
    for file in compose_files {
        let path = Path::new(file);
        if !path.exists() {
            eprintln!(
                "{}\n{}\n\n{}\n",
                format!("❌ Docker Compose file not found: {}", file).red().bold(),
                "",
                "💡 Tip: Check your manifest.toml [dev] section or ensure the file exists.".yellow()
            );
            return Ok(false);
        }
    }

    // Build file arguments
    let file_args: Vec<String> = compose_files.iter()
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
                format!("❌ Invalid Docker Compose file(s): {}", compose_files.join(", ")).red().bold(),
                "Docker error:".yellow(),
                stderr,
                "💡 Check your compose file syntax and network configurations.".yellow()
            );
            return Ok(false);
        }
        if output.status.success() {
            if let Ok(config) = serde_json::from_slice::<Value>(&output.stdout) {
                if let Some(services) = config.get("services").and_then(|s| s.as_object()) {
                    let mut not_running = Vec::new();

                    for service in services.values() {
                        if let Some(container_name) = service.get("container_name").and_then(|c| c.as_str()) {
                            // Check if container is running
                            let inspect = Command::new("docker")
                                .args(&["inspect", "-f", "{{.State.Running}}", container_name])
                                .stdout(Stdio::piped())
                                .stderr(Stdio::null())
                                .output();

                            let is_running = if let Ok(inspect_output) = inspect {
                                inspect_output.status.success() &&
                                    String::from_utf8_lossy(&inspect_output.stdout).trim() == "true"
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
                            println!("     {} detected stopped/missing container: {}", "🔍".dimmed(), name.dimmed());
                        }
                    } else {
                        println!("     {} containers already running; refreshing...", "✓".dimmed());
                    }
                }
            }
        }
    }

    // Execute docker compose up -d --remove-orphans
    let mut up_args = cmd_args.clone();
    up_args.extend(&["up", "-d", "--remove-orphans"]);

    let output = Command::new("docker")
        .args(&up_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to execute docker compose up"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", "❌ Docker Compose up failed:".red().bold());
        eprintln!("{}", stderr);
        return Ok(false);
    }

    Ok(true)
}

/// Orchestrated startup: supabase -> workspace -> apps
fn orchestrated_up(manifest: &Manifest) -> Result<()> {
    let dev = &manifest.dev;

    // 1. Start Supabase (if configured)
    if let Some(supabase_files) = &dev.supabase {
        println!("{}", "📦 Starting Supabase...".cyan().bold());
        let files: Vec<&str> = supabase_files.iter().map(|s| s.as_str()).collect();

        if !smart_compose_up(None, &files)? {
            bail!("❌ Failed to start Supabase");
        }

        // Wait for Supabase DB to be healthy
        println!("   {} Waiting for Supabase DB to be healthy...", "⏳".dimmed());
        let health_check = "docker compose -f supabase/docker-compose.yml exec -T db pg_isready -U postgres -h localhost";
        let mut retries = 30;
        while retries > 0 {
            if exec_command(health_check).unwrap_or(false) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            retries -= 1;
        }
        if retries == 0 {
            println!("   {} Supabase DB health check timed out, continuing anyway...", "⚠️".yellow());
        } else {
            println!("   {} Supabase DB is healthy", "✅".green());
        }
    }

    // 2. Start Traefik (if configured)
    if let Some(traefik) = &dev.traefik {
        println!("{}", "🔀 Starting Traefik...".cyan().bold());

        if !smart_compose_up(None, &[traefik.as_str()])? {
            println!("   {} Traefik failed to start, continuing anyway...", "⚠️".yellow());
        }
    }

    // 3. Start workspace container (root docker-compose.yml)
    let workspace_compose = Path::new("docker-compose.yml");
    if workspace_compose.exists() {
        println!("{}", "🛠️  Starting workspace...".cyan().bold());

        if !smart_compose_up(None, &["docker-compose.yml"])? {
            println!("   {} Workspace failed to start, continuing anyway...", "⚠️".yellow());
            println!("   {} Apps will run without shared workspace container", "ℹ️".dimmed());
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
        println!("   {} Found {} apps via pattern: {}", "🔍".dimmed(), compose_files.len(), apps_pattern.dimmed());

        for compose_path in &compose_files {
            // Extract app name from path (apps/foo/docker-compose.yml -> foo)
            let app_name = Path::new(compose_path)
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            println!("   {} Starting {}...", "→".dimmed(), app_name.bold());

            if smart_compose_up(None, &[compose_path.as_str()])? {
                println!("   {} {} started", "✅".green(), app_name);
            } else {
                println!("   {} {} failed to start", "⚠️".yellow(), app_name);
            }
        }
    }

    println!("\n{}", "✅ All services started!".green().bold());

    // Display accessible URLs
    println!();
    println!("{}", "📋 Available URLs:".bright_yellow());
    println!("   Supabase Studio:    http://studio.agiletec.localhost:8081");
    println!("   Supabase API:       http://localhost:18000");
    println!("   Traefik Dashboard:  http://localhost:8081");
    println!();
    println!("{}", "🚀 Apps (libs building on first access):".bright_yellow());
    println!("   Dashboard:          http://agiletec.localhost:8081/dashboard");
    println!("   Evidence Script:    http://agiletec.localhost:8081/evidence-script");
    println!("   Auto Call:          http://agiletec.localhost:8081/auto-call");
    println!("   Realtime Service:   http://agiletec.localhost:8081/realtime");
    println!("   Corporate Site:     http://agiletec.localhost:8081");
    println!("   FocusToday API:     http://agiletec.localhost:8081/focustoday-api");
    println!("   FocusToday Web:     http://agiletec.localhost:8081/focustoday");
    println!("   FocusToday Mobile:  http://agiletec.localhost:8081/focustoday-mobile");
    println!();

    Ok(())
}

/// Orchestrated shutdown: apps -> workspace -> supabase
fn orchestrated_down(manifest: &Manifest) -> Result<()> {
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

    // 2. Stop workspace (root docker-compose.yml)
    let workspace_compose = Path::new("docker-compose.yml");
    if workspace_compose.exists() {
        println!("{}", "🛑 Stopping workspace...".cyan().bold());
        let cmd = "docker compose -f docker-compose.yml down --remove-orphans";
        let _ = exec_command(cmd);
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
        let files: Vec<String> = supabase_files.iter()
            .map(|f| format!("-f {}", f))
            .collect();
        let cmd = format!("docker compose {} down --remove-orphans", files.join(" "));
        let _ = exec_command(&cmd);
    }

    println!("\n{}", "✅ All services stopped!".green().bold());
    Ok(())
}

/// Build docker compose command with orchestration files
fn build_compose_command(manifest: &Manifest, base_cmd: &str) -> String {
    // Check if orchestration.dev is configured
    if let Some(dev) = &manifest.orchestration.dev {
        let mut compose_files = Vec::new();

        // Add workspace compose file
        if let Some(workspace) = &dev.workspace {
            compose_files.push(format!("-f {}", workspace));
        }

        // Add supabase compose files
        if let Some(supabase) = &dev.supabase {
            for file in supabase {
                compose_files.push(format!("-f {}", file));
            }
        }

        // Add traefik compose file
        if let Some(traefik) = &dev.traefik {
            compose_files.push(format!("-f {}", traefik));
        }

        if !compose_files.is_empty() {
            return format!("docker compose {} {}", compose_files.join(" "), base_cmd);
        }
    }

    // Fall back to default (docker-compose.yml if exists)
    let workspace_compose = Path::new("docker-compose.yml");
    if workspace_compose.exists() {
        // If we have a root docker-compose.yml, use it
        return format!("docker compose -f docker-compose.yml {}", base_cmd);
    }

    format!("docker compose {}", base_cmd)
}

/// Build clean command from manifest.toml [workspace.clean] section
///
/// Docker First Philosophy:
/// - Clean HOST side artifacts only (leaked node_modules, .next, etc.)
/// - NEVER touch container cache (preserve build speed)
/// - Exclude supabase and .git directories
fn build_clean_command(manifest: &Manifest) -> String {
    let clean = &manifest.workspace.clean;
    let mut parts = Vec::new();

    // Status message
    parts.push("echo '🧹 Cleaning host build artifacts...'".to_string());

    // Recursive patterns (e.g., node_modules) - clean on host side only
    // Use simple find without -prune to catch all matching directories
    for pattern in &clean.recursive {
        parts.push(format!(
            "find . -maxdepth 3 -type d -name '{}' -not -path './supabase/*' -not -path './.git/*' -exec rm -rf {{}} + 2>/dev/null || true",
            pattern
        ));
    }

    // Root directories - clean on host side only
    // These are typically in manifest.toml [workspace.clean].dirs
    if !clean.dirs.is_empty() {
        let dirs = clean.dirs.iter()
            .map(|d| format!("./{}", d))
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(format!("rm -rf {} 2>/dev/null || true", dirs));
    }

    // Always clean .DS_Store (macOS artifacts)
    parts.push("find . -name '.DS_Store' -delete 2>/dev/null || true".to_string());

    // Success message
    parts.push("echo '✅ Cleaned host build artifacts (container cache preserved)'".to_string());

    parts.join("; ")
}

/// Default commands - CLI is the source of truth, manifest can override
fn default_commands(manifest: &Manifest) -> IndexMap<String, String> {
    let pm = get_package_manager(manifest);
    let service = &manifest.workspace.service;

    let mut cmds = IndexMap::new();

    // Docker compose commands (no package manager)
    cmds.insert("up".to_string(), build_compose_command(manifest, "up -d"));
    cmds.insert("down".to_string(), build_compose_command(manifest, "down --remove-orphans"));
    cmds.insert("logs".to_string(), build_compose_command(manifest, "logs -f"));
    cmds.insert("ps".to_string(), build_compose_command(manifest, "ps"));
    cmds.insert("shell".to_string(), build_compose_command(manifest, &format!("exec -it {} sh", service)));

    // Detect project type: Rust or Node
    let is_rust_project = !manifest.project.rust_edition.is_empty()
        || !manifest.project.binary_name.is_empty();

    if is_rust_project {
        // Rust project: use cargo commands
        cmds.insert("install".to_string(), "cargo install --path .".to_string());
        cmds.insert("build".to_string(), "cargo build --release".to_string());
        cmds.insert("test".to_string(), "cargo test".to_string());
        cmds.insert("lint".to_string(), "cargo clippy".to_string());
        cmds.insert("format".to_string(), "cargo fmt".to_string());
        cmds.insert("dev".to_string(), "cargo watch -x run".to_string());
    } else {
        // Node project: use package manager commands (auto-inferred from manifest.workspace.package_manager)
        cmds.insert("install".to_string(), build_compose_command(manifest, &format!("exec {} {} install", service, pm)));
        cmds.insert("dev".to_string(), build_compose_command(manifest, &format!("exec {} {} dev", service, pm)));
        cmds.insert("build".to_string(), build_compose_command(manifest, &format!("exec {} {} build", service, pm)));
        cmds.insert("test".to_string(), build_compose_command(manifest, &format!("exec {} {} test", service, pm)));
        cmds.insert("lint".to_string(), build_compose_command(manifest, &format!("exec {} {} lint", service, pm)));
        cmds.insert("typecheck".to_string(), build_compose_command(manifest, &format!("exec {} {} typecheck", service, pm)));
        cmds.insert("format".to_string(), build_compose_command(manifest, &format!("exec {} {} format", service, pm)));
    }

    // Clean command
    cmds.insert("clean".to_string(), build_clean_command(manifest));

    cmds
}

/// Check if orchestration is configured in manifest
fn has_orchestration(manifest: &Manifest) -> bool {
    let dev = &manifest.dev;
    // Check for any orchestration config (supabase, traefik, or non-default apps_pattern)
    dev.supabase.is_some()
        || dev.traefik.is_some()
        || !dev.apps_pattern.is_empty()
}

/// Execute a command defined in manifest.toml [commands] section
pub fn run(task: &str) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    // Special handling for up/down with orchestration
    if has_orchestration(&manifest) {
        match task {
            "up" => return orchestrated_up(&manifest),
            "down" => return orchestrated_down(&manifest),
            _ => {}
        }
    }

    // Merge: defaults + manifest overrides (manifest wins)
    let mut commands = default_commands(&manifest);
    for (key, value) in manifest.commands.iter() {
        commands.insert(key.clone(), value.clone());
    }

    // Check if command exists
    let cmd = commands
        .get(task)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "❌ Command '{}' not found in manifest.toml [commands] section.\n\n\
                 Available commands:\n{}",
                task.bold(),
                commands
                    .keys()
                    .map(|k| format!("  - {}", k))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })?;

    println!("🚀 Running: {}", cmd.cyan());

    // Execute command
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .status()
    }
    .with_context(|| format!("Failed to execute: {}", cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

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

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

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

    let cmd = build_compose_command(&manifest, &args.join(" "));

    println!("🚀 Running: {}", cmd.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &cmd])
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .status()
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

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    if cmd.is_empty() {
        bail!("❌ No command specified. Usage: airis exec <service> <cmd>");
    }

    let exec_cmd = format!("exec {} {}", service, cmd.join(" "));
    let full_cmd = build_compose_command(&manifest, &exec_cmd);

    println!("🚀 Running: {}", full_cmd.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &full_cmd])
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .status()
    }
    .with_context(|| format!("Failed to execute: {}", full_cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Build production Docker image for an app
pub fn run_build_prod(app: &str) -> Result<()> {
    use std::time::Instant;

    let app_dir = format!("apps/{}", app);
    let dockerfile = format!("{}/Dockerfile.prod", app_dir);

    if !Path::new(&app_dir).exists() {
        bail!("❌ App directory {} not found", app_dir);
    }

    if !Path::new(&dockerfile).exists() {
        bail!("❌ Dockerfile.prod not found in {}", app_dir);
    }

    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();

    println!("{}", "==================================".bright_blue());
    println!("{}", "Building Production Image".bright_blue().bold());
    println!("App: {}", app.cyan());
    println!("Timestamp: {}", timestamp);
    println!("{}", "==================================".bright_blue());

    let start = Instant::now();

    // Build with BuildKit
    let cmd = format!(
        "DOCKER_BUILDKIT=1 docker build -f {} -t {}:latest -t {}:{} --progress=plain .",
        dockerfile, app, app, timestamp
    );

    println!("🚀 Running: {}", cmd.cyan());

    let status = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .status()
        .with_context(|| "Failed to execute docker build")?;

    let duration = start.elapsed().as_secs();

    if !status.success() {
        bail!("Build failed with exit code: {:?}", status.code());
    }

    // Get image size
    let size_output = Command::new("docker")
        .args(["images", &format!("{}:latest", app), "--format", "{{.Size}}"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string());

    println!();
    println!("{}", "==================================".bright_blue());
    println!("Build completed in {}s", duration);
    println!("{}", "==================================".bright_blue());
    println!();
    println!("{}", "📊 Build Metrics:".bright_yellow());
    println!("  Duration: {}s", duration);
    println!("  Image Size: {}", size_output.trim());
    println!();
    println!("{}", "✅ Build successful!".green());
    println!();
    println!("{}", "Next steps:".bright_yellow());
    println!("  1. Test locally: docker run -p 3000:3000 {}:latest", app);
    println!("  2. Verify health: curl http://localhost:3000/api/health");

    Ok(())
}

/// Quick build test for standalone output
pub fn run_build_quick(app: &str) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    let app_dir = format!("apps/{}", app);

    if !Path::new(&app_dir).exists() {
        bail!("❌ App directory {} not found", app_dir);
    }

    println!("🔨 Testing production build for {}", app.cyan());
    println!();

    // Check for standalone output in next.config
    let next_config = format!("{}/next.config.mjs", app_dir);
    if Path::new(&next_config).exists() {
        let config_content = std::fs::read_to_string(&next_config)?;
        if config_content.contains("output") && config_content.contains("standalone") {
            println!("{}", "✅ Standalone output configured".green());
        } else {
            println!("{}", "⚠️  Warning: Standalone output not found in next.config.mjs".yellow());
        }
    }

    // Build in workspace
    let exec_cmd = format!("exec workspace sh -c 'cd {} && pnpm build'", app_dir);
    let full_cmd = build_compose_command(&manifest, &exec_cmd);

    println!("🚀 Running: {}", full_cmd.cyan());

    let status = Command::new("sh")
        .arg("-c")
        .arg(&full_cmd)
        .status()
        .with_context(|| "Failed to execute build")?;

    if !status.success() {
        bail!("Build failed with exit code: {:?}", status.code());
    }

    println!();
    println!("{}", "✅ Build completed!".green());
    println!();
    println!("{}", "📁 Checking output directory...".bright_yellow());

    // Check standalone output
    let check_cmd = format!(
        "exec workspace sh -c 'ls -lh {0}/.next/standalone/ 2>/dev/null || echo \"Standalone output not found\"'",
        app_dir
    );
    let check_full_cmd = build_compose_command(&manifest, &check_cmd);

    let _ = Command::new("sh")
        .arg("-c")
        .arg(&check_full_cmd)
        .status();

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

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    let restart_cmd = match service {
        Some(svc) => format!("restart {}", svc),
        None => "restart".to_string(),
    };

    let full_cmd = build_compose_command(&manifest, &restart_cmd);

    println!("🚀 Running: {}", full_cmd.cyan());

    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", &full_cmd])
            .status()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(&full_cmd)
            .status()
    }
    .with_context(|| format!("Failed to execute: {}", full_cmd))?;

    if !status.success() {
        bail!("Command failed with exit code: {:?}", status.code());
    }

    Ok(())
}

/// Run tests with coverage check
pub fn run_test_coverage(min_coverage: u8) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    println!("🧪 Running tests with coverage check");
    println!("📊 Minimum coverage threshold: {}%", min_coverage);
    println!();

    // Run tests with coverage in workspace
    let test_cmd = "exec workspace pnpm test:coverage";
    let full_cmd = build_compose_command(&manifest, test_cmd);

    println!("🚀 Running: {}", full_cmd.cyan());

    let output = Command::new("sh")
        .arg("-c")
        .arg(&full_cmd)
        .output()
        .with_context(|| "Failed to execute tests")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Print output
    if !stdout.is_empty() {
        println!("{}", stdout);
    }
    if !stderr.is_empty() {
        eprintln!("{}", stderr);
    }

    if !output.status.success() {
        bail!("Tests failed with exit code: {:?}", output.status.code());
    }

    // Parse coverage from output
    // Look for patterns like "All files  |   85.5 |"
    let coverage_regex = regex::Regex::new(r"All files\s*\|\s*(\d+\.?\d*)")?;

    if let Some(captures) = coverage_regex.captures(&stdout) {
        if let Some(coverage_match) = captures.get(1) {
            let coverage: f64 = coverage_match.as_str().parse().unwrap_or(0.0);

            println!();
            if coverage >= min_coverage as f64 {
                println!(
                    "{}",
                    format!("✅ Coverage {:.1}% meets threshold {}%", coverage, min_coverage).green()
                );
            } else {
                bail!(
                    "❌ Coverage {:.1}% is below threshold {}%",
                    coverage,
                    min_coverage
                );
            }
        }
    } else {
        println!("{}", "⚠️  Could not parse coverage from output".yellow());
        println!("Tests passed, but coverage check skipped.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_run_missing_manifest() {
        let dir = tempdir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = run("test");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("manifest.toml not found"));
    }

    #[test]
    fn test_run_missing_command() {
        let dir = tempdir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create minimal manifest
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[commands]
test = "echo 'test'"
"#;
        fs::write(dir.path().join("manifest.toml"), manifest_content).unwrap();

        let result = run("nonexistent");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("nonexistent") && err_msg.contains("not found"),
            "Expected error about 'nonexistent' not found, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_get_package_manager_pnpm() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "pnpm@10.22.0"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert_eq!(get_package_manager(&manifest), "pnpm");
    }

    #[test]
    fn test_get_package_manager_bun() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "bun@1.0.0"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert_eq!(get_package_manager(&manifest), "bun");
    }

    #[test]
    fn test_get_package_manager_npm() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "npm@10.0.0"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert_eq!(get_package_manager(&manifest), "npm");
    }

    #[test]
    fn test_get_package_manager_yarn() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "yarn@4.0.0"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert_eq!(get_package_manager(&manifest), "yarn");
    }

    #[test]
    fn test_get_package_manager_default() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert_eq!(get_package_manager(&manifest), "pnpm");
    }

    #[test]
    fn test_default_commands_uses_package_manager() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "bun@1.0.0"
service = "app"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let cmds = default_commands(&manifest);

        // Should use bun instead of pnpm
        assert!(cmds.get("install").unwrap().contains("bun install"));
        assert!(cmds.get("dev").unwrap().contains("bun dev"));
        assert!(cmds.get("test").unwrap().contains("bun test"));
        // Should use custom service name
        assert!(cmds.get("shell").unwrap().contains("exec -it app sh"));
    }

    #[test]
    fn test_manifest_commands_override_defaults() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "pnpm@10.0.0"

[commands]
test = "custom test command"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();

        // Simulate merge logic
        let mut commands = default_commands(&manifest);
        for (key, value) in manifest.commands.iter() {
            commands.insert(key.clone(), value.clone());
        }

        // test should be overridden
        assert_eq!(commands.get("test").unwrap(), "custom test command");
        // up should still be default
        assert!(commands.get("up").unwrap().contains("docker compose"));
        // dev should still use pnpm
        assert!(commands.get("dev").unwrap().contains("pnpm dev"));
    }

    #[test]
    fn test_manifest_can_add_custom_commands() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[commands]
my-custom = "echo custom"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();

        // Simulate merge logic
        let mut commands = default_commands(&manifest);
        for (key, value) in manifest.commands.iter() {
            commands.insert(key.clone(), value.clone());
        }

        // Custom command should exist
        assert_eq!(commands.get("my-custom").unwrap(), "echo custom");
        // Defaults should still exist
        assert!(commands.contains_key("up"));
        assert!(commands.contains_key("down"));
    }
}
