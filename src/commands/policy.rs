//! Policy Gates: Pre-deployment validation checks
//!
//! Validates workspace state before bundle/deploy:
//! - Git clean state
//! - Required environment variables
//! - Forbidden files detection
//! - Secret scanning

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
            println!("{}", format!("❌ {} policy violation(s):", errors.len()).red().bold());
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

/// Check for clean git working directory
fn check_git_clean(result: &mut PolicyResult) -> Result<()> {
    use colored::Colorize;

    print!("🔍 Checking git status... ");

    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;

    if !output.status.success() {
        println!("{}", "skipped (not a git repo)".dimmed());
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let dirty_files: Vec<&str> = stdout.lines().collect();

    if dirty_files.is_empty() {
        println!("{}", "clean".green());
    } else {
        println!("{}", format!("{} uncommitted changes", dirty_files.len()).red());
        result.violations.push(PolicyViolation {
            rule: "require_clean_git".to_string(),
            message: format!(
                "Git working directory has {} uncommitted change(s)",
                dirty_files.len()
            ),
            severity: Severity::Error,
        });
    }

    Ok(())
}

/// Check required environment variables
fn check_required_env(required: &[String], result: &mut PolicyResult) {
    use colored::Colorize;

    print!("🔍 Checking environment variables... ");

    let mut missing = Vec::new();
    for var in required {
        if std::env::var(var).is_err() {
            missing.push(var.clone());
        }
    }

    if missing.is_empty() {
        println!("{}", "ok".green());
    } else {
        println!("{}", format!("{} missing", missing.len()).red());
        for var in &missing {
            result.violations.push(PolicyViolation {
                rule: "require_env".to_string(),
                message: format!("Missing required environment variable: {}", var),
                severity: Severity::Error,
            });
        }
    }
}

/// Check for forbidden files
fn check_forbidden_files(forbidden: &[String], result: &mut PolicyResult) -> Result<()> {
    use colored::Colorize;

    print!("🔍 Checking forbidden files... ");

    let mut found = Vec::new();
    for file in forbidden {
        let path = Path::new(file);
        if path.exists() {
            found.push(file.clone());
        }
    }

    if found.is_empty() {
        println!("{}", "none found".green());
    } else {
        println!("{}", format!("{} found", found.len()).red());
        for file in &found {
            result.violations.push(PolicyViolation {
                rule: "forbid_files".to_string(),
                message: format!("Forbidden file exists: {}", file),
                severity: Severity::Error,
            });
        }
    }

    Ok(())
}

/// Check for forbidden patterns
fn check_forbidden_patterns(patterns: &[String], result: &mut PolicyResult) -> Result<()> {
    use colored::Colorize;

    print!("🔍 Checking forbidden patterns... ");

    let mut found = Vec::new();

    for pattern in patterns {
        // Use glob to find matches
        if let Ok(paths) = glob::glob(pattern) {
            for path in paths.flatten() {
                found.push(path.display().to_string());
            }
        }
    }

    if found.is_empty() {
        println!("{}", "none found".green());
    } else {
        println!("{}", format!("{} matches", found.len()).red());
        for file in &found {
            result.violations.push(PolicyViolation {
                rule: "forbid_patterns".to_string(),
                message: format!("Forbidden pattern match: {}", file),
                severity: Severity::Error,
            });
        }
    }

    Ok(())
}

/// Scan for potential secrets in source files
fn scan_secrets(project: Option<&str>, max_size_mb: u64, result: &mut PolicyResult) -> Result<()> {
    use colored::Colorize;

    print!("🔍 Scanning for secrets... ");

    let scan_dir = project.map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

    if !scan_dir.exists() {
        println!("{}", "skipped (directory not found)".dimmed());
        return Ok(());
    }

    let max_size = max_size_mb * 1024 * 1024;
    let mut secrets_found = Vec::new();

    // Secret patterns to detect
    let secret_patterns: &[(&str, &str)] = &[
        (r#"(?i)api[_-]?key\s*[:=]\s*["'][a-zA-Z0-9]{20,}["']"#, "API key"),
        (r#"(?i)secret[_-]?key\s*[:=]\s*["'][a-zA-Z0-9]{20,}["']"#, "Secret key"),
        (r#"(?i)password\s*[:=]\s*["'][^"']{8,}["']"#, "Password"),
        (r#"(?i)aws[_-]?access[_-]?key[_-]?id\s*[:=]\s*["']?[A-Z0-9]{16,}["']?"#, "AWS Access Key"),
        (r#"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[:=]\s*["']?[a-zA-Z0-9/+=]{40}["']?"#, "AWS Secret Key"),
        (r"ghp_[a-zA-Z0-9]{36}", "GitHub Personal Access Token"),
        (r"gho_[a-zA-Z0-9]{36}", "GitHub OAuth Token"),
        (r"sk-[a-zA-Z0-9]{48}", "OpenAI API Key"),
        (r"xox[baprs]-[a-zA-Z0-9-]+", "Slack Token"),
    ];

    let compiled_patterns: Vec<_> = secret_patterns
        .iter()
        .filter_map(|(p, name)| regex::Regex::new(p).ok().map(|r| (r, *name)))
        .collect();

    // Walk directory
    for entry in walkdir::WalkDir::new(&scan_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Skip common non-source files
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(
            ext,
            "js" | "ts" | "jsx" | "tsx" | "py" | "rs" | "go" | "java" | "rb" | "php" | "env" | "json" | "yaml" | "yml" | "toml"
        ) {
            continue;
        }

        // Skip node_modules, .git, etc.
        let path_str = path.to_string_lossy();
        if path_str.contains("node_modules")
            || path_str.contains(".git")
            || path_str.contains("target/")
            || path_str.contains("dist/")
        {
            continue;
        }

        // Check file size
        if let Ok(meta) = path.metadata() {
            if meta.len() > max_size {
                continue;
            }
        }

        // Read and scan
        if let Ok(content) = fs::read_to_string(path) {
            for (regex, name) in &compiled_patterns {
                if regex.is_match(&content) {
                    secrets_found.push((path.display().to_string(), name.to_string()));
                    break; // Only report once per file
                }
            }
        }
    }

    if secrets_found.is_empty() {
        println!("{}", "none found".green());
    } else {
        println!("{}", format!("{} potential secret(s)", secrets_found.len()).yellow());
        for (file, secret_type) in &secrets_found {
            result.violations.push(PolicyViolation {
                rule: "scan_secrets".to_string(),
                message: format!("Potential {} in: {}", secret_type, file),
                severity: Severity::Warning,
            });
        }
    }

    Ok(())
}

/// Check policies before bundle (returns error if any violations)
pub fn enforce(project: Option<&str>) -> Result<()> {
    let result = check(project)?;

    if !result.passed {
        bail!("Policy check failed. Fix violations before proceeding.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_policy_config_default() {
        let config = PolicyConfig::default();
        assert!(!config.gates.require_clean_git);
        assert!(config.gates.require_env.is_empty());
        assert!(config.gates.forbid_files.is_empty());
    }

    #[test]
    fn test_policy_template() {
        let template = PolicyConfig::template();
        assert!(template.contains("[gates]"));
        assert!(template.contains("require_clean_git"));
        assert!(template.contains("[security]"));
        assert!(template.contains("scan_secrets"));
    }

    #[test]
    fn test_forbidden_files_check() {
        let temp = tempdir().unwrap();
        let forbidden_file = temp.path().join(".env.local");
        std::fs::write(&forbidden_file, "SECRET=123").unwrap();

        let mut result = PolicyResult::default();
        result.passed = true;

        // Use absolute path to avoid thread-safety issues with set_current_dir
        let abs_path = forbidden_file.to_string_lossy().to_string();
        check_forbidden_files(&[abs_path.clone()], &mut result).unwrap();

        assert!(!result.violations.is_empty());
        assert!(result.violations[0].message.contains(".env.local"));
    }

    #[test]
    fn test_required_env_missing() {
        let mut result = PolicyResult::default();
        result.passed = true;

        check_required_env(&["DEFINITELY_NOT_SET_12345".to_string()], &mut result);

        assert!(!result.violations.is_empty());
        assert!(result.violations[0].message.contains("DEFINITELY_NOT_SET_12345"));
    }

    #[test]
    fn test_severity_enum() {
        assert_eq!(Severity::Error, Severity::Error);
        assert_ne!(Severity::Error, Severity::Warning);
    }
}
