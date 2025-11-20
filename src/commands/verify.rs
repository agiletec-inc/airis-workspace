//! Verify command: system health checks for Docker services
//!
//! Performs connectivity checks for Traefik, Kong, and workspace services.

use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;

/// Health check result
struct CheckResult {
    name: String,
    url: String,
    passed: bool,
    status_code: String,
}

/// Run the verify command
pub fn run() -> Result<()> {
    println!("{}", "🔍 Running system health checks...".bright_blue());
    println!();

    let mut failures = 0;

    // Check Traefik external endpoints
    println!("{}", "==> Traefik (external)".yellow());
    let traefik_checks = vec![
        ("API /health", "http://localhost:8000/health", vec!["200", "401"]),
    ];

    for (name, url, expected) in traefik_checks {
        let result = check_endpoint(name, url, &expected);
        print_result(&result);
        if !result.passed {
            failures += 1;
        }
    }

    println!();

    // Check workspace container -> Kong (internal)
    println!("{}", "==> Workspace container -> Kong (internal)".yellow());

    // Find workspace container
    let workspace_container = find_workspace_container();

    match workspace_container {
        Some(container) => {
            let kong_checks = vec![
                ("Kong /auth health", "http://kong:8000/auth/v1/health", vec!["200", "401"]),
                ("Kong /rest root", "http://kong:8000/rest/v1", vec!["200", "401"]),
                ("Kong /storage health", "http://kong:8000/storage/v1/health", vec!["200", "401", "400"]),
            ];

            for (name, url, expected) in kong_checks {
                let result = check_endpoint_in_container(&container, name, url, &expected);
                print_result(&result);
                if !result.passed {
                    failures += 1;
                }
            }
        }
        None => {
            println!("{}  workspace container (agiletec-workspace) not running", "FAIL".red());
            failures += 1;
        }
    }

    println!();

    // Check Traefik 5xx errors
    println!("{}", "==> Traefik recent 5xx (last 30s)".yellow());
    let has_5xx = check_traefik_5xx();
    if has_5xx {
        println!("{}  Found 5xx responses in Traefik logs", "FAIL".red());
        failures += 1;
    } else {
        println!("{}", "None".green());
    }

    println!();

    // Final result
    if failures > 0 {
        println!("{} VERIFY FAILED ({})", "✗".red(), failures);
        std::process::exit(1);
    }

    println!("{}", "✓ All endpoint checks passed".green());
    Ok(())
}

/// Check an endpoint from the host
fn check_endpoint(name: &str, url: &str, expected: &[&str]) -> CheckResult {
    let output = Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", url])
        .output();

    let status_code = match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => "000".to_string(),
    };

    let passed = expected.contains(&status_code.as_str());

    CheckResult {
        name: name.to_string(),
        url: url.to_string(),
        passed,
        status_code,
    }
}

/// Check an endpoint from inside a container
fn check_endpoint_in_container(container: &str, name: &str, url: &str, expected: &[&str]) -> CheckResult {
    let cmd = format!(
        "curl -s -o /dev/null -w '%{{http_code}}' {}",
        url
    );

    let output = Command::new("docker")
        .args(["exec", container, "sh", "-c", &cmd])
        .output();

    let status_code = match output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => "000".to_string(),
    };

    let passed = expected.contains(&status_code.as_str());

    CheckResult {
        name: name.to_string(),
        url: url.to_string(),
        passed,
        status_code,
    }
}

/// Find the workspace container
fn find_workspace_container() -> Option<String> {
    let output = Command::new("docker")
        .args(["ps", "--format", "{{.Names}}"])
        .output()
        .ok()?;

    let names = String::from_utf8_lossy(&output.stdout);

    for name in names.lines() {
        if name.contains("workspace") {
            return Some(name.to_string());
        }
    }

    None
}

/// Check for 5xx errors in Traefik logs
fn check_traefik_5xx() -> bool {
    let output = Command::new("docker")
        .args(["logs", "traefik", "--since=30s"])
        .output();

    match output {
        Ok(out) => {
            let logs = String::from_utf8_lossy(&out.stderr);
            logs.contains(" 502 ") || logs.contains(" 503 ") || logs.contains(" 504 ")
        }
        Err(_) => false,
    }
}

/// Print a check result
fn print_result(result: &CheckResult) {
    if result.passed {
        println!(
            "{}  {} ({}) -> {}",
            "PASS".green(),
            result.name,
            result.url,
            result.status_code
        );
    } else {
        println!(
            "{}  {} ({}) -> {}",
            "FAIL".red(),
            result.name,
            result.url,
            result.status_code
        );
    }
}
