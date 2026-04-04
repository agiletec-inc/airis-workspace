//! Policy Gates: Pre-deployment validation checks
//!
//! Validates workspace state before bundle/deploy:
//! - Git clean state
//! - Required environment variables
//! - Forbidden files detection
//! - Secret scanning

pub(crate) mod checkers;

#[cfg(test)]
mod tests;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use checkers::{
    check_banned_env_vars, check_forbidden_files, check_forbidden_patterns, check_git_clean,
    check_mock_patterns, check_required_env, check_type_enforcement, scan_secrets,
};

use crate::manifest::{MANIFEST_FILE, Manifest};

/// Policy configuration from .airis/policies.toml
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PolicyConfig {
    #[serde(default)]
    pub gates: GatesConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GatesConfig {
    /// Require git working directory to be clean
    #[serde(default)]
    pub require_clean_git: bool,
    /// Required environment variables
    #[serde(default)]
    pub require_env: Vec<String>,
    /// Forbidden files (exact paths)
    #[serde(default)]
    pub forbid_files: Vec<String>,
    /// Forbidden file patterns (glob)
    #[serde(default)]
    pub forbid_patterns: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Scan for secrets in files
    #[serde(default)]
    pub scan_secrets: bool,
    /// Maximum file size in MB (files larger are skipped)
    #[serde(default = "default_max_file_size")]
    pub max_file_size_mb: u64,
}

fn default_max_file_size() -> u64 {
    50
}

/// Policy check result
#[derive(Debug, Default)]
pub struct PolicyResult {
    pub passed: bool,
    pub violations: Vec<PolicyViolation>,
    #[allow(dead_code)]
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct PolicyViolation {
    #[allow(dead_code)]
    pub rule: String,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl PolicyConfig {
    /// Load from .airis/policies.toml
    pub fn load() -> Result<Self> {
        let path = PathBuf::from(".airis/policies.toml");
        if !path.exists() {
            // Return default config if no policies file
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: PolicyConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Generate default policies.toml template
    pub fn template() -> String {
        r#"# Policy Gates Configuration
# Run: airis policy check

[gates]
# Require clean git working directory before bundle/deploy
require_clean_git = true

# Required environment variables (checked at bundle time)
require_env = [
    # "DATABASE_URL",
    # "API_KEY",
]

# Forbidden files - fail if these exist
forbid_files = [
    ".env.local",
    ".env.production",
    "secrets.json",
    "credentials.json",
]

# Forbidden patterns (glob) - fail if matches found
forbid_patterns = [
    "**/*.secret",
    "**/credentials.*",
    "**/.env.*.local",
]

[security]
# Scan for potential secrets in source files
scan_secrets = true

# Skip files larger than this (MB)
max_file_size_mb = 50
"#
        .to_string()
    }
}

/// Initialize policies.toml
pub fn init() -> Result<()> {
    use colored::Colorize;

    let dir = PathBuf::from(".airis");
    let path = dir.join("policies.toml");

    if path.exists() {
        println!("{}", "⚠️  .airis/policies.toml already exists".yellow());
        return Ok(());
    }

    fs::create_dir_all(&dir)?;
    fs::write(&path, PolicyConfig::template())?;

    println!("{}", "✅ Created .airis/policies.toml".green());
    println!("   Edit the file to configure policy gates.");

    Ok(())
}

/// Run policy checks
pub fn check(project: Option<&str>) -> Result<PolicyResult> {
    use colored::Colorize;

    println!("{}", "==================================".bright_blue());
    println!("{}", "airis policy check".bright_blue().bold());
    if let Some(p) = project {
        println!("Project: {}", p.cyan());
    }
    println!("{}", "==================================".bright_blue());

    let config = PolicyConfig::load()?;
    let mut result = PolicyResult {
        passed: true,
        violations: Vec::new(),
        warnings: Vec::new(),
    };

    // 1. Git clean check
    if config.gates.require_clean_git {
        check_git_clean(&mut result)?;
    }

    // 2. Required env check
    if !config.gates.require_env.is_empty() {
        check_required_env(&config.gates.require_env, &mut result);
    }

    // 3. Forbidden files check
    if !config.gates.forbid_files.is_empty() {
        check_forbidden_files(&config.gates.forbid_files, &mut result)?;
    }

    // 4. Forbidden patterns check
    if !config.gates.forbid_patterns.is_empty() {
        check_forbidden_patterns(&config.gates.forbid_patterns, &mut result)?;
    }

    // 5. Secret scanning
    if config.security.scan_secrets {
        scan_secrets(project, config.security.max_file_size_mb, &mut result)?;
    }

    // 6-8. Governance checks from manifest.toml [policy] (with [testing] fallback)
    let manifest_path = std::path::Path::new(MANIFEST_FILE);
    if manifest_path.exists()
        && let Ok(manifest) = Manifest::load(manifest_path)
    {
        let testing = &manifest.policy.testing;

        // 6. Forbidden mock pattern scanning
        if !testing.forbidden_patterns.is_empty() {
            check_mock_patterns(&testing.forbidden_patterns, project, &mut result)?;
        }

        // 7. Type enforcement (DB tests must import generated types)
        if let Some(te) = &testing.type_enforcement {
            check_type_enforcement(
                &te.generated_types_path,
                &te.required_imports,
                project,
                &mut result,
            )?;
        }

        // 8. Banned environment variables (from [policy.security])
        let security = &manifest.policy.security;
        if !security.banned_env_vars.is_empty() {
            check_banned_env_vars(
                &security.banned_env_vars,
                &security.allowed_paths,
                project,
                &mut result,
            )?;
        }

        // Use manifest security settings if available, override .airis/policies.toml
        if security.scan_secrets && !config.security.scan_secrets {
            scan_secrets(project, security.max_file_size_mb, &mut result)?;
        }
    }

    // Print results
    println!();
    if result.violations.is_empty() {
        println!("{}", "✅ All policy checks passed!".green().bold());
    } else {
        let errors: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Error)
            .collect();
        let warnings: Vec<_> = result
            .violations
            .iter()
            .filter(|v| v.severity == Severity::Warning)
            .collect();

        if !errors.is_empty() {
            println!(
                "{}",
                format!("❌ {} policy violation(s):", errors.len())
                    .red()
                    .bold()
            );
            for v in &errors {
                println!("   {} {}", "•".red(), v.message);
            }
            result.passed = false;
        }

        if !warnings.is_empty() {
            println!("{}", format!("⚠️  {} warning(s):", warnings.len()).yellow());
            for v in &warnings {
                println!("   {} {}", "•".yellow(), v.message);
            }
        }
    }

    println!("{}", "==================================".bright_blue());

    Ok(result)
}

/// Check policies before bundle (returns error if any violations)
pub fn enforce(project: Option<&str>) -> Result<()> {
    let result = check(project)?;

    if !result.passed {
        bail!("Policy check failed. Fix violations before proceeding.");
    }

    Ok(())
}
