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

/// Find the compose file in the current directory.
/// Checks in order of priority: compose.yaml, compose.yml, docker-compose.yaml, docker-compose.yml
/// Returns the filename if found, None otherwise.
fn find_compose_file() -> Option<&'static str> {
    // Modern naming (preferred)
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
fn ensure_env_file() {
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
            Err(e) => println!(
                "   {} Failed to copy .env.example: {}",
                "⚠️".yellow(),
                e
            ),
        }
    }
}

/// Run post_up hooks from manifest (idempotent, warns on failure)
fn run_post_up(manifest: &Manifest) {
    let hooks = &manifest.dev.post_up;
    if hooks.is_empty() {
        return;
    }

    println!("\n{}", "🔧 Running post_up hooks...".cyan().bold());
    for hook in hooks {
        println!("   {} {}", "→".dimmed(), hook.dimmed());
        let status = Command::new("sh")
            .arg("-c")
            .arg(hook)
            .status();

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
                println!(
                    "   {} post_up hook error: {} — {}",
                    "⚠️".yellow(),
                    hook,
                    e
                );
            }
        }
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
    up_args.extend(&["up", "-d", "--build", "--remove-orphans"]);

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

/// Discovered service information
#[derive(Debug, Clone)]
struct DiscoveredService {
    name: String,
    url: String,
    is_reachable: bool,
}

/// Extract the host port from a manifest ServiceConfig.
/// Handles formats: "${VAR:-DEFAULT}:CONTAINER", "HOST:CONTAINER", single port
fn extract_host_port_from_service(svc: &crate::manifest::ServiceConfig) -> Option<u16> {
    // Try ports array first, then fallback to deprecated port field
    let port_str = svc.ports.first().map(|s| s.as_str());
    if let Some(port_str) = port_str {
        // Handle ${VAR:-DEFAULT}:CONTAINER format
        // The ":-" inside ${} conflicts with ":" as port separator,
        // so split on "}:" to find the boundary
        if port_str.starts_with("${") {
            if let Some(close_brace) = port_str.find('}') {
                let var_part = &port_str[2..close_brace]; // "VAR:-DEFAULT"
                if let Some(default_val) = var_part.split(":-").nth(1) {
                    return default_val.parse().ok();
                }
            }
            return None;
        }
        // Plain "HOST:CONTAINER" format
        let host_part = port_str.split(':').next()?;
        return host_part.parse().ok();
    }
    svc.port
}

