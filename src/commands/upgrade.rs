//! Upgrade command: self-update airis to the latest version
//!
//! Downloads and installs the latest version from GitHub Releases.

use anyhow::{Context, Result};
use colored::Colorize;
use serde::Deserialize;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// GitHub Release response structure
#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
    html_url: String,
}

/// GitHub Release asset
#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Run upgrade check only
pub fn run_check() -> Result<()> {
    println!("{}", "🔍 Checking for updates...".bright_blue());
    println!();

    let current = env!("CARGO_PKG_VERSION");
    let latest = fetch_latest_version()?;

    println!("Current version: {}", current.cyan());
    println!("Latest version:  {}", latest.cyan());
    println!();

    if version_gt(&latest, current) {
        println!(
            "{}",
            format!("✨ New version available: {} → {}", current, latest)
                .green()
                .bold()
        );
        println!();
        println!("To upgrade, run:");
        println!("  {}", "airis upgrade".cyan());
    } else {
        println!("{}", "✅ Already up to date!".green());
    }

    Ok(())
}

/// Run upgrade to specific version or latest
pub fn run(target_version: Option<String>) -> Result<()> {
    println!("{}", "🚀 Upgrading airis...".bright_blue());
    println!();

    let current = env!("CARGO_PKG_VERSION");

    // Determine target version
    let target = match target_version {
        Some(v) => {
            // Remove 'v' prefix if present
            let version = v.strip_prefix('v').unwrap_or(&v).to_string();
            println!("Target version: {}", version.cyan());
            version
        }
        None => {
            let latest = fetch_latest_version()?;
            if !version_gt(&latest, current) {
                println!("{}", "✅ Already up to date!".green());
                println!("   Current version: {}", current);
                return Ok(());
            }
            println!(
                "Upgrading: {} → {}",
                current.yellow(),
                latest.green().bold()
            );
            latest
        }
    };

    // Check if same version
    if target == current {
        println!("{}", "✅ Already running this version!".green());
        return Ok(());
    }

    // Detect platform
    let (os, arch) = detect_platform()?;
    println!("Platform: {}-{}", os, arch);
    println!();

    // Fetch release info for target version
    let release = fetch_release(&target)?;

    // Find matching asset
    let asset_name = format!("airis-{}-{}", os, arch);
    let asset = release
        .assets
        .iter()
        .find(|a| a.name.starts_with(&asset_name) || a.name == format!("{}.tar.gz", asset_name))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "No binary found for platform {}-{}\n\
                 Available assets: {:?}",
                os,
                arch,
                release.assets.iter().map(|a| &a.name).collect::<Vec<_>>()
            )
        })?;

    println!("Downloading: {}", asset.name.cyan());

    // Download to temp file
    let temp_dir = env::temp_dir();
    let download_path = temp_dir.join(&asset.name);
    download_file(&asset.browser_download_url, &download_path)?;

    // Extract if needed
    let binary_path = if asset.name.ends_with(".tar.gz") {
        println!("Extracting...");
        extract_tar_gz(&download_path, &temp_dir)?;
        temp_dir.join("airis")
    } else {
        download_path.clone()
    };

    // Make executable
    let mut perms = fs::metadata(&binary_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&binary_path, perms)?;

    // Verify binary works
    println!("Verifying...");
    let output = Command::new(&binary_path)
        .arg("-V")
        .output()
        .context("Failed to verify downloaded binary")?;

    if !output.status.success() {
        anyhow::bail!("Downloaded binary verification failed");
    }

    let verified_version = String::from_utf8_lossy(&output.stdout);
    println!("Verified: {}", verified_version.trim().dimmed());

    // Find current binary location
    let current_exe = env::current_exe().context("Failed to get current executable path")?;
    println!();
    println!("Installing to: {}", current_exe.display());

    // Backup current binary
    let backup_path = current_exe.with_extension("backup");
    if current_exe.exists() {
        fs::copy(&current_exe, &backup_path).context("Failed to backup current binary")?;
    }

    // Replace binary
    match fs::copy(&binary_path, &current_exe) {
        Ok(_) => {
            // Clean up backup on success
            let _ = fs::remove_file(&backup_path);
        }
        Err(e) => {
            // Restore backup on failure
            if backup_path.exists() {
                let _ = fs::copy(&backup_path, &current_exe);
                let _ = fs::remove_file(&backup_path);
            }
            return Err(e).context("Failed to install new binary");
        }
    }

    // Clean up temp files
    let _ = fs::remove_file(&download_path);
    if asset.name.ends_with(".tar.gz") {
        let _ = fs::remove_file(temp_dir.join("airis"));
    }

    println!();
    println!(
        "{}",
        format!("✅ Successfully upgraded to v{}!", target)
            .green()
            .bold()
    );
    println!();
    println!("Release notes: {}", release.html_url.cyan());

    Ok(())
}

