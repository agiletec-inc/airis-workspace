//! Manifest validation: syntax, app paths, port conflicts, required env vars, env patterns

use anyhow::{Context, Result, bail};
use colored::Colorize;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::manifest::{MANIFEST_FILE, Manifest};

/// Validate manifest.toml: syntax, app paths, port conflicts, required env vars
pub fn validate_manifest() -> Result<()> {
    validate_manifest_impl(false)
}

pub fn validate_manifest_impl(quiet: bool) -> Result<()> {
    if !quiet {
        println!("{}", "🔍 Validating manifest.toml...".bright_blue());
    }

    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        bail!(
            "manifest.toml not found. Create one (see docs/manifest.md) or ask Claude Code via /airis:init."
        );
    }

    // 1. Syntax validation (parse TOML)
    let manifest = Manifest::load(manifest_path).context("Failed to parse manifest.toml")?;
    if !quiet {
        println!("  {} Syntax valid", "✅".green());
    }

    let mut failures = 0;

    // 2. Validate app paths exist
    for app_name in manifest.apps.keys() {
        let app_path = Path::new("apps").join(app_name);
        if !app_path.exists() {
            if !quiet {
                println!("  {} App path not found: apps/{}", "❌".red(), app_name);
            }
            failures += 1;
        }
    }
    if !quiet && (manifest.apps.is_empty() || failures == 0) {
        println!("  {} App paths valid", "✅".green());
    }

    // 3. Validate lib paths exist
    for lib_name in manifest.libs.keys() {
        let lib_path = Path::new("libs").join(lib_name);
        if !lib_path.exists() {
            if !quiet {
                println!("  {} Lib path not found: libs/{}", "❌".red(), lib_name);
            }
            failures += 1;
        }
    }
    if !quiet && (manifest.libs.is_empty() || failures == 0) {
        println!("  {} Lib paths valid", "✅".green());
    }

    // 4. Check for port conflicts in services
    let mut ports: HashSet<u16> = HashSet::new();
    let mut port_conflicts = 0;
    for (service_name, service) in &manifest.service {
        if let Some(port) = service.port
            && !ports.insert(port)
        {
            if !quiet {
                println!(
                    "  {} Port conflict: {} uses port {} (already in use)",
                    "❌".red(),
                    service_name,
                    port
                );
            }
            port_conflicts += 1;
        }
    }
    if !quiet && port_conflicts == 0 {
        println!("  {} No port conflicts", "✅".green());
    }
    failures += port_conflicts;

    // 5. Validate required environment variables from [env] section
    if !manifest.env.required.is_empty() {
        let env_failures = validate_required_env_vars_impl(&manifest, quiet)?;
        failures += env_failures;
    }

    // 6. Validate env patterns if defined
    let pattern_failures = validate_env_patterns_impl(&manifest, quiet)?;
    failures += pattern_failures;

    if failures > 0 {
        bail!("manifest.toml validation failed with {} errors", failures);
    }

    if !quiet {
        println!("{}", "✅ manifest.toml validation passed!".green());
    }
    Ok(())
}

/// Validate required environment variables are set
pub fn validate_required_env_vars_impl(manifest: &Manifest, quiet: bool) -> Result<usize> {
    let mut failures = 0;

    if !quiet {
        println!(
            "  {} Checking required environment variables...",
            "🔍".dimmed()
        );
    }

    for var_name in &manifest.env.required {
        if std::env::var(var_name).is_err() {
            // Also check .env file in project root
            let env_file = Path::new(".env");
            let mut found = false;
            if env_file.exists() {
                let content = fs::read_to_string(env_file)?;
                for line in content.lines() {
                    if line.starts_with(&format!("{}=", var_name)) {
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                if !quiet {
                    let description = manifest
                        .env
                        .validation
                        .get(var_name)
                        .and_then(|v| v.description.as_ref())
                        .map(|d| format!(" ({})", d))
                        .unwrap_or_default();
                    println!(
                        "  {} Missing required env var: {}{}",
                        "❌".red(),
                        var_name,
                        description
                    );
                }
                failures += 1;
            }
        }
    }

    if !quiet && failures == 0 && !manifest.env.required.is_empty() {
        println!("  {} Required env vars present", "✅".green());
    }

    Ok(failures)
}

/// Validate environment variable patterns
pub fn validate_env_patterns_impl(manifest: &Manifest, quiet: bool) -> Result<usize> {
    let mut failures = 0;

    for (var_name, validation) in &manifest.env.validation {
        if let Some(pattern) = &validation.pattern {
            // Get the value from environment or .env file
            let value = std::env::var(var_name).ok().or_else(|| {
                let env_file = Path::new(".env");
                if env_file.exists()
                    && let Ok(content) = fs::read_to_string(env_file)
                {
                    for line in content.lines() {
                        if line.starts_with(&format!("{}=", var_name)) {
                            return line.split('=').nth(1).map(|s| s.to_string());
                        }
                    }
                }
                None
            });

            if let Some(val) = value {
                match Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(&val) {
                            if !quiet {
                                let desc = validation
                                    .description
                                    .as_deref()
                                    .unwrap_or("invalid format");
                                println!(
                                    "  {} {} does not match pattern '{}': {}",
                                    "❌".red(),
                                    var_name,
                                    pattern,
                                    desc
                                );
                            }
                            failures += 1;
                        }
                    }
                    Err(e) => {
                        if !quiet {
                            println!(
                                "  {} Invalid regex pattern for {}: {}",
                                "⚠️".yellow(),
                                var_name,
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    if !quiet && failures == 0 && !manifest.env.validation.is_empty() {
        println!("  {} Env var patterns valid", "✅".green());
    }

    Ok(failures)
}