/// Parse service ports from docker compose config JSON
/// Returns Vec<(service_name, host_port)>
fn parse_service_ports_from_config(config: &Value) -> Vec<(String, u16)> {
    let mut results = Vec::new();

    if let Some(services) = config.get("services").and_then(|s| s.as_object()) {
        for (svc_name, svc_config) in services {
            // Use container_name if available, otherwise service name
            let display_name = svc_config
                .get("container_name")
                .and_then(|c| c.as_str())
                .unwrap_or(svc_name)
                .to_string();

            if let Some(ports) = svc_config.get("ports").and_then(|p| p.as_array()) {
                for port in ports {
                    // Object format (from docker compose config --format json)
                    if let Some(published) = port.get("published") {
                        let p = published
                            .as_u64()
                            .map(|p| p as u16)
                            .or_else(|| published.as_str().and_then(|s| s.parse().ok()));
                        if let Some(host_port) = p {
                            if host_port > 0 {
                                results.push((display_name.clone(), host_port));
                            }
                        }
                    } else if let Some(port_str) = port.as_str() {
                        // String format fallback: "HOST_PORT:CONTAINER_PORT" or "HOST:HOST_PORT:CONTAINER_PORT"
                        let parts: Vec<&str> = port_str.split(':').collect();
                        let host_port = match parts.len() {
                            2 => parts[0].parse::<u16>().ok(),
                            3 => parts[1].parse::<u16>().ok(),
                            _ => None,
                        };
                        if let Some(p) = host_port {
                            if p > 0 {
                                results.push((display_name.clone(), p));
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

/// Collect all compose files from manifest configuration
fn collect_all_compose_files(manifest: &Manifest) -> Vec<String> {
    let mut files = Vec::new();

    // Check orchestration.dev first
    if let Some(dev) = &manifest.orchestration.dev {
        if let Some(workspace) = &dev.workspace {
            if Path::new(workspace).exists() {
                files.push(workspace.clone());
            }
        }
        if let Some(supabase_files) = &dev.supabase {
            for f in supabase_files {
                if Path::new(f).exists() && !files.contains(f) {
                    files.push(f.clone());
                }
            }
        }
        if let Some(traefik) = &dev.traefik {
            if Path::new(traefik).exists() && !files.contains(traefik) {
                files.push(traefik.clone());
            }
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
            if Path::new(f).exists() && !files.contains(f) {
                files.push(f.clone());
            }
        }
    }
    if let Some(traefik_file) = &manifest.dev.traefik {
        if Path::new(traefik_file).exists() && !files.contains(traefik_file) {
            files.push(traefik_file.clone());
        }
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

/// Discover service URLs from compose file port mappings
fn discover_compose_port_urls(compose_files: &[String]) -> Vec<DiscoveredService> {
    let mut services = Vec::new();
    let mut seen_ports = std::collections::HashSet::new();

    for file in compose_files {
        let output = Command::new("docker")
            .args(["compose", "-f", file, "config", "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output
            && output.status.success()
        {
            if let Ok(config) = serde_json::from_slice::<Value>(&output.stdout) {
                for (name, port) in parse_service_ports_from_config(&config) {
                    if seen_ports.contains(&port) {
                        continue;
                    }
                    seen_ports.insert(port);

                    let url = format!("http://localhost:{}", port);
                    services.push(DiscoveredService {
                        name,
                        url,
                        is_reachable: false,
                    });
                }
            }
        }
    }

    services
}

/// Display URLs discovered from compose file port mappings (no manifest required)
fn display_compose_urls(compose_files: &[String]) {
    let mut services = discover_compose_port_urls(compose_files);
    for svc in &mut services {
        svc.is_reachable = is_service_reachable(&svc.url);
    }
    display_url_table(&services);
}

/// Display URL table (shared between display_service_urls and run_ps)
fn display_url_table(services: &[DiscoveredService]) {
    if services.is_empty() {
        return;
    }

    println!();
    println!("{}", "=== Services ===".bright_yellow());
    for service in services {
        let status = if service.is_reachable {
            "✓".green()
        } else {
            "✗".red()
        };
        println!(
            "  {} {:<24}{}",
            status,
            format!("{}:", service.name),
            service.url
        );
    }
    println!("{}", "===".bright_yellow());
}

/// Condense docker status string for compact display
/// "Up 3 minutes" → "Up 3m", "Up About an hour" → "Up ~1h"
fn condense_status(status: &str) -> String {
    let s = status.trim();

    if let Some(rest) = s.strip_prefix("Up ") {
        // Handle "About an hour" / "About a minute"
        if rest.starts_with("About an hour") {
            return "Up ~1h".to_string();
        }
        if rest.starts_with("About a minute") {
            return "Up ~1m".to_string();
        }

        // Parse "X unit" pattern (e.g., "3 minutes", "2 hours")
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Ok(_) = parts[0].parse::<u64>() {
                let short_unit = match parts[1] {
                    u if u.starts_with("second") => "s",
                    u if u.starts_with("minute") => "m",
                    u if u.starts_with("hour") => "h",
                    u if u.starts_with("day") => "d",
                    u if u.starts_with("week") => "w",
                    u if u.starts_with("month") => "mo",
                    _ => return s.to_string(),
                };
                return format!("Up {}{}", parts[0], short_unit);
            }
        }
    }

    s.to_string()
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

/// Display service URLs - 3-tier priority discovery:
/// 1. manifest.dev.urls (explicit, highest priority)
/// 2. compose port mapping auto-detection
/// 3. Traefik Docker labels + static config (lowest priority)
fn display_service_urls(manifest: &Manifest) -> Result<()> {
    let mut all_services: Vec<DiscoveredService> = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    // Tier 1: manifest.dev.urls (explicit configuration)
    if let Some(dev_urls) = &manifest.dev.urls {
        for entry in dev_urls.infra.iter().chain(dev_urls.apps.iter()) {
            if seen_urls.insert(entry.url.clone()) {
                all_services.push(DiscoveredService {
                    name: entry.name.clone(),
                    url: entry.url.clone(),
                    is_reachable: is_service_reachable(&entry.url),
                });
            }
        }
    }

    // Tier 2: manifest.service port definitions (works regardless of profiles)
    for (svc_name, svc_config) in &manifest.service {
        if let Some(host_port) = extract_host_port_from_service(svc_config) {
            let url = format!("http://localhost:{}", host_port);
            if seen_urls.insert(url.clone()) {
                let display_name = svc_config
                    .container_name
                    .as_deref()
                    .unwrap_or(svc_name)
                    .to_string();
                all_services.push(DiscoveredService {
                    name: display_name,
                    url,
                    is_reachable: false,
                });
            }
        }
    }

    // Tier 3: compose port mapping auto-detection
    let compose_files = collect_all_compose_files(manifest);
    for svc in discover_compose_port_urls(&compose_files) {
        if seen_urls.insert(svc.url.clone()) {
            all_services.push(DiscoveredService {
                name: svc.name,
                url: svc.url.clone(),
                is_reachable: is_service_reachable(&svc.url),
            });
        }
    }

    // Tier 4: Traefik Docker labels + static config
    if manifest.dev.traefik.is_some() {
        // Extract Traefik port
        let traefik_port = if let Some(traefik_file) = &manifest.dev.traefik {
            let output = Command::new("docker")
                .args(["compose", "-f", traefik_file, "config", "--format", "json"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output();

            if let Ok(output) = output {
                serde_json::from_slice::<Value>(&output.stdout)
                    .ok()
                    .and_then(|config| {
                        config
                            .get("services")
                            .and_then(|s| s.get("traefik"))
                            .and_then(|t| t.get("ports"))
                            .and_then(|p| p.as_array())
                            .and_then(|ports| {
                                for port in ports {
                                    if let Some(published) = port.get("published") {
                                        return published
                                            .as_u64()
                                            .map(|p| p as u16)
                                            .or_else(|| {
                                                published.as_str().and_then(|s| s.parse().ok())
                                            });
                                    }
                                }
                                None
                            })
                    })
            } else {
                None
            }
        } else {
            None
        }
        .unwrap_or(8081);

        let workspace_name = &manifest.workspace.name;
        let docker_routers = get_docker_traefik_routers(workspace_name);
        let static_routers = if let Some(traefik_file) = &manifest.dev.traefik {
            let traefik_dir = Path::new(traefik_file).parent().unwrap_or(Path::new("."));
            parse_traefik_routers(traefik_dir.to_str().unwrap_or("."))
        } else {
            Vec::new()
        };

        for (router_name, host, path) in
            docker_routers.into_iter().chain(static_routers.into_iter())
        {
            let url = format!(
                "http://{}:{}{}",
                host,
                traefik_port,
                if path == "/" {
                    "".to_string()
                } else {
                    path.clone()
                }
            );

            if !host.is_empty() && host.contains('.') && seen_urls.insert(url.clone()) {
                let is_reachable = is_service_reachable(&url);
                let display_name = router_name
                    .replace('-', " ")
                    .replace('_', " ")
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

                all_services.push(DiscoveredService {
                    name: display_name,
                    url,
                    is_reachable,
                });
            }
        }
    }

    // Check reachability for services that haven't been checked yet
    for svc in &mut all_services {
        if !svc.is_reachable {
            svc.is_reachable = is_service_reachable(&svc.url);
        }
    }

    display_url_table(&all_services);
    Ok(())
}

/// Orchestrated startup: supabase -> workspace -> apps
fn orchestrated_up(manifest: &Manifest) -> Result<()> {
    ensure_env_file();
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
            if exec_command(&health_check)? {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
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

        if !smart_compose_up(None, &[traefik.as_str()])? {
            bail!("Traefik failed to start. Check `docker compose -f {} logs`", traefik);
        }
    }

    // 3. Start workspace container (root compose.yml or docker-compose.yml)
    if let Some(compose_file) = find_compose_file() {
        println!("{}", "🛠️  Starting workspace...".cyan().bold());

        if !smart_compose_up(None, &[compose_file])? {
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

    // Run post_up hooks (e.g., DB migration)
    run_post_up(manifest);

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

    // 2. Stop workspace (root compose.yml or docker-compose.yml)
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

    // Fall back to default (compose.yml or docker-compose.yml if exists)
    if let Some(compose_file) = find_compose_file() {
        return Ok(format!("docker compose -f {} {}", compose_file, base_cmd));
    }

    // STRICT: No compose file found - return error with resolution steps
    bail!(
        "No compose file found.\n\n\
         Expected: compose.yml (or docker-compose.yml) or [orchestration.dev] config in manifest.toml\n\
         Verify:   airis manifest json\n\
         Generate: airis gen"
    );
}

/// Validate a clean path/pattern is safe (no path traversal, no absolute paths)
///
/// Returns Some(sanitized_value) if safe, None if dangerous
#[cfg(test)]
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
#[cfg(test)]
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
#[cfg(test)]
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
    let mut cmds = IndexMap::new();

    // Detect project type: Rust or Node
    let is_rust_project = !manifest.project.rust_edition.is_empty()
        || !manifest.project.binary_name.is_empty();

    if is_rust_project {
        // Rust project: use cargo commands (no docker compose required)
        cmds.insert("build".to_string(), "cargo build --release".to_string());
        cmds.insert("test".to_string(), "cargo test".to_string());
        cmds.insert("lint".to_string(), "cargo clippy".to_string());
        cmds.insert("format".to_string(), "cargo fmt".to_string());
    } else {
        // Docker/Node project: up/down/ps only — no workspace container
        // Install runs inside Dockerfile RUN, not via CLI
        cmds.insert("up".to_string(), build_compose_command(manifest, "up -d --build --remove-orphans")?);
        cmds.insert("down".to_string(), build_compose_command(manifest, "down --remove-orphans")?);
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
    // Only count apps_pattern as orchestration if it matches actual files
    if !dev.apps_pattern.is_empty() {
        if let Ok(mut entries) = glob(&dev.apps_pattern) {
            return entries.next().is_some();
        }
    }
    false
}

/// Execute a command defined in manifest.toml [commands] section
pub fn run(task: &str, extra_args: &[String]) -> Result<()> {
    // Block shell metacharacters in extra_args to prevent host command injection
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

    // Allow up/down without manifest.toml if compose file exists
    if !manifest_path.exists() {
        if matches!(task, "up" | "down") {
            // Check for compose files (modern: compose.yml, legacy: docker-compose.yml)
            if let Some(compose_file) = find_compose_file() {
                if task == "up" {
                    ensure_env_file();
                }
                let action = if task == "up" { "up -d --build --remove-orphans" } else { "down" };
                let cmd = format!("docker compose -f {} {}", compose_file, action);

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
                if task == "up" {
                    println!("\n{}", "✅ All services started!".green().bold());
                    display_compose_urls(&[compose_file.to_string()]);
                }
                return Ok(());
            }
        }

        bail!(
            "❌ manifest.toml not found. Run {} first.",
            "airis init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path)
        .with_context(|| "Failed to load manifest.toml")?;

    // Special handling for up/down with orchestration
    // User-defined [commands] override always takes priority over orchestration
    if !manifest.commands.contains_key(task) && has_orchestration(&manifest) {
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

    if task == "up" {
        ensure_env_file();
    }

    // Append extra arguments if provided (metacharacter check already done above)
    let full_cmd = if extra_args.is_empty() {
        cmd.to_string()
    } else {
        format!("{} {}", cmd, extra_args.join(" "))
    };

    println!("🚀 Running: {}", full_cmd.cyan());

    // Execute command
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

    // Display URLs after successful "up" command
    if task == "up" {
        println!("\n{}", "✅ All services started!".green().bold());
        run_post_up(&manifest);
        display_service_urls(&manifest)?;
    }

    Ok(())
}

/// Show running services with status and URLs
pub fn run_ps() -> Result<()> {
    // Get running containers with status
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}\t{{.Status}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .with_context(|| "Failed to execute docker ps")?;

    if !output.status.success() {
        bail!("docker ps failed");
    }

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
        println!("{}", "No running containers".yellow());
        return Ok(());
    }

    // Build URL map from compose files
    let url_map = {
        let manifest_path = Path::new("manifest.toml");
        let port_services = if manifest_path.exists() {
            let manifest = Manifest::load(manifest_path)?;
            let compose_files = collect_all_compose_files(&manifest);
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

    println!();
    println!("{}", "=== Running Services ===".bright_yellow());
    for (name, status) in &containers {
        let condensed = condense_status(status);
        if let Some(url) = url_map.get(*name) {
            println!("  {:<24}{:<10} {}", name, condensed, url);
        } else {
            println!("  {:<24}{}", name, condensed);
        }
    }
    println!("{}", "===".bright_yellow());
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
            let result = run("test", &[]);
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
"#;
        let result = std::panic::catch_unwind(|| {
            let manifest: Manifest = toml::from_str(manifest_content).unwrap();
            let cmds = default_commands(&manifest).unwrap();

            // Only up/down/ps for Node projects — no workspace container
            assert!(cmds.contains_key("up"));
            assert!(cmds.contains_key("down"));
            assert!(cmds.contains_key("ps"));
            assert!(!cmds.contains_key("install"));
            assert!(!cmds.contains_key("dev"));
            assert!(!cmds.contains_key("shell"));
            assert!(!cmds.contains_key("build"));
            assert!(!cmds.contains_key("test"));
            assert!(!cmds.contains_key("lint"));
            assert!(!cmds.contains_key("clean"));
            assert!(!cmds.contains_key("logs"));
            // "up" should include --build
            assert!(cmds.get("up").unwrap().contains("up -d --build"));
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
            // up should still be default with --build
            assert!(commands.get("up").unwrap().contains("docker compose"));
            assert!(commands.get("up").unwrap().contains("--build"));
            // dev/install are no longer in defaults
            assert!(!commands.contains_key("dev"));
            assert!(!commands.contains_key("install"));
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
            assert!(err_msg.contains("airis gen"));
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

    #[test]
    fn test_condense_status_minutes() {
        assert_eq!(condense_status("Up 3 minutes"), "Up 3m");
        assert_eq!(condense_status("Up 38 minutes"), "Up 38m");
        assert_eq!(condense_status("Up 1 minute"), "Up 1m");
    }

    #[test]
    fn test_condense_status_hours() {
        assert_eq!(condense_status("Up 2 hours"), "Up 2h");
        assert_eq!(condense_status("Up About an hour"), "Up ~1h");
    }

    #[test]
    fn test_condense_status_other() {
        assert_eq!(condense_status("Up 5 seconds"), "Up 5s");
        assert_eq!(condense_status("Up 3 days"), "Up 3d");
        assert_eq!(condense_status("Up About a minute"), "Up ~1m");
    }

    #[test]
    fn test_condense_status_passthrough() {
        assert_eq!(condense_status("Exited (0)"), "Exited (0)");
        assert_eq!(condense_status("Created"), "Created");
    }

    #[test]
    fn test_parse_service_ports_object_format() {
        let config: Value = serde_json::json!({
            "services": {
                "web": {
                    "container_name": "my-web",
                    "ports": [
                        {
                            "mode": "ingress",
                            "target": 3000,
                            "published": "3000",
                            "protocol": "tcp"
                        }
                    ]
                },
                "api": {
                    "ports": [
                        {
                            "mode": "ingress",
                            "target": 8080,
                            "published": 8080,
                            "protocol": "tcp"
                        }
                    ]
                }
            }
        });

        let result = parse_service_ports_from_config(&config);
        assert_eq!(result.len(), 2);
        // BTreeMap sorts keys alphabetically: "api" before "web"
        assert_eq!(result[0], ("api".to_string(), 8080));
        assert_eq!(result[1], ("my-web".to_string(), 3000));
    }

    #[test]
    fn test_parse_service_ports_string_format() {
        let config: Value = serde_json::json!({
            "services": {
                "web": {
                    "ports": ["3000:3000"]
                },
                "api": {
                    "ports": ["8080:80"]
                }
            }
        });

        let result = parse_service_ports_from_config(&config);
        assert_eq!(result.len(), 2);
        // BTreeMap sorts keys alphabetically: "api" before "web"
        assert_eq!(result[0], ("api".to_string(), 8080));
        assert_eq!(result[1], ("web".to_string(), 3000));
    }

    #[test]
    fn test_parse_service_ports_no_ports() {
        let config: Value = serde_json::json!({
            "services": {
                "db": {
                    "image": "postgres:15"
                }
            }
        });

        let result = parse_service_ports_from_config(&config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_service_ports_empty_services() {
        let config: Value = serde_json::json!({
            "services": {}
        });

        let result = parse_service_ports_from_config(&config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_service_ports_skips_zero_port() {
        let config: Value = serde_json::json!({
            "services": {
                "web": {
                    "ports": [
                        {
                            "target": 3000,
                            "published": "0"
                        }
                    ]
                }
            }
        });

        let result = parse_service_ports_from_config(&config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_manifest_dev_urls_parsing() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[[dev.urls.infra]]
name = "Supabase Studio"
url = "http://localhost:54323"

[[dev.urls.apps]]
name = "Dashboard"
url = "http://localhost:3000"

[[dev.urls.apps]]
name = "API"
url = "http://localhost:8080"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        let urls = manifest.dev.urls.unwrap();
        assert_eq!(urls.infra.len(), 1);
        assert_eq!(urls.infra[0].name, "Supabase Studio");
        assert_eq!(urls.infra[0].url, "http://localhost:54323");
        assert_eq!(urls.apps.len(), 2);
        assert_eq!(urls.apps[0].name, "Dashboard");
        assert_eq!(urls.apps[1].name, "API");
    }

    #[test]
    fn test_ensure_env_file_copies_example() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = std::panic::catch_unwind(|| {
            // Create .env.example
            std::fs::write(".env.example", "DATABASE_URL=postgres://localhost").unwrap();

            // .env should not exist yet
            assert!(!Path::new(".env").exists());

            ensure_env_file();

            // .env should now exist with same content
            assert!(Path::new(".env").exists());
            let content = std::fs::read_to_string(".env").unwrap();
            assert_eq!(content, "DATABASE_URL=postgres://localhost");
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_ensure_env_file_noop_when_env_exists() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = std::panic::catch_unwind(|| {
            std::fs::write(".env.example", "NEW_VALUE=true").unwrap();
            std::fs::write(".env", "EXISTING=keep").unwrap();

            ensure_env_file();

            // .env should retain original content (not overwritten)
            let content = std::fs::read_to_string(".env").unwrap();
            assert_eq!(content, "EXISTING=keep");
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_ensure_env_file_noop_when_no_example() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        let result = std::panic::catch_unwind(|| {
            // No .env.example → nothing should happen
            ensure_env_file();
            assert!(!Path::new(".env").exists());
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_dev_section_post_up_default_empty() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert!(manifest.dev.post_up.is_empty());
    }

    #[test]
    fn test_dev_section_post_up_with_hooks() {
        let manifest_content = r#"
version = 1

[workspace]
name = "test"

[dev]
post_up = [
    "docker compose exec workspace pnpm db:migrate",
    "docker compose exec workspace pnpm db:seed",
]
"#;
        let manifest: Manifest = toml::from_str(manifest_content).unwrap();
        assert_eq!(manifest.dev.post_up.len(), 2);
        assert_eq!(
            manifest.dev.post_up[0],
            "docker compose exec workspace pnpm db:migrate"
        );
        assert_eq!(
            manifest.dev.post_up[1],
            "docker compose exec workspace pnpm db:seed"
        );
    }

    #[test]
    fn test_extra_args_blocks_shell_injection_semicolon() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        // Create a minimal manifest.toml
        std::fs::write(
            "manifest.toml",
            r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm"
service = "workspace"
image = "node:22"
[commands]
test = "echo safe"
"#,
        )
        .unwrap();

        let result = std::panic::catch_unwind(|| {
            let result = run("test", &["; rm -rf /".to_string()]);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("Shell metacharacters"),
                "Expected shell metacharacter error, got: {}",
                err
            );
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_extra_args_blocks_shell_injection_pipe() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        std::fs::write(
            "manifest.toml",
            r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm"
service = "workspace"
image = "node:22"
[commands]
test = "echo safe"
"#,
        )
        .unwrap();

        let result = std::panic::catch_unwind(|| {
            let result = run("test", &["| cat /etc/passwd".to_string()]);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("Shell metacharacters"),
                "Expected shell metacharacter error, got: {}",
                err
            );
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_extra_args_blocks_command_substitution() {
        let _guard = DIR_LOCK.lock().unwrap();
        let dir = tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();

        std::fs::write(
            "manifest.toml",
            r#"
version = 1
[workspace]
name = "test"
package_manager = "pnpm"
service = "workspace"
image = "node:22"
[commands]
test = "echo safe"
"#,
        )
        .unwrap();

        let result = std::panic::catch_unwind(|| {
            let result = run("test", &["$(whoami)".to_string()]);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(
                err.contains("Shell metacharacters"),
                "Expected shell metacharacter error, got: {}",
                err
            );
        });

        std::env::set_current_dir(original_dir).unwrap();
        result.unwrap();
    }

    #[test]
    fn test_extract_host_port_env_var_default() {
        let svc = crate::manifest::ServiceConfig {
            image: String::new(),
            port: None,
            ports: vec!["${CORPORATE_PORT:-3000}:3000".to_string()],
            command: None,
            volumes: vec![],
            env: IndexMap::new(),
            profiles: vec![],
            depends_on: vec![],
            restart: None,
            shm_size: None,
            container_name: None,
            working_dir: None,
            extra_hosts: vec![],
            deploy: None,
            watch: vec![],
            extends: None,
            devices: vec![],
            runtime: None,
            gpu: None,
        };
        assert_eq!(extract_host_port_from_service(&svc), Some(3000));
    }

    #[test]
    fn test_extract_host_port_plain_number() {
        let svc = crate::manifest::ServiceConfig {
            image: String::new(),
            port: None,
            ports: vec!["8080:80".to_string()],
            command: None,
            volumes: vec![],
            env: IndexMap::new(),
            profiles: vec![],
            depends_on: vec![],
            restart: None,
            shm_size: None,
            container_name: None,
            working_dir: None,
            extra_hosts: vec![],
            deploy: None,
            watch: vec![],
            extends: None,
            devices: vec![],
            runtime: None,
            gpu: None,
        };
        assert_eq!(extract_host_port_from_service(&svc), Some(8080));
    }

    #[test]
    fn test_extract_host_port_fallback_to_port_field() {
        let svc = crate::manifest::ServiceConfig {
            image: String::new(),
            port: Some(9090),
            ports: vec![],
            command: None,
            volumes: vec![],
            env: IndexMap::new(),
            profiles: vec![],
            depends_on: vec![],
            restart: None,
            shm_size: None,
            container_name: None,
            working_dir: None,
            extra_hosts: vec![],
            deploy: None,
            watch: vec![],
            extends: None,
            devices: vec![],
            runtime: None,
            gpu: None,
        };
        assert_eq!(extract_host_port_from_service(&svc), Some(9090));
    }

    #[test]
    fn test_extract_host_port_no_ports() {
        let svc = crate::manifest::ServiceConfig {
            image: String::new(),
            port: None,
            ports: vec![],
            command: None,
            volumes: vec![],
            env: IndexMap::new(),
            profiles: vec![],
            depends_on: vec![],
            restart: None,
            shm_size: None,
            container_name: None,
            working_dir: None,
            extra_hosts: vec![],
            deploy: None,
            watch: vec![],
            extends: None,
            devices: vec![],
            runtime: None,
            gpu: None,
        };
        assert_eq!(extract_host_port_from_service(&svc), None);
    }
}
