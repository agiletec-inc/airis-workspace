use anyhow::Result;
use colored::Colorize;
use serde_json::Value;
use std::process::{Command, Stdio};

use crate::manifest::Manifest;

use super::compose::collect_all_compose_files;
use super::traefik::{get_docker_traefik_routers, parse_traefik_routers};
use super::{REACHABILITY_POLL_INTERVAL_SECS, TCP_CONNECT_TIMEOUT_MS, TCP_READ_TIMEOUT_MS};

/// Discovered service information
#[derive(Debug, Clone)]
pub(super) struct DiscoveredService {
    pub name: String,
    pub url: String,
    pub is_reachable: bool,
}

/// Extract the host port from a manifest ServiceConfig.
/// Handles formats: "${VAR:-DEFAULT}:CONTAINER", "HOST:CONTAINER", single port
pub(super) fn extract_host_port_from_service(
    svc: &crate::manifest::ServiceConfig,
) -> Option<u16> {
    // Try ports array first, then fallback to deprecated port field
    let port_str = svc.ports.first().map(|s| s.as_str());
    if let Some(port_str) = port_str {
        // Handle ${VAR:-DEFAULT}:CONTAINER format
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
pub(super) fn parse_service_ports_from_config(config: &Value) -> Vec<(String, u16)> {
    let mut results = Vec::new();

    if let Some(services) = config.get("services").and_then(|s| s.as_object()) {
        for (svc_name, svc_config) in services {
            let display_name = svc_config
                .get("container_name")
                .and_then(|c| c.as_str())
                .unwrap_or(svc_name)
                .to_string();

            if let Some(ports) = svc_config.get("ports").and_then(|p| p.as_array()) {
                for port in ports {
                    if let Some(published) = port.get("published") {
                        let p = published
                            .as_u64()
                            .map(|p| p as u16)
                            .or_else(|| published.as_str().and_then(|s| s.parse().ok()));
                        if let Some(host_port) = p
                            && host_port > 0
                        {
                            results.push((display_name.clone(), host_port));
                        }
                    } else if let Some(port_str) = port.as_str() {
                        let parts: Vec<&str> = port_str.split(':').collect();
                        let host_port = match parts.len() {
                            2 => parts[0].parse::<u16>().ok(),
                            3 => parts[1].parse::<u16>().ok(),
                            _ => None,
                        };
                        if let Some(p) = host_port
                            && p > 0
                        {
                            results.push((display_name.clone(), p));
                        }
                    }
                }
            }
        }
    }

    results
}

/// Discover service URLs from compose file port mappings
pub(super) fn discover_compose_port_urls(compose_files: &[String]) -> Vec<DiscoveredService> {
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
            && let Ok(config) = serde_json::from_slice::<Value>(&output.stdout)
        {
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

    services
}

/// Display URLs discovered from compose file port mappings (no manifest required)
pub(super) fn display_compose_urls(compose_files: &[String]) {
    let mut services = discover_compose_port_urls(compose_files);
    wait_for_services_reachable(&mut services, 30);
    display_url_table(&services);
}

/// Wait for services to become reachable, polling every REACHABILITY_POLL_INTERVAL_SECS
/// until all are reachable or timeout_secs expires. If timeout_secs is 0, check once only.
pub(super) fn wait_for_services_reachable(
    services: &mut [DiscoveredService],
    timeout_secs: u64,
) {
    use std::time::{Duration, Instant};

    if services.is_empty() {
        return;
    }

    for svc in services.iter_mut() {
        if !svc.is_reachable {
            svc.is_reachable = is_service_reachable(&svc.url);
        }
    }

    if timeout_secs == 0 || services.iter().all(|s| s.is_reachable) {
        return;
    }

    let unreachable_count = services.iter().filter(|s| !s.is_reachable).count();
    println!(
        "   {} Waiting for {} service(s) to become reachable (timeout: {}s)...",
        "⏳".dimmed(),
        unreachable_count,
        timeout_secs
    );

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_secs(REACHABILITY_POLL_INTERVAL_SECS));

        for svc in services.iter_mut() {
            if !svc.is_reachable {
                svc.is_reachable = is_service_reachable(&svc.url);
            }
        }

        if services.iter().all(|s| s.is_reachable) {
            break;
        }
    }
}

/// Check if a service is reachable via HTTP
pub(super) fn is_service_reachable(url: &str) -> bool {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

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

    let connect_host = if host.ends_with(".localhost") {
        "127.0.0.1"
    } else {
        host
    };

    let addr = match format!("{}:{}", connect_host, port).parse() {
        Ok(a) => a,
        Err(_) => return false,
    };

    let mut stream =
        match TcpStream::connect_timeout(&addr, Duration::from_millis(TCP_CONNECT_TIMEOUT_MS)) {
            Ok(s) => s,
            Err(_) => return false,
        };

    let _ = stream.set_read_timeout(Some(Duration::from_millis(TCP_READ_TIMEOUT_MS)));

    let request = format!(
        "HEAD {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        path, host
    );

    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }

    let mut buffer = [0u8; 128];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return false,
    };

    let response = String::from_utf8_lossy(&buffer[..bytes_read]);

    response.contains(" 200 ")
        || response.contains(" 201 ")
        || response.contains(" 204 ")
        || response.contains(" 301 ")
        || response.contains(" 302 ")
        || response.contains(" 307 ")
        || response.contains(" 308 ")
}

/// Display URL table
pub(super) fn display_url_table(services: &[DiscoveredService]) {
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

/// Display service URLs - 3-tier priority discovery
pub(super) fn display_service_urls(manifest: &Manifest) -> Result<()> {
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

    // Tier 2: manifest.service port definitions
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
                                        return published.as_u64().map(|p| p as u16).or_else(
                                            || published.as_str().and_then(|s| s.parse().ok()),
                                        );
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
            let traefik_dir = std::path::Path::new(traefik_file.as_str())
                .parent()
                .unwrap_or(std::path::Path::new("."));
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
                    .replace(['-', '_'], " ")
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

    wait_for_services_reachable(&mut all_services, manifest.dev.reachability_timeout);

    display_url_table(&all_services);
    Ok(())
}

/// Condense docker status string for compact display
pub(super) fn condense_status(status: &str) -> String {
    let s = status.trim();

    if let Some(rest) = s.strip_prefix("Up ") {
        if rest.starts_with("About an hour") {
            return "Up ~1h".to_string();
        }
        if rest.starts_with("About a minute") {
            return "Up ~1m".to_string();
        }

        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() >= 2 && parts[0].parse::<u64>().is_ok() {
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

    s.to_string()
}