/// Fetch the latest release version from GitHub
fn fetch_latest_version() -> Result<String> {
    let release = fetch_release("latest")?;
    Ok(release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name)
        .to_string())
}

/// Fetch release information from GitHub
fn fetch_release(version: &str) -> Result<Release> {
    let url = if version == "latest" {
        "https://api.github.com/repos/agiletec-inc/airis-monorepo/releases/latest".to_string()
    } else {
        format!(
            "https://api.github.com/repos/agiletec-inc/airis-monorepo/releases/tags/v{}",
            version
        )
    };

    let output = Command::new("curl")
        .args([
            "-sS",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: airis-upgrade",
            &url,
        ])
        .output()
        .context("Failed to fetch release info from GitHub")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("GitHub API request failed: {}", stderr);
    }

    let body = String::from_utf8(output.stdout).context("Invalid UTF-8 in GitHub response")?;

    // Check for error response
    if body.contains("\"message\":") && body.contains("Not Found") {
        anyhow::bail!("Version {} not found in GitHub releases", version);
    }

    serde_json::from_str(&body).context("Failed to parse GitHub release response")
}

/// Download a file from URL to path
fn download_file(url: &str, path: &PathBuf) -> Result<()> {
    let output = Command::new("curl")
        .args([
            "-sS",
            "-L", // Follow redirects
            "-o",
            &path.to_string_lossy(),
            url,
        ])
        .output()
        .context("Failed to download file")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Download failed: {}", stderr);
    }

    // Verify file was created
    if !path.exists() || fs::metadata(path)?.len() == 0 {
        anyhow::bail!("Downloaded file is empty or missing");
    }

    Ok(())
}

/// Extract a .tar.gz file
fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let output = Command::new("tar")
        .args([
            "-xzf",
            &archive.to_string_lossy(),
            "-C",
            &dest.to_string_lossy(),
        ])
        .output()
        .context("Failed to extract archive")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Extraction failed: {}", stderr);
    }

    Ok(())
}

/// Detect current platform (os, arch)
fn detect_platform() -> Result<(String, String)> {
    let os = match env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        "windows" => "windows",
        other => anyhow::bail!("Unsupported OS: {}", other),
    };

    let arch = match env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => anyhow::bail!("Unsupported architecture: {}", other),
    };

    Ok((os.to_string(), arch.to_string()))
}

/// Compare versions (returns true if v1 > v2)
fn version_gt(v1: &str, v2: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };

    let v1_parts = parse(v1);
    let v2_parts = parse(v2);

    for (a, b) in v1_parts.iter().zip(v2_parts.iter()) {
        if a > b {
            return true;
        }
        if a < b {
            return false;
        }
    }

    v1_parts.len() > v2_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_gt() {
        assert!(version_gt("1.66.0", "1.65.0"));
        assert!(version_gt("2.0.0", "1.99.99"));
        assert!(version_gt("1.0.1", "1.0.0"));
        assert!(!version_gt("1.65.0", "1.66.0"));
        assert!(!version_gt("1.65.0", "1.65.0"));
    }

    #[test]
    fn test_detect_platform() {
        // Just verify it doesn't panic
        let result = detect_platform();
        assert!(result.is_ok());
        let (os, arch) = result.unwrap();
        assert!(!os.is_empty());
        assert!(!arch.is_empty());
    }
}
