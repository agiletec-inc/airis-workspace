//! Bundle command: Generate complete deployment packages
//!
//! Creates distribution-ready artifacts from built projects:
//! - bundle.json: Metadata (version, hash, deps, timestamps)
//! - image.tar: Docker image tarball (docker save)
//! - artifact.tar.gz: Standalone build artifacts

mod artifacts;
mod k8s;
mod metadata;

#[cfg(test)]
mod tests;

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

use crate::docker_build::{cache_hit, compute_content_hash};

use artifacts::{export_docker_image, format_size, package_artifacts};
use k8s::generate_k8s_manifests;
use metadata::generate_metadata;

/// Bundle output result
#[derive(Debug)]
#[allow(dead_code)]
pub struct BundleResult {
    pub output_dir: PathBuf,
    pub bundle_json: PathBuf,
    pub image_tar: Option<PathBuf>,
    pub artifact_tar: Option<PathBuf>,
    pub k8s_dir: Option<PathBuf>,
}

/// Run bundle command
pub fn run(project: &str, output_dir: Option<&Path>, k8s: bool) -> Result<BundleResult> {
    use colored::Colorize;

    println!("{}", "==================================".bright_blue());
    println!("{}", "airis bundle".bright_blue().bold());
    println!("Project: {}", project.cyan());
    if k8s {
        println!("K8s:     {}", "enabled".green());
    }
    println!("{}", "==================================".bright_blue());

    let root = std::env::current_dir()?;

    // 1. Validate project exists
    let project_path = root.join(project);
    if !project_path.exists() {
        bail!("Project not found: {}", project);
    }

    // 2. Calculate content hash
    let hash = compute_content_hash(&root, project)?;
    println!("📋 Content hash: {}", hash.yellow());

    // 3. Check for cached build
    let cached = cache_hit(project, &hash);
    let cache_hit_status = cached.is_some();

    if cached.is_none() {
        println!(
            "{}",
            "⚠️  No cached build found. Run 'airis build --docker' first.".yellow()
        );
        bail!(
            "No cached build for {}. Run: airis build --docker {}",
            project,
            project
        );
    }

    let cached = cached.unwrap();
    println!("✅ Found cached build: {}", cached.image_ref.green());

    // 4. Create output directory
    let dist_dir = output_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| root.join("dist"));
    let project_name = project.rsplit('/').next().unwrap_or(project);
    let bundle_dir = dist_dir.join(project_name);
    fs::create_dir_all(&bundle_dir).with_context(|| {
        format!(
            "Failed to create bundle directory: {}",
            bundle_dir.display()
        )
    })?;

    println!(
        "📦 Bundle output: {}",
        bundle_dir.display().to_string().cyan()
    );

    // 5. Generate bundle.json
    let metadata = generate_metadata(project, &hash, &cached.image_ref, cache_hit_status)?;
    let bundle_json_path = bundle_dir.join("bundle.json");
    let json_content = serde_json::to_string_pretty(&metadata)?;
    fs::write(&bundle_json_path, &json_content)?;
    println!("✅ Generated: bundle.json");

    // 6. Export Docker image (docker save)
    let image_tar_path = bundle_dir.join("image.tar");
    let image_tar = export_docker_image(&cached.image_ref, &image_tar_path)?;
    if image_tar.is_some() {
        let size = fs::metadata(&image_tar_path)?.len();
        println!("✅ Generated: image.tar ({})", format_size(size).dimmed());
    }

    // 7. Package build artifacts
    let artifact_tar_path = bundle_dir.join("artifact.tar.gz");
    let artifact_tar = package_artifacts(&root, project, &artifact_tar_path)?;
    if artifact_tar.is_some() {
        let size = fs::metadata(&artifact_tar_path)?.len();
        println!(
            "✅ Generated: artifact.tar.gz ({})",
            format_size(size).dimmed()
        );
    }

    // 8. Generate Kubernetes manifests (if --k8s flag)
    let k8s_dir = if k8s {
        let k8s_path = generate_k8s_manifests(&bundle_dir, project, &cached.image_ref)?;
        println!("✅ Generated: k8s/ (deployment.yaml, service.yaml)");
        Some(k8s_path)
    } else {
        None
    };

    // 9. Print summary
    println!();
    println!("{}", "==================================".bright_blue());
    println!("{}", "✅ Bundle complete!".green().bold());
    println!("   Output: {}", bundle_dir.display());
    println!("   Hash:   {}", hash);
    if k8s {
        println!("   K8s:    {}/k8s/", bundle_dir.display());
    }
    println!("{}", "==================================".bright_blue());

    Ok(BundleResult {
        output_dir: bundle_dir,
        bundle_json: bundle_json_path,
        image_tar,
        artifact_tar,
        k8s_dir,
    })
}
