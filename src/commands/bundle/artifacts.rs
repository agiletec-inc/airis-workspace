//! Docker image export, tar.gz creation, artifact detection

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Export Docker image to tarball
pub(super) fn export_docker_image(image_ref: &str, output_path: &Path) -> Result<Option<PathBuf>> {
    use colored::Colorize;

    println!("📤 Exporting Docker image...");

    let output_path_str = output_path
        .to_str()
        .context("Docker image output path contains non-UTF-8 characters")?;

    let output = Command::new("docker")
        .args(["save", "-o", output_path_str, image_ref])
        .output()
        .context("Failed to run docker save")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", format!("⚠️  docker save failed: {}", stderr).yellow());
        return Ok(None);
    }

    Ok(Some(output_path.to_path_buf()))
}

/// Package build artifacts to tar.gz
pub(super) fn package_artifacts(
    root: &Path,
    project: &str,
    output_path: &Path,
) -> Result<Option<PathBuf>> {
    use colored::Colorize;

    let project_path = root.join(project);

    // Detect artifact directories based on project type
    let artifact_dirs = detect_artifact_dirs(&project_path);

    if artifact_dirs.is_empty() {
        println!("{}", "⚠️  No build artifacts found to package".yellow());
        return Ok(None);
    }

    println!("📦 Packaging artifacts: {:?}", artifact_dirs);

    // Create tar.gz using tar command
    let output_path_str = output_path
        .to_str()
        .context("Artifact output path contains non-UTF-8 characters")?;
    let root_str = root
        .to_str()
        .context("Root path contains non-UTF-8 characters")?;

    let mut args = vec!["-czf".to_string(), output_path_str.to_string()];

    for dir in &artifact_dirs {
        let rel_path = dir.strip_prefix(root).unwrap_or(dir);
        let rel_path_str = rel_path
            .to_str()
            .context("Relative artifact path contains non-UTF-8 characters")?;
        args.push("-C".to_string());
        args.push(root_str.to_string());
        args.push(rel_path_str.to_string());
    }

    let output = Command::new("tar")
        .args(&args)
        .output()
        .context("Failed to create tar.gz")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("{}", format!("⚠️  tar failed: {}", stderr).yellow());
        return Ok(None);
    }

    Ok(Some(output_path.to_path_buf()))
}

/// Detect artifact directories based on project type
pub(super) fn detect_artifact_dirs(project_path: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // Next.js standalone
    let nextjs_standalone = project_path.join(".next").join("standalone");
    if nextjs_standalone.exists() {
        dirs.push(nextjs_standalone);
    }

    // Next.js static
    let nextjs_static = project_path.join(".next").join("static");
    if nextjs_static.exists() {
        dirs.push(nextjs_static);
    }

    // Generic dist
    let dist = project_path.join("dist");
    if dist.exists() {
        dirs.push(dist);
    }

    // Rust target/release
    let rust_release = project_path.join("target").join("release");
    if rust_release.exists() {
        dirs.push(rust_release);
    }

    // Python dist (wheel)
    let python_dist = project_path.join("dist");
    if python_dist.exists() && !dirs.contains(&python_dist) {
        dirs.push(python_dist);
    }

    // Public assets
    let public = project_path.join("public");
    if public.exists() {
        dirs.push(public);
    }

    dirs
}

/// Format byte size to human-readable string
pub(super) fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
