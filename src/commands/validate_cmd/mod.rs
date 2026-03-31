//! Validate command: workspace configuration validation
//!
//! Validates Traefik ports, networks, environment variables, and manifest.toml.

mod deps;
mod env;
mod manifest_check;
mod networks;
mod ports;

#[cfg(test)]
mod tests;

use anyhow::{Result, bail};
use colored::Colorize;
use serde::Serialize;

pub use deps::{validate_dependencies, validate_dependencies_impl};
pub use env::{validate_env, validate_env_impl};
pub use manifest_check::{validate_manifest, validate_manifest_impl};
pub use networks::{validate_networks, validate_networks_impl};
pub use ports::{validate_ports, validate_ports_impl};

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

/// Structured validation result for JSON output
#[derive(Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub checks: Vec<ValidationCheck>,
    pub summary: String,
}

#[derive(Serialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// Run validation
pub fn run(action: ValidateAction, json_output: bool) -> Result<()> {
    if json_output {
        run_json(action)
    } else {
        run_human(action)
    }
}

fn run_human(action: ValidateAction) -> Result<()> {
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

/// Type alias for validation action tuple to reduce complexity
type ValidationAction<'a> = (&'a str, Box<dyn Fn() -> Result<()>>, &'a str);

fn run_json(action: ValidateAction) -> Result<()> {
    let mut checks = Vec::new();

    // Run the requested validations and collect results (quiet mode)
    let actions: Vec<ValidationAction> = match action {
        ValidateAction::Manifest => vec![(
            "manifest",
            Box::new(|| validate_manifest_impl(true)) as Box<dyn Fn() -> Result<()>>,
            "Run `airis init` to regenerate manifest.toml",
        )],
        ValidateAction::Ports => vec![(
            "ports",
            Box::new(|| validate_ports_impl(true)),
            "Use `expose:` instead of `ports:` in compose.yml",
        )],
        ValidateAction::Networks => vec![(
            "networks",
            Box::new(|| validate_networks_impl(true)),
            "Check Traefik network configuration",
        )],
        ValidateAction::Env => vec![(
            "env",
            Box::new(|| validate_env_impl(true)),
            "Check .env files for disallowed public keys",
        )],
        ValidateAction::Dependencies | ValidateAction::Architecture => vec![(
            "dependencies",
            Box::new(|| validate_dependencies_impl(true)),
            "Run `npx dependency-cruiser` to check architecture",
        )],
        ValidateAction::All => vec![
            (
                "manifest",
                Box::new(|| validate_manifest_impl(true)) as Box<dyn Fn() -> Result<()>>,
                "Run `airis init` to regenerate",
            ),
            (
                "ports",
                Box::new(|| validate_ports_impl(true)),
                "Use `expose:` instead of `ports:`",
            ),
            (
                "networks",
                Box::new(|| validate_networks_impl(true)),
                "Check Traefik network config",
            ),
            (
                "env",
                Box::new(|| validate_env_impl(true)),
                "Check .env files",
            ),
            (
                "dependencies",
                Box::new(|| validate_dependencies_impl(true)),
                "Run dependency-cruiser",
            ),
        ],
    };

    for (name, validator, fix_hint) in actions {
        let result = validator();
        checks.push(ValidationCheck {
            name: name.to_string(),
            passed: result.is_ok(),
            error: result.as_ref().err().map(|e| e.to_string()),
            fix: if result.is_err() {
                Some(fix_hint.to_string())
            } else {
                None
            },
        });
    }

    let all_passed = checks.iter().all(|c| c.passed);
    let failed_count = checks.iter().filter(|c| !c.passed).count();

    let result = ValidationResult {
        valid: all_passed,
        checks,
        summary: if all_passed {
            "All validations passed".to_string()
        } else {
            format!("{} validation(s) failed", failed_count)
        },
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    if !all_passed {
        std::process::exit(1);
    }

    Ok(())
}
