//! Validate command: workspace configuration validation
//!
//! Validates Traefik ports, networks, environment variables, and manifest.toml.

use anyhow::{bail, Context, Result};
use colored::Colorize;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::manifest::{Manifest, MANIFEST_FILE};

/// Validate action types
pub enum ValidateAction {
    Ports,
    Networks,
    Env,
    Dependencies,
    Architecture,
    /// Validate manifest.toml syntax, app paths, port conflicts
    Manifest,
    All,
}

/// Run validation
pub fn run(action: ValidateAction) -> Result<()> {
    match action {
        ValidateAction::Ports => validate_ports(),
        ValidateAction::Networks => validate_networks(),
        ValidateAction::Env => validate_env(),
        ValidateAction::Dependencies | ValidateAction::Architecture => validate_dependencies(),
        ValidateAction::Manifest => validate_manifest(),
        ValidateAction::All => {
            let mut failures = 0;

            println!("{}", "🔍 Running all validations...".bright_blue());
            println!();

            if let Err(e) = validate_manifest() {
                eprintln!("  {} Manifest validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if let Err(e) = validate_ports() {
                eprintln!("  {} Ports validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if let Err(e) = validate_networks() {
                eprintln!("  {} Networks validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if let Err(e) = validate_env() {
                eprintln!("  {} Env validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if let Err(e) = validate_dependencies() {
                eprintln!("  {} Dependencies validation failed: {}", "❌".red(), e);
                failures += 1;
            }

            if failures > 0 {
                bail!("Validation failed with {} errors", failures);
            }

            println!();
            println!("{}", "✅ All validations passed!".green());
            Ok(())
        }
    }
}

/// Validate that no ports: mapping exists in application docker-compose files
fn validate_ports() -> Result<()> {
    println!("{}", "🔍 Checking for ports: mapping in application docker-compose files...".bright_blue());

    // Use ripgrep to find ports: mappings
    let output = Command::new("rg")
        .args([
            "-n",
            r"^\s*ports\s*:",
            "--glob", "apps/*/docker-compose*.yml",
            "--glob", "!apps/*/docker-compose.override*.yml",
            ".",
        ])
        .output()
        .context("Failed to run ripgrep")?;

    let matches = String::from_utf8_lossy(&output.stdout);

    if !matches.is_empty() {
        println!();
        println!("{}", "❌ ERROR: Found ports: mapping in application docker-compose.".red());
        println!();
        println!("Found:");
        for line in matches.lines() {
            println!("  {}", line);
        }
        println!();
        println!("   {} Wrong:", "❌".red());
        println!("   ports:");
        println!("     - \"4010:3000\"");
        println!();
        println!("   {} Right:", "✅".green());
        println!("   expose:");
        println!("     - \"3000\"");
        println!("   labels:");
        println!("     - traefik.enable=true");
        println!("     - traefik.http.routers.app.rule=Host(`app.localhost`)");
        println!("     - traefik.http.services.app.loadbalancer.server.port=3000");
        println!();
        println!("   Exception: Only allowed in:");
        println!("   - Infrastructure (traefik/, supabase/)");
        println!("   - Override files (compose.*.override.yml, compose.dev.yml)");

        bail!("Found ports: mapping in application docker-compose files");
    }

    println!("{}", "✅ No ports: mapping found in application docker-compose.".green());
    Ok(())
}

/// Validate Traefik network wiring in application docker-compose files
fn validate_networks() -> Result<()> {
    println!("{}", "🔍 Checking Traefik network wiring in apps/*/docker-compose.yml...".bright_blue());

    let apps_dir = Path::new("apps");
    if !apps_dir.exists() {
        println!("  {} No apps directory found", "⏭️".dimmed());
        return Ok(());
    }

    let mut failures = 0;
    let proxy_network = std::env::var("EXTERNAL_PROXY_NETWORK").unwrap_or_else(|_| "coolify".to_string());

    for entry in fs::read_dir(apps_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let compose_file = path.join("docker-compose.yml");
        if !compose_file.exists() {
            continue;
        }

        let project = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        // Read and parse the compose file
        let content = fs::read_to_string(&compose_file)
            .with_context(|| format!("Failed to read {}", compose_file.display()))?;

        // Check for required network configurations using simple string matching
        // A more robust solution would use a YAML parser

        // Check for agiletec_default network
        if !content.contains("agiletec_default") {
            println!("  {} {}: networks.default should reference 'agiletec_default'", "❌".red(), project);
            failures += 1;
        }

        // Check for proxy network
        if !content.contains(&proxy_network) && !content.contains("EXTERNAL_PROXY_NETWORK") {
            println!("  {} {}: networks.proxy should reference '{}' or EXTERNAL_PROXY_NETWORK", "❌".red(), project, proxy_network);
            failures += 1;
        }

        // Check for traefik.docker.network label
        if content.contains("traefik.enable=true")
            && !content.contains("traefik.docker.network=") {
                println!("  {} {}: Traefik-enabled services need traefik.docker.network label", "❌".red(), project);
                failures += 1;
            }
    }

    if failures > 0 {
        println!();
        println!("⚠️  Traefik ネットワーク設定を修正してください");
        bail!("Found {} network configuration issues", failures);
    }

    println!("{}", "✅ Traefik network wiring looks good.".green());
    Ok(())
}

/// Validate frontend environment variables
fn validate_env() -> Result<()> {
    println!("{}", "🔍 Checking frontend environment variables...".bright_blue());

    let allowed_keys = vec![
        "NEXT_PUBLIC_SUPABASE_URL",
        "NEXT_PUBLIC_SUPABASE_ANON_KEY",
        "EXPO_PUBLIC_SUPABASE_URL",
        "EXPO_PUBLIC_SUPABASE_ANON_KEY",
    ];

    let mut disallowed = Vec::new();

    // Check .env files in apps
    let apps_dir = Path::new("apps");
    if apps_dir.exists() {
        for entry in fs::read_dir(apps_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Check various .env files
            for env_file in &[".env", ".env.local", ".env.development"] {
                let env_path = path.join(env_file);
                if env_path.exists() {
                    check_env_file(&env_path, &allowed_keys, &mut disallowed)?;
                }
            }
        }
    }

    if !disallowed.is_empty() {
        println!();
        println!("{}", "Disallowed public environment keys detected:".red());
        for item in &disallowed {
            println!("  - {}", item);
        }
        println!();
        println!("Allowed keys: {}", allowed_keys.join(", "));
        bail!("Found {} disallowed public environment keys", disallowed.len());
    }

    println!("{}", "✅ Environment variables look good.".green());
    Ok(())
}

/// Check a single .env file for disallowed public keys
fn check_env_file(path: &Path, allowed: &[&str], disallowed: &mut Vec<String>) -> Result<()> {
    let content = fs::read_to_string(path)?;

    for line in content.lines() {
        // Skip comments and empty lines
        if line.trim().starts_with('#') || line.trim().is_empty() {
            continue;
        }

        // Extract key
        if let Some(key) = line.split('=').next() {
            let key = key.trim();

            // Check if it's a public key
            if (key.starts_with("NEXT_PUBLIC_") || key.starts_with("EXPO_PUBLIC_"))
                && !allowed.contains(&key) {
                    disallowed.push(format!("{}: {}", path.display(), key));
                }
        }
    }

    Ok(())
}


/// Validate dependency architecture rules
/// Checks that apps only depend on libs (public API), and no cross-app dependencies exist
fn validate_dependencies() -> Result<()> {
    println!("{}", "🔍 Validating dependency architecture...".bright_blue());

    // Check if dependency-cruiser config exists
    let config_path = Path::new("tools/dependency-cruiser.cjs");
    if !config_path.exists() {
        println!("  {} dependency-cruiser config not found, skipping", "⏭️".yellow());
        return Ok(());
    }

    // Check if dependency-cruiser is installed
    let check = Command::new("npx")
        .args(["dependency-cruiser", "--version"])
        .output();

    if check.is_err() {
        println!("  {} dependency-cruiser not installed, skipping", "⏭️".yellow());
        println!("  {} Install with: pnpm add -D dependency-cruiser", "💡".dimmed());
        return Ok(());
    }

    // Run dependency-cruiser
    println!("  {} Running dependency-cruiser...", "⚙️".dimmed());
    let status = Command::new("npx")
        .args([
            "dependency-cruiser",
            "--config",
            "tools/dependency-cruiser.cjs",
            "--output-type",
            "err",
            "apps",
            "libs",
        ])
        .status()
        .context("Failed to run dependency-cruiser")?;

    if !status.success() {
        bail!("Dependency architecture validation failed. Fix violations above.");
    }

    println!("  {} No architecture violations found", "✅".green());
    Ok(())
}

/// Validate manifest.toml: syntax, app paths, port conflicts, required env vars
fn validate_manifest() -> Result<()> {
    println!("{}", "🔍 Validating manifest.toml...".bright_blue());

    let manifest_path = Path::new(MANIFEST_FILE);
    if !manifest_path.exists() {
        bail!("manifest.toml not found. Run `airis init` to create one.");
    }

    // 1. Syntax validation (parse TOML)
    let manifest = Manifest::load(manifest_path)
        .context("Failed to parse manifest.toml")?;
    println!("  {} Syntax valid", "✅".green());

    let mut failures = 0;

    // 2. Validate app paths exist
    for app_name in manifest.apps.keys() {
        let app_path = Path::new("apps").join(app_name);
        if !app_path.exists() {
            println!("  {} App path not found: apps/{}", "❌".red(), app_name);
            failures += 1;
        }
    }
    if manifest.apps.is_empty() || failures == 0 {
        println!("  {} App paths valid", "✅".green());
    }

    // 3. Validate lib paths exist
    for lib_name in manifest.libs.keys() {
        let lib_path = Path::new("libs").join(lib_name);
        if !lib_path.exists() {
            println!("  {} Lib path not found: libs/{}", "❌".red(), lib_name);
            failures += 1;
        }
    }
    if manifest.libs.is_empty() || failures == 0 {
        println!("  {} Lib paths valid", "✅".green());
    }

    // 4. Check for port conflicts in services
    let mut ports: HashSet<u16> = HashSet::new();
    let mut port_conflicts = 0;
    for (service_name, service) in &manifest.service {
        if let Some(port) = service.port {
            if !ports.insert(port) {
                println!("  {} Port conflict: {} uses port {} (already in use)", "❌".red(), service_name, port);
                port_conflicts += 1;
            }
        }
    }
    if port_conflicts == 0 {
        println!("  {} No port conflicts", "✅".green());
    }
    failures += port_conflicts;

    // 5. Validate required environment variables from [env] section
    if !manifest.env.required.is_empty() {
        let env_failures = validate_required_env_vars(&manifest)?;
        failures += env_failures;
    }

    // 6. Validate env patterns if defined
    let pattern_failures = validate_env_patterns(&manifest)?;
    failures += pattern_failures;

    if failures > 0 {
        bail!("manifest.toml validation failed with {} errors", failures);
    }

    println!("{}", "✅ manifest.toml validation passed!".green());
    Ok(())
}

/// Validate required environment variables are set
fn validate_required_env_vars(manifest: &Manifest) -> Result<usize> {
    let mut failures = 0;

    println!("  {} Checking required environment variables...", "🔍".dimmed());

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
                let description = manifest.env.validation.get(var_name)
                    .and_then(|v| v.description.as_ref())
                    .map(|d| format!(" ({})", d))
                    .unwrap_or_default();
                println!("  {} Missing required env var: {}{}", "❌".red(), var_name, description);
                failures += 1;
            }
        }
    }

    if failures == 0 && !manifest.env.required.is_empty() {
        println!("  {} Required env vars present", "✅".green());
    }

    Ok(failures)
}

/// Validate environment variable patterns
fn validate_env_patterns(manifest: &Manifest) -> Result<usize> {
    let mut failures = 0;

    for (var_name, validation) in &manifest.env.validation {
        if let Some(pattern) = &validation.pattern {
            // Get the value from environment or .env file
            let value = std::env::var(var_name).ok().or_else(|| {
                let env_file = Path::new(".env");
                if env_file.exists() {
                    if let Ok(content) = fs::read_to_string(env_file) {
                        for line in content.lines() {
                            if line.starts_with(&format!("{}=", var_name)) {
                                return line.split('=').nth(1).map(|s| s.to_string());
                            }
                        }
                    }
                }
                None
            });

            if let Some(val) = value {
                match Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(&val) {
                            let desc = validation.description.as_deref().unwrap_or("invalid format");
                            println!("  {} {} does not match pattern '{}': {}", "❌".red(), var_name, pattern, desc);
                            failures += 1;
                        }
                    }
                    Err(e) => {
                        println!("  {} Invalid regex pattern for {}: {}", "⚠️".yellow(), var_name, e);
                    }
                }
            }
        }
    }

    if failures == 0 && !manifest.env.validation.is_empty() {
        println!("  {} Env var patterns valid", "✅".green());
    }

    Ok(failures)
}
