use anyhow::{Context, Result, bail};
use colored::Colorize;
use std::path::Path;
use std::process::Command;

use crate::manifest::Manifest;

use super::build_compose_command;

fn validate_app_name(app: &str) -> Result<()> {
    if !app
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        bail!(
            "Invalid app name: \"{}\". Only [a-zA-Z0-9._-] allowed.",
            app
        );
    }
    Ok(())
}

/// Build production Docker image for an app
pub fn run_build_prod(app: &str) -> Result<()> {
    use std::time::Instant;

    validate_app_name(app)?;

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

    let build_cmd = format!(
        "docker build -f {} -t {}:latest -t {}:{} --progress=plain .",
        dockerfile, app, app, timestamp
    );
    println!("🚀 Running: {}", build_cmd.cyan());

    let status = Command::new("docker")
        .arg("build")
        .arg("-f")
        .arg(&dockerfile)
        .arg("-t")
        .arg(format!("{}:latest", app))
        .arg("-t")
        .arg(format!("{}:{}", app, timestamp))
        .arg("--progress=plain")
        .arg(".")
        .env("DOCKER_BUILDKIT", "1")
        .status()
        .with_context(|| "Failed to execute docker build")?;

    let duration = start.elapsed().as_secs();

    if !status.success() {
        bail!("Build failed with exit code: {:?}", status.code());
    }

    let size_output = Command::new("docker")
        .args([
            "images",
            &format!("{}:latest", app),
            "--format",
            "{{.Size}}",
        ])
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
    validate_app_name(app)?;
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Create one (see docs/manifest.md) or ask Claude Code via {}.",
            "/airis:init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path).with_context(|| "Failed to load manifest.toml")?;

    let app_dir = format!("apps/{}", app);

    if !Path::new(&app_dir).exists() {
        bail!("❌ App directory {} not found", app_dir);
    }

    println!("🔨 Testing production build for {}", app.cyan());
    println!();

    let next_config = format!("{}/next.config.mjs", app_dir);
    if Path::new(&next_config).exists() {
        let config_content = std::fs::read_to_string(&next_config)?;
        if config_content.contains("output") && config_content.contains("standalone") {
            println!("{}", "✅ Standalone output configured".green());
        } else {
            println!(
                "{}",
                "⚠️  Warning: Standalone output not found in next.config.mjs".yellow()
            );
        }
    }

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

    let check_cmd = format!(
        "exec workspace sh -c 'ls -lh {0}/.next/standalone/ 2>/dev/null || echo \"Standalone output not found\"'",
        app_dir
    );
    let check_full_cmd = build_compose_command(&manifest, &check_cmd)?;

    let _ = Command::new("sh").arg("-c").arg(&check_full_cmd).status();

    Ok(())
}

/// Run tests with coverage check
pub fn run_test_coverage(min_coverage: u8) -> Result<()> {
    let manifest_path = Path::new("manifest.toml");

    if !manifest_path.exists() {
        bail!(
            "❌ manifest.toml not found. Create one (see docs/manifest.md) or ask Claude Code via {}.",
            "/airis:init".bold()
        );
    }

    let manifest = Manifest::load(manifest_path).with_context(|| "Failed to load manifest.toml")?;

    println!("🧪 Running tests with coverage check");
    println!("📊 Minimum coverage threshold: {}%", min_coverage);
    println!();

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

    if !stdout.is_empty() {
        println!("{}", stdout);
    }
    if !stderr.is_empty() {
        eprintln!("{}", stderr);
    }

    if !output.status.success() {
        bail!("Tests failed with exit code: {:?}", output.status.code());
    }

    let coverage_regex = regex::Regex::new(r"All files\s*\|\s*(\d+\.?\d*)")?;

    if let Some(captures) = coverage_regex.captures(&stdout) {
        if let Some(coverage_match) = captures.get(1) {
            let coverage: f64 = coverage_match.as_str().parse().unwrap_or(0.0);

            println!();
            if coverage >= min_coverage as f64 {
                println!(
                    "{}",
                    format!(
                        "✅ Coverage {:.1}% meets threshold {}%",
                        coverage, min_coverage
                    )
                    .green()
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

/// Validate a clean path/pattern is safe (no path traversal, no absolute paths)
#[cfg(test)]
pub(super) fn validate_clean_path(path: &str) -> Option<String> {
    let trimmed = path.trim();

    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with('/') || trimmed.starts_with('~') {
        return None;
    }

    if trimmed.contains("..") {
        return None;
    }

    let is_safe = trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '/');

    if !is_safe {
        return None;
    }

    if trimmed == "." || trimmed == "./" {
        return None;
    }

    Some(trimmed.to_string())
}

/// Validate a recursive pattern (for find -name)
#[cfg(test)]
pub(super) fn validate_clean_pattern(pattern: &str) -> Option<String> {
    let trimmed = pattern.trim();

    if trimmed.is_empty() {
        return None;
    }

    if trimmed.contains('/') || trimmed.contains("..") {
        return None;
    }

    let is_safe = trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '*' || c == '?');

    if !is_safe {
        return None;
    }

    let escaped = trimmed.replace('\'', "'\\''");

    Some(escaped)
}

/// Build clean command from manifest.toml [workspace.clean] section
#[cfg(test)]
pub(super) fn build_clean_command(manifest: &Manifest) -> String {
    let clean = &manifest.workspace.clean;
    let mut parts = Vec::new();

    parts.push("echo '🧹 Cleaning host build artifacts...'".to_string());

    for pattern in &clean.recursive {
        if let Some(safe_pattern) = validate_clean_pattern(pattern) {
            parts.push(format!(
                "find . -maxdepth 3 -type d -name '{}' -not -path './supabase/*' -not -path './infra/*' -not -path './.git/*' -exec rm -rf {{}} + 2>/dev/null || true",
                safe_pattern
            ));
        } else {
            parts.push(format!(
                "echo '⚠️  Skipped unsafe recursive pattern: {}'",
                pattern.replace('\'', "")
            ));
        }
    }

    for dir in &clean.dirs {
        if let Some(safe_dir) = validate_clean_path(dir) {
            parts.push(format!("rm -rf './{}'", safe_dir.replace('\'', "'\\''")));
        } else {
            parts.push(format!(
                "echo '⚠️  Skipped unsafe clean path: {}'",
                dir.replace('\'', "")
            ));
        }
    }

    parts.push("find . -name '.DS_Store' -delete 2>/dev/null || true".to_string());
    parts.push("echo '✅ Cleaned host build artifacts (container cache preserved)'".to_string());

    parts.join("; ")
}
