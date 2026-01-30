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
                "{}\n\n\n{}\n",
                format!("❌ Docker Compose file not found: {}", file).red().bold(),
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
        if output.status.success()
            && let Ok(config) = serde_json::from_slice::<Value>(&output.stdout)
                && let Some(services) = config.get("services").and_then(|s| s.as_object()) {
                    let mut not_running = Vec::new();

                    for service in services.values() {
                        if let Some(container_name) = service.get("container_name").and_then(|c| c.as_str()) {
                            // Check if container is running
                            let inspect = Command::new("docker")
                                .args(["inspect", "-f", "{{.State.Running}}", container_name])
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

    // Execute docker compose up -d --remove-orphans
    let mut up_args = cmd_args.clone();
    up_args.extend(&["up", "-d", "--remove-orphans"]);

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

/// Extract published ports from a docker-compose file
#[allow(dead_code)]
fn get_compose_ports(compose_file: &str) -> Vec<(String, String, u16)> {
    let output = Command::new("docker")
        .args(["compose", "-f", compose_file, "config", "--format", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let mut results = Vec::new();

    if let Ok(output) = output
        && output.status.success()
            && let Ok(config) = serde_json::from_slice::<Value>(&output.stdout)
                && let Some(services) = config.get("services").and_then(|s| s.as_object()) {
                    for (service_name, service) in services {
                        // Get container_name or use service name
                        let display_name = service
                            .get("container_name")
                            .and_then(|c| c.as_str())
                            .unwrap_or(service_name)
                            .to_string();

                        // Get ports
                        if let Some(ports) = service.get("ports").and_then(|p| p.as_array()) {
                            for port in ports {
                                // Port can be string "8080:80" or object {target: 80, published: 8080}
                                if let Some(port_str) = port.as_str() {
                                    // Parse "host:container" or just "container"
                                    if let Some((host, _)) = port_str.split_once(':')
                                        && let Ok(p) = host.parse::<u16>() {
                                            results.push((service_name.clone(), display_name.clone(), p));
                                        }
                                } else if let Some(obj) = port.as_object()
                                    && let Some(published) = obj.get("published") {
                                        let p = if let Some(p) = published.as_u64() {
                                            p as u16
                                        } else if let Some(s) = published.as_str() {
                                            s.parse().unwrap_or(0)
                                        } else {
                                            0
                                        };
                                        if p > 0 {
                                            results.push((service_name.clone(), display_name.clone(), p));
                                        }
                                    }
                            }
                        }

                        // Note: Traefik-routed services don't expose ports directly
                        // They are accessible via the Traefik proxy
                    }
                }

    results
}

/// Discovered service information
#[derive(Debug, Clone)]
struct DiscoveredService {
    name: String,
    url: String,
    is_reachable: bool,
}

/// Get running Docker containers
fn get_running_containers() -> Vec<String> {
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    if let Ok(output) = output
        && output.status.success() {
            return String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
        }
    Vec::new()
}

/// Parse Traefik dynamic config to get routers
fn parse_traefik_routers(traefik_dir: &str) -> Vec<(String, String, String)> {
    // Returns: (router_name, host, path_prefix)
    let routers_file = format!("{}/dynamic/routers.yml", traefik_dir);

    let content = match std::fs::read_to_string(&routers_file) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();

    // Parse YAML using serde_yaml
    let yaml: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    // Navigate to http.routers
    let routers = yaml
        .get("http")
        .and_then(|h| h.get("routers"))
        .and_then(|r| r.as_mapping());

    if let Some(routers_map) = routers {
        let host_regex = regex::Regex::new(r"Host\(`([^`]+)`\)").ok();
        let path_regex = regex::Regex::new(r"PathPrefix\(`([^`]+)`\)").ok();

        for (name, config) in routers_map {
            let router_name = match name.as_str() {
                Some(n) => n.to_string(),
                None => continue,
            };

            // Get rule string
            let rule = config
                .get("rule")
                .and_then(|r| r.as_str())
                .unwrap_or("");

            // Extract host from rule
            let host = host_regex.as_ref()
                .and_then(|re| re.captures(rule))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            // Extract path prefix from rule
            let path = path_regex.as_ref()
                .and_then(|re| re.captures(rule))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_else(|| "/".to_string());

            if !host.is_empty() {
                results.push((router_name, host, path));
            }
        }
    }

    results
}

/// Check if a service is reachable via HTTP (checks for 502/503/504 errors)
fn is_service_reachable(url: &str) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    // Parse URL to get host:port and path
    let url_without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);

    let (host_port_str, path) = match url_without_scheme.find('/') {
        Some(idx) => (&url_without_scheme[..idx], &url_without_scheme[idx..]),
        None => (url_without_scheme, "/"),
    };

    let parts: Vec<&str> = host_port_str.split(':').collect();
    let host = parts.first().unwrap_or(&"localhost");
    let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(80);

    // For .localhost domains, connect to localhost but use Host header
    let connect_host = if host.ends_with(".localhost") {
        "127.0.0.1"
    } else {
        host
    };

    let addr = match format!("{}:{}", connect_host, port).parse() {
        Ok(a) => a,
        Err(_) => return false,
    };

    // Connect with timeout
    let mut stream = match TcpStream::connect_timeout(&addr, Duration::from_millis(200)) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Set read timeout
    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));

    // Send minimal HTTP request with correct Host header
    let request = format!(
        "HEAD {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );

    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    // Read response (just need the status line)
    let mut buffer = [0u8; 128];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return false,
    };

    let response = String::from_utf8_lossy(&buffer[..bytes_read]);

    // Only 2xx responses are considered "working"
    // Everything else (4xx, 5xx) means the service is broken
    if response.contains(" 200 ") || response.contains(" 201 ") ||
       response.contains(" 204 ") || response.contains(" 301 ") ||
       response.contains(" 302 ") || response.contains(" 307 ") ||
       response.contains(" 308 ") {
        return true;
    }

    false
}

/// Get Traefik routers from Docker labels on running containers
fn get_docker_traefik_routers(workspace_name: &str) -> Vec<(String, String, String)> {
    // Returns: (router_name, host, path_prefix)
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let containers: Vec<String> = match output {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect()
        }
        _ => return Vec::new(),
    };

    let mut results = Vec::new();
    let host_regex = regex::Regex::new(r"Host\(`([^`]+)`\)").ok();
    let path_regex = regex::Regex::new(r"PathPrefix\(`([^`]+)`\)").ok();

    for container in containers {
        // Skip containers not belonging to this workspace
        if !container.contains(workspace_name) && !container.contains("supabase") {
            continue;
        }

        // Get labels for this container
        let output = Command::new("docker")
            .args(["inspect", "--format", "{{json .Config.Labels}}", &container])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output
            && let Ok(labels) = serde_json::from_slice::<serde_json::Map<String, Value>>(&output.stdout) {
                // Check if traefik is enabled
                let traefik_enabled = labels
                    .get("traefik.enable")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "true")
                    .unwrap_or(false);

                if !traefik_enabled {
                    continue;
                }

                // Find router rules
                for (key, value) in &labels {
                    if key.contains(".rule") && key.starts_with("traefik.http.routers.") {
                        let rule = value.as_str().unwrap_or("");

                        // Extract router name from key
                        let router_name = key
                            .strip_prefix("traefik.http.routers.")
                            .and_then(|s| s.strip_suffix(".rule"))
                            .unwrap_or(&container)
                            .to_string();

                        // Check entrypoint is "web"
                        let entrypoint_key = format!("traefik.http.routers.{}.entrypoints", router_name);
                        let entrypoint = labels
                            .get(&entrypoint_key)
                            .and_then(|v| v.as_str())
                            .unwrap_or("web");

                        if entrypoint != "web" {
                            continue;
                        }

                        // Extract host
                        let host = host_regex.as_ref()
                            .and_then(|re| re.captures(rule))
                            .and_then(|c| c.get(1))
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_default();

                        // Extract path prefix
                        let path = path_regex.as_ref()
                            .and_then(|re| re.captures(rule))
                            .and_then(|c| c.get(1))
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_else(|| "/".to_string());

                        // Only include hosts with a proper domain structure
                        if !host.is_empty() && host.contains('.') {
                            results.push((router_name, host, path));
                        }
                    }
                }
            }
    }

    results
}

