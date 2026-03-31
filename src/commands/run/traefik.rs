use serde_json::Value;
use std::process::{Command, Stdio};

/// Parse Traefik dynamic config to get routers
pub(super) fn parse_traefik_routers(traefik_dir: &str) -> Vec<(String, String, String)> {
    // Returns: (router_name, host, path_prefix)
    let routers_file = format!("{}/dynamic/routers.yml", traefik_dir);

    let content = match std::fs::read_to_string(&routers_file) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();

    // Parse YAML using serde_yml
    let yaml: serde_yml::Value = match serde_yml::from_str(&content) {
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
            let rule = config.get("rule").and_then(|r| r.as_str()).unwrap_or("");

            // Extract host from rule
            let host = host_regex
                .as_ref()
                .and_then(|re| re.captures(rule))
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();

            // Extract path prefix from rule
            let path = path_regex
                .as_ref()
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

/// Get Traefik routers from Docker labels on running containers
pub(super) fn get_docker_traefik_routers(workspace_name: &str) -> Vec<(String, String, String)> {
    // Returns: (router_name, host, path_prefix)
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let containers: Vec<String> = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
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
            && let Ok(labels) =
                serde_json::from_slice::<serde_json::Map<String, Value>>(&output.stdout)
        {
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
                    let entrypoint_key =
                        format!("traefik.http.routers.{}.entrypoints", router_name);
                    let entrypoint = labels
                        .get(&entrypoint_key)
                        .and_then(|v| v.as_str())
                        .unwrap_or("web");

                    if entrypoint != "web" {
                        continue;
                    }

                    // Extract host
                    let host = host_regex
                        .as_ref()
                        .and_then(|re| re.captures(rule))
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .unwrap_or_default();

                    // Extract path prefix
                    let path = path_regex
                        .as_ref()
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
