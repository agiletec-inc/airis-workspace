//! All policy check functions

use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{PolicyResult, PolicyViolation, Severity};

/// Check for clean git working directory
pub(super) fn check_git_clean(result: &mut PolicyResult) -> Result<()> {
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
        println!(
            "{}",
            format!("{} uncommitted changes", dirty_files.len()).red()
        );
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
pub(super) fn check_required_env(required: &[String], result: &mut PolicyResult) {
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
pub(super) fn check_forbidden_files(forbidden: &[String], result: &mut PolicyResult) -> Result<()> {
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
pub(super) fn check_forbidden_patterns(
    patterns: &[String],
    result: &mut PolicyResult,
) -> Result<()> {
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

/// Check that DB-touching test files import generated types.
///
/// Walks integration/e2e test files and verifies that at least one of the
/// `required_imports` patterns appears in each file that references DB operations.
pub(crate) fn check_type_enforcement(
    generated_types_path: &str,
    required_imports: &[String],
    project: Option<&str>,
    result: &mut PolicyResult,
) -> Result<()> {
    print!("🔍 Checking type enforcement in tests... ");

    if required_imports.is_empty() {
        println!("{}", "skipped (no required_imports configured)".dimmed());
        return Ok(());
    }

    let scan_dir = project
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if !scan_dir.exists() {
        println!("{}", "skipped (directory not found)".dimmed());
        return Ok(());
    }

    let compiled: Vec<regex::Regex> = required_imports
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    if compiled.is_empty() {
        println!("{}", "skipped (no valid patterns)".dimmed());
        return Ok(());
    }

    // DB-related keywords that indicate a test touches the database
    let db_indicators = [
        "supabase",
        "createClient",
        "prisma",
        "drizzle",
        "pg.",
        "postgres",
        "from(", // supabase .from('table')
        "query(",
        "insert(",
        "select(",
        "database",
    ];

    let mut violations_found: Vec<String> = Vec::new();

    for entry in walkdir::WalkDir::new(&scan_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Only scan integration/e2e test files
        let is_integration_test = file_name.contains(".integration.")
            || file_name.contains(".e2e.")
            || file_name.contains(".spec.");
        if !is_integration_test {
            continue;
        }

        let path_str = path.to_string_lossy();
        if path_str.contains("node_modules")
            || path_str.contains(".git")
            || path_str.contains("target/")
            || path_str.contains("dist/")
        {
            continue;
        }

        if let Ok(content) = fs::read_to_string(path) {
            // Check if this file touches the database
            let touches_db = db_indicators
                .iter()
                .any(|indicator| content.contains(indicator));

            if !touches_db {
                continue;
            }

            // Check if any required import pattern is present
            let has_required_import = compiled.iter().any(|re| re.is_match(&content));

            if !has_required_import {
                violations_found.push(path.display().to_string());
            }
        }
    }

    if violations_found.is_empty() {
        println!("{}", "ok".green());
    } else {
        println!(
            "{}",
            format!("{} file(s) missing generated types", violations_found.len()).red()
        );
        for file in &violations_found {
            result.violations.push(PolicyViolation {
                rule: "testing.type_enforcement".to_string(),
                message: format!(
                    "{} touches DB but does not import generated types from `{}`",
                    file, generated_types_path
                ),
                severity: Severity::Error,
            });
        }
    }

    Ok(())
}

/// Scan test files for forbidden mock patterns (from manifest.toml [testing])
///
/// Walks integration/e2e test files and checks for patterns like
/// `vi.mock.*supabase` that indicate mocking of real services.
pub(crate) fn check_mock_patterns(
    patterns: &[String],
    project: Option<&str>,
    result: &mut PolicyResult,
) -> Result<()> {
    print!("🔍 Checking for forbidden mock patterns... ");

    let scan_dir = project
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if !scan_dir.exists() {
        println!("{}", "skipped (directory not found)".dimmed());
        return Ok(());
    }

    let compiled: Vec<(regex::Regex, &str)> = patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok().map(|r| (r, p.as_str())))
        .collect();

    if compiled.is_empty() {
        println!("{}", "skipped (no valid patterns)".dimmed());
        return Ok(());
    }

    let mut violations_found: Vec<(String, usize, String)> = Vec::new();

    for entry in walkdir::WalkDir::new(&scan_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Only scan test files
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let is_test_file = file_name.contains(".test.")
            || file_name.contains(".spec.")
            || file_name.contains(".integration.")
            || file_name.contains(".e2e.");
        if !is_test_file {
            continue;
        }

        // Skip node_modules, .git, target, dist
        let path_str = path.to_string_lossy();
        if path_str.contains("node_modules")
            || path_str.contains(".git")
            || path_str.contains("target/")
            || path_str.contains("dist/")
        {
            continue;
        }

        if let Ok(content) = fs::read_to_string(path) {
            for (line_num, line) in content.lines().enumerate() {
                for (regex, pattern) in &compiled {
                    if regex.is_match(line) {
                        violations_found.push((
                            path.display().to_string(),
                            line_num + 1,
                            pattern.to_string(),
                        ));
                    }
                }
            }
        }
    }

    if violations_found.is_empty() {
        println!("{}", "none found".green());
    } else {
        println!(
            "{}",
            format!("{} violation(s)", violations_found.len()).red()
        );
        for (file, line, pattern) in &violations_found {
            result.violations.push(PolicyViolation {
                rule: "testing.forbidden_patterns".to_string(),
                message: format!("Forbidden mock pattern `{}` at {}:{}", pattern, file, line),
                severity: Severity::Error,
            });
        }
    }

    Ok(())
}

/// Scan source files for banned environment variable names.
/// Files matching allowed_paths globs are skipped.
pub(crate) fn check_banned_env_vars(
    banned: &[String],
    allowed_paths: &[String],
    project: Option<&str>,
    result: &mut PolicyResult,
) -> Result<()> {
    print!("🔍 Checking banned env vars... ");

    if banned.is_empty() {
        println!("{}", "skipped (none configured)".dimmed());
        return Ok(());
    }

    let scan_dir = project
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if !scan_dir.exists() {
        println!("{}", "skipped (directory not found)".dimmed());
        return Ok(());
    }

    let allowed_globs: Vec<glob::Pattern> = allowed_paths
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    let mut violations_found: Vec<(String, usize, String)> = Vec::new();

    for entry in walkdir::WalkDir::new(&scan_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();

        // Only scan source files
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(
            ext,
            "js" | "ts" | "jsx" | "tsx" | "py" | "rs" | "go" | "java" | "rb" | "php"
        ) {
            continue;
        }

        let path_str = path.to_string_lossy();
        if path_str.contains("node_modules")
            || path_str.contains(".git")
            || path_str.contains("target/")
            || path_str.contains("dist/")
        {
            continue;
        }

        // Skip allowed paths
        let relative = path
            .strip_prefix(&scan_dir)
            .unwrap_or(path)
            .to_string_lossy();
        if allowed_globs
            .iter()
            .any(|g| g.matches(&relative) || relative.starts_with(g.as_str().trim_end_matches('*')))
        {
            continue;
        }

        if let Ok(content) = fs::read_to_string(path) {
            for (line_num, line) in content.lines().enumerate() {
                for var in banned {
                    if line.contains(var) {
                        violations_found.push((
                            path.display().to_string(),
                            line_num + 1,
                            var.clone(),
                        ));
                    }
                }
            }
        }
    }

    if violations_found.is_empty() {
        println!("{}", "none found".green());
    } else {
        println!(
            "{}",
            format!("{} violation(s)", violations_found.len()).red()
        );
        for (file, line, var) in &violations_found {
            result.violations.push(PolicyViolation {
                rule: "policy.security.banned_env_vars".to_string(),
                message: format!("Banned env var `{}` at {}:{}", var, file, line),
                severity: Severity::Error,
            });
        }
    }

    Ok(())
}

/// Scan for potential secrets in source files
pub(super) fn scan_secrets(
    project: Option<&str>,
    max_size_mb: u64,
    result: &mut PolicyResult,
) -> Result<()> {
    print!("🔍 Scanning for secrets... ");

    let scan_dir = project
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));

    if !scan_dir.exists() {
        println!("{}", "skipped (directory not found)".dimmed());
        return Ok(());
    }

    let max_size = max_size_mb * 1024 * 1024;
    let mut secrets_found = Vec::new();

    // Secret patterns to detect
    let secret_patterns: &[(&str, &str)] = &[
        (
            r#"(?i)api[_-]?key\s*[:=]\s*["'][a-zA-Z0-9]{20,}["']"#,
            "API key",
        ),
        (
            r#"(?i)secret[_-]?key\s*[:=]\s*["'][a-zA-Z0-9]{20,}["']"#,
            "Secret key",
        ),
        (r#"(?i)password\s*[:=]\s*["'][^"']{8,}["']"#, "Password"),
        (
            r#"(?i)aws[_-]?access[_-]?key[_-]?id\s*[:=]\s*["']?[A-Z0-9]{16,}["']?"#,
            "AWS Access Key",
        ),
        (
            r#"(?i)aws[_-]?secret[_-]?access[_-]?key\s*[:=]\s*["']?[a-zA-Z0-9/+=]{40}["']?"#,
            "AWS Secret Key",
        ),
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
            "js" | "ts"
                | "jsx"
                | "tsx"
                | "py"
                | "rs"
                | "go"
                | "java"
                | "rb"
                | "php"
                | "env"
                | "json"
                | "yaml"
                | "yml"
                | "toml"
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
        if let Ok(meta) = path.metadata()
            && meta.len() > max_size
        {
            continue;
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
        println!(
            "{}",
            format!("{} potential secret(s)", secrets_found.len()).yellow()
        );
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