/// Display service URLs - dynamically discover from running containers and Traefik config
fn display_service_urls(manifest: &Manifest) -> Result<()> {
    let running_containers = get_running_containers();

    if running_containers.is_empty() {
        println!();
        println!("{}", "⚠️  No containers running".yellow());
        return Ok(());
    }

    let mut infra_services: Vec<DiscoveredService> = Vec::new();
    let mut app_services: Vec<DiscoveredService> = Vec::new();

    // 1. Check Traefik port (usually 8081 for dev)
    let traefik_port = if let Some(traefik_file) = &manifest.dev.traefik {
        let output = Command::new("docker")
            .args(["compose", "-f", traefik_file, "config", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output {
            if let Ok(config) = serde_json::from_slice::<Value>(&output.stdout) {
                config.get("services")
                    .and_then(|s| s.get("traefik"))
                    .and_then(|t| t.get("ports"))
                    .and_then(|p| p.as_array())
                    .and_then(|ports| {
                        for port in ports {
                            if let Some(published) = port.get("published") {
                                return published.as_u64().map(|p| p as u16)
                                    .or_else(|| published.as_str().and_then(|s| s.parse().ok()));
                            }
                        }
                        None
                    })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }.unwrap_or(8081);

    // 2. Get routers from Docker labels (dynamic discovery)
    let workspace_name = &manifest.workspace.name;
    let docker_routers = get_docker_traefik_routers(workspace_name);

    // 3. Get routers from static Traefik config
    let static_routers = if let Some(traefik_file) = &manifest.dev.traefik {
        let traefik_dir = Path::new(traefik_file).parent().unwrap_or(Path::new("."));
        parse_traefik_routers(traefik_dir.to_str().unwrap_or("."))
    } else {
        Vec::new()
    };

    // Combine and deduplicate routers
    let mut seen_urls = std::collections::HashSet::new();

    for (router_name, host, path) in docker_routers.into_iter().chain(static_routers.into_iter()) {
        let url = format!("http://{}:{}{}", host, traefik_port, if path == "/" { "".to_string() } else { path.clone() });

        if seen_urls.contains(&url) {
            continue;
        }
        seen_urls.insert(url.clone());

        let is_reachable = is_service_reachable(&url);

        // Determine display name
        let display_name = router_name
            .replace("-", " ")
            .replace("_", " ")
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        let service = DiscoveredService {
            name: display_name,
            url,
            is_reachable,
        };

        if router_name.contains("studio") || router_name.contains("api") && !router_name.contains("focustoday") {
            infra_services.push(service);
        } else {
            app_services.push(service);
        }
    }

    // 4. Display results
    if !infra_services.is_empty() {
        println!();
        println!("{}", "📋 Infrastructure:".bright_yellow());
        for service in &infra_services {
            let status = if service.is_reachable {
                "✓".green()
            } else {
                "✗".red()
            };
            println!("   {} {:<20} {}", status, format!("{}:", service.name), service.url);
        }
    }

    if !app_services.is_empty() {
        println!();
        println!("{}", "🚀 Apps:".bright_yellow());
        for service in &app_services {
            let status = if service.is_reachable {
                "✓".green()
            } else {
                "✗".red()
            };
            println!("   {} {:<20} {}", status, format!("{}:", service.name), service.url);
        }
    }

    println!();
    Ok(())
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
        let compose_file = supabase_files.first().map(|s| s.as_str()).unwrap_or("supabase/docker-compose.yml");
        let health_check = format!("docker compose -f {} exec -T db pg_isready -U postgres -h localhost", compose_file);
        let mut retries = 30;
        while retries > 0 {
            if exec_command(&health_check).unwrap_or(false) {
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

    // Display URLs - either from manifest config or auto-discover from compose files
    display_service_urls(manifest)?;

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
///
/// STRICT: Always requires explicit -f flag to prevent cwd-dependent behavior.
/// Returns Result to allow proper error handling when no compose file is found.
fn build_compose_command(manifest: &Manifest, base_cmd: &str) -> Result<String> {
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
            return Ok(format!("docker compose {} {}", compose_files.join(" "), base_cmd));
        }
    }

    // Fall back to default (docker-compose.yml if exists)
    let workspace_compose = Path::new("docker-compose.yml");
    if workspace_compose.exists() {
        // If we have a root docker-compose.yml, use it
        return Ok(format!("docker compose -f docker-compose.yml {}", base_cmd));
    }

    // STRICT: No compose file found - return error with resolution steps
    bail!(
        "No compose file found.\n\n\
         Expected: docker-compose.yml or [orchestration.dev] config in manifest.toml\n\
         Verify:   airis manifest json\n\
         Generate: airis generate files"
    );
}

/// Validate a clean path/pattern is safe (no path traversal, no absolute paths)
///
/// Returns Some(sanitized_value) if safe, None if dangerous
fn validate_clean_path(path: &str) -> Option<String> {
    let trimmed = path.trim();

    // Reject empty paths
    if trimmed.is_empty() {
        return None;
    }

    // Reject absolute paths
    if trimmed.starts_with('/') || trimmed.starts_with('~') {
        return None;
    }

    // Reject path traversal attempts
    if trimmed.contains("..") {
        return None;
    }

    // Reject shell metacharacters that could be exploited
    // Allow only alphanumeric, dash, underscore, dot, and forward slash
    let is_safe = trimmed.chars().all(|c| {
        c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/'
    });

    if !is_safe {
        return None;
    }

    // Reject paths that start with a dot followed by nothing or slash (hidden dirs are ok like .next)
    // But reject "." or "./" as they would delete everything
    if trimmed == "." || trimmed == "./" {
        return None;
    }

    Some(trimmed.to_string())
}

/// Validate a recursive pattern (for find -name)
///
/// Returns Some(sanitized_pattern) if safe, None if dangerous
fn validate_clean_pattern(pattern: &str) -> Option<String> {
    let trimmed = pattern.trim();

    // Reject empty patterns
    if trimmed.is_empty() {
        return None;
    }

    // Patterns should be simple names, not paths
    if trimmed.contains('/') || trimmed.contains("..") {
        return None;
    }

    // Reject shell metacharacters except for glob wildcards (* and ?)
    // which are handled safely by find -name
    let is_safe = trimmed.chars().all(|c| {
        c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '*' || c == '?'
    });

    if !is_safe {
        return None;
    }

    // Escape single quotes for shell safety
    let escaped = trimmed.replace('\'', "'\\''");

    Some(escaped)
}

/// Build clean command from manifest.toml [workspace.clean] section
///
/// Docker First Philosophy:
/// - Clean HOST side artifacts only (leaked node_modules, .next, etc.)
/// - NEVER touch container cache (preserve build speed)
/// - Exclude supabase and .git directories
///
/// Security:
/// - Validates all paths to prevent path traversal attacks
/// - Rejects absolute paths and ".." sequences
/// - Sanitizes shell metacharacters
fn build_clean_command(manifest: &Manifest) -> String {
    let clean = &manifest.workspace.clean;
    let mut parts = Vec::new();

    // Status message
    parts.push("echo '🧹 Cleaning host build artifacts...'".to_string());

    // Recursive patterns (e.g., node_modules) - clean on host side only
    // Use simple find without -prune to catch all matching directories
    // Exclude: supabase, infra (contains Supabase local), .git
    for pattern in &clean.recursive {
        if let Some(safe_pattern) = validate_clean_pattern(pattern) {
            parts.push(format!(
                "find . -maxdepth 3 -type d -name '{}' -not -path './supabase/*' -not -path './infra/*' -not -path './.git/*' -exec rm -rf {{}} + 2>/dev/null || true",
                safe_pattern
            ));
        } else {
            // Warn about skipped dangerous pattern
            parts.push(format!(
                "echo '⚠️  Skipped unsafe recursive pattern: {}'",
                pattern.replace('\'', "")
            ));
        }
    }

    // Root directories - clean on host side only
    // These are typically in manifest.toml [workspace.clean].dirs
    // Process each directory individually for safety
    for dir in &clean.dirs {
        if let Some(safe_dir) = validate_clean_path(dir) {
            // Use -maxdepth 0 to only match the exact path, not traverse into it first
            // This ensures we're deleting what we intend to delete
            parts.push(format!(
                "rm -rf './{}'",
                safe_dir.replace('\'', "'\\''")
            ));
        } else {
            // Warn about skipped dangerous path
            parts.push(format!(
                "echo '⚠️  Skipped unsafe clean path: {}'",
                dir.replace('\'', "")
            ));
        }
    }

    // Always clean .DS_Store (macOS artifacts)
    parts.push("find . -name '.DS_Store' -delete 2>/dev/null || true".to_string());

    // Success message
    parts.push("echo '✅ Cleaned host build artifacts (container cache preserved)'".to_string());

    parts.join("; ")
}

/// Default commands - CLI is the source of truth, manifest can override
fn default_commands(manifest: &Manifest) -> Result<IndexMap<String, String>> {
    let pm = get_package_manager(manifest);
    let service = &manifest.workspace.service;

    let mut cmds = IndexMap::new();

    // Docker compose commands (no package manager)
    cmds.insert("up".to_string(), build_compose_command(manifest, "up -d")?);
    cmds.insert("down".to_string(), build_compose_command(manifest, "down --remove-orphans")?);
    cmds.insert("logs".to_string(), build_compose_command(manifest, "logs -f")?);
    cmds.insert("ps".to_string(), build_compose_command(manifest, "ps")?);
    cmds.insert("shell".to_string(), build_compose_command(manifest, &format!("exec -it {} sh", service))?);

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
        cmds.insert("install".to_string(), build_compose_command(manifest, &format!("exec {} {} install", service, pm))?);
        cmds.insert("dev".to_string(), build_compose_command(manifest, &format!("exec {} {} dev", service, pm))?);
        cmds.insert("build".to_string(), build_compose_command(manifest, &format!("exec {} {} build", service, pm))?);
        cmds.insert("test".to_string(), build_compose_command(manifest, &format!("exec {} {} test", service, pm))?);
        cmds.insert("lint".to_string(), build_compose_command(manifest, &format!("exec {} {} lint", service, pm))?);
        cmds.insert("typecheck".to_string(), build_compose_command(manifest, &format!("exec {} {} typecheck", service, pm))?);
        cmds.insert("format".to_string(), build_compose_command(manifest, &format!("exec {} {} format", service, pm))?);
    }

    // Clean command
    cmds.insert("clean".to_string(), build_clean_command(manifest));

    Ok(cmds)
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
    let mut commands = default_commands(&manifest)?;
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

    let cmd = build_compose_command(&manifest, &args.join(" "))?;

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
    let full_cmd = build_compose_command(&manifest, &exec_cmd)?;

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
    let full_cmd = build_compose_command(&manifest, &exec_cmd)?;

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
    let check_full_cmd = build_compose_command(&manifest, &check_cmd)?;

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

    let full_cmd = build_compose_command(&manifest, &restart_cmd)?;

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
    let full_cmd = build_compose_command(&manifest, test_cmd)?;

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

    use tempfile::tempdir;

    // Helper to run tests with serialization since set_current_dir is not thread-safe
    use std::sync::Mutex;
    static DIR_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_run_missing_manifest() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = std::panic::catch_unwind(|| {
            let result = run("test");
            assert!(result.is_err());
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("manifest.toml not found"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_run_missing_command() {
        // Note: This test checks default_commands directly to avoid
        // directory change race conditions with other tests
        // Use Rust project to avoid needing docker-compose.yml
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[project]
rust_edition = "2024"
binary_name = "test"

[commands]
test = "echo 'test'"
"#;
        let manifest: crate::manifest::Manifest =
            toml::from_str(manifest_content).unwrap();

        // Merge defaults with manifest commands (Rust project doesn't need compose)
        let commands = default_commands(&manifest).unwrap();

        // Check that nonexistent command is not in the map
        assert!(
            !commands.contains_key("nonexistent"),
            "Command 'nonexistent' should not exist in commands"
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
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create docker-compose.yml for Node project test
        std::fs::write("docker-compose.yml", "version: '3'").unwrap();

        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "bun@1.0.0"
service = "app"
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();
            let cmds = default_commands(&manifest).unwrap();

            // Should use bun instead of pnpm
            assert!(cmds.get("install").unwrap().contains("bun install"));
            assert!(cmds.get("dev").unwrap().contains("bun dev"));
            assert!(cmds.get("test").unwrap().contains("bun test"));
            // Should use custom service name
            assert!(cmds.get("shell").unwrap().contains("exec -it app sh"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_manifest_commands_override_defaults() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create docker-compose.yml for Node project test
        std::fs::write("docker-compose.yml", "version: '3'").unwrap();

        let manifest_content = r#"
version = 1

[workspace]
name = "test"
package_manager = "pnpm@10.0.0"

[commands]
test = "custom test command"
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();

            // Simulate merge logic
            let mut commands = default_commands(&manifest).unwrap();
            for (key, value) in manifest.commands.iter() {
                commands.insert(key.clone(), value.clone());
            }

            // test should be overridden
            assert_eq!(commands.get("test").unwrap(), "custom test command");
            // up should still be default
            assert!(commands.get("up").unwrap().contains("docker compose"));
            // dev should still use pnpm
            assert!(commands.get("dev").unwrap().contains("pnpm dev"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_manifest_can_add_custom_commands() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create docker-compose.yml for Node project test
        std::fs::write("docker-compose.yml", "version: '3'").unwrap();

        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[commands]
my-custom = "echo custom"
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();

            // Simulate merge logic
            let mut commands = default_commands(&manifest).unwrap();
            for (key, value) in manifest.commands.iter() {
                commands.insert(key.clone(), value.clone());
            }

            // Custom command should exist
            assert_eq!(commands.get("my-custom").unwrap(), "echo custom");
            // Defaults should still exist
            assert!(commands.contains_key("up"));
            assert!(commands.contains_key("down"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    // Security tests for clean command validation

    #[test]
    fn test_validate_clean_path_safe_paths() {
        // Safe paths should be accepted
        assert!(validate_clean_path(".next").is_some());
        assert!(validate_clean_path("dist").is_some());
        assert!(validate_clean_path("build").is_some());
        assert!(validate_clean_path("apps/dashboard/.next").is_some());
        assert!(validate_clean_path("node_modules").is_some());
    }

    #[test]
    fn test_validate_clean_path_rejects_traversal() {
        // Path traversal should be rejected
        assert!(validate_clean_path("..").is_none());
        assert!(validate_clean_path("../").is_none());
        assert!(validate_clean_path("../other-project").is_none());
        assert!(validate_clean_path("foo/../bar").is_none());
        assert!(validate_clean_path("../../important").is_none());
    }

    #[test]
    fn test_validate_clean_path_rejects_absolute() {
        // Absolute paths should be rejected
        assert!(validate_clean_path("/").is_none());
        assert!(validate_clean_path("/tmp").is_none());
        assert!(validate_clean_path("/etc/passwd").is_none());
        assert!(validate_clean_path("~").is_none());
        assert!(validate_clean_path("~/Documents").is_none());
    }

    #[test]
    fn test_validate_clean_path_rejects_shell_chars() {
        // Shell metacharacters should be rejected
        assert!(validate_clean_path("foo; rm -rf /").is_none());
        assert!(validate_clean_path("foo && bar").is_none());
        assert!(validate_clean_path("$(whoami)").is_none());
        assert!(validate_clean_path("`id`").is_none());
        assert!(validate_clean_path("foo|bar").is_none());
        assert!(validate_clean_path("foo > bar").is_none());
    }

    #[test]
    fn test_validate_clean_path_rejects_dangerous() {
        // Current directory should be rejected (would delete everything)
        assert!(validate_clean_path(".").is_none());
        assert!(validate_clean_path("./").is_none());
        assert!(validate_clean_path("").is_none());
    }

    #[test]
    fn test_validate_clean_pattern_safe_patterns() {
        // Safe patterns should be accepted
        assert!(validate_clean_pattern("node_modules").is_some());
        assert!(validate_clean_pattern(".next").is_some());
        assert!(validate_clean_pattern("*.log").is_some());
        assert!(validate_clean_pattern("dist").is_some());
    }

    #[test]
    fn test_validate_clean_pattern_rejects_paths() {
        // Patterns with paths should be rejected (find -name doesn't use paths)
        assert!(validate_clean_pattern("foo/bar").is_none());
        assert!(validate_clean_pattern("../node_modules").is_none());
    }

    #[test]
    fn test_validate_clean_pattern_rejects_shell_injection() {
        // Shell injection attempts should be rejected
        assert!(validate_clean_pattern("'; rm -rf /; '").is_none());
        assert!(validate_clean_pattern("$(whoami)").is_none());
        assert!(validate_clean_pattern("`id`").is_none());
    }

    #[test]
    fn test_build_clean_command_filters_unsafe() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[workspace.clean]
dirs = [".next", "../dangerous", "/etc", "dist"]
recursive = ["node_modules", "'; rm -rf /;"]
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let cmd = build_clean_command(&manifest);

        // Safe paths should be in the command
        assert!(cmd.contains(".next"));
        assert!(cmd.contains("dist"));
        assert!(cmd.contains("node_modules"));

        // Dangerous paths should be skipped (warning shown instead)
        assert!(cmd.contains("Skipped unsafe clean path: ../dangerous"));
        assert!(cmd.contains("Skipped unsafe clean path: /etc"));
        assert!(cmd.contains("Skipped unsafe recursive pattern"));
    }

    #[test]
    fn test_build_compose_command_no_compose_file_errors() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // No docker-compose.yml, no orchestration config
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();

            // Should error because no compose file exists
            let result = build_compose_command(&manifest, "up -d");
            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("No compose file found"));
            assert!(err_msg.contains("airis manifest json"));
            assert!(err_msg.contains("airis generate files"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_build_compose_command_with_compose_file_succeeds() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create docker-compose.yml
        std::fs::write("docker-compose.yml", "version: '3'").unwrap();

        let manifest_content = r#"
version = 1

[workspace]
name = "test"
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();
            let result = build_compose_command(&manifest, "up -d");
            assert!(result.is_ok());
            let cmd = result.unwrap();
            assert!(cmd.contains("-f docker-compose.yml"));
            assert!(cmd.contains("up -d"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_build_compose_command_with_orchestration_succeeds() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // No docker-compose.yml, but orchestration config exists
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[orchestration.dev]
workspace = "docker-compose.yml"
traefik = "traefik/docker-compose.yml"
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();
            let result = build_compose_command(&manifest, "up -d");
            assert!(result.is_ok());
            let cmd = result.unwrap();
            assert!(cmd.contains("-f docker-compose.yml"));
            assert!(cmd.contains("-f traefik/docker-compose.yml"));
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }
}
