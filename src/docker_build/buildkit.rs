//! BuildKit runner and main docker_build entry point

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::channel::{RuntimeChannel, resolve_channel};
use crate::pnpm::{PnpmLock, build_workspace_map};

use super::cache::{cache_hit, cache_store};
use super::context::ContextBuilder;
use super::dockerfile::generate_dockerfile_for_toolchain;
use super::hash::compute_hash;
use super::{BuildConfig, BuildResult, CachedArtifact};

/// Run BuildKit build
pub fn run_buildkit(
    ctx_dir: &Path,
    dockerfile_content: &str,
    config: &BuildConfig,
    hash: &str,
) -> Result<BuildResult> {
    use std::time::Instant;

    let start = Instant::now();

    // Write Dockerfile to context
    let dockerfile_path = ctx_dir.join("Dockerfile");
    fs::write(&dockerfile_path, dockerfile_content)?;

    // Determine image name
    let app_name = config.target.rsplit('/').next().unwrap_or(&config.target);
    let image_name = config
        .image_name
        .clone()
        .unwrap_or_else(|| format!("{}:airis-{}", app_name, hash));

    println!("🐳 Building image: {}", image_name);
    println!("   Context: {}", ctx_dir.display());

    // Build command
    let mut cmd = Command::new("docker");
    cmd.arg("buildx")
        .arg("build")
        .arg("--progress=plain")
        .arg("-t")
        .arg(&image_name)
        .arg("-f")
        .arg(&dockerfile_path);

    // Add build args
    for (key, value) in &config.build_args {
        cmd.arg("--build-arg").arg(format!("{}={}", key, value));
    }

    // Target the prod stage when building from unified multi-stage Dockerfile
    cmd.arg("--target").arg("prod");

    if config.no_cache {
        cmd.arg("--no-cache");
    }

    if config.push {
        cmd.arg("--push");
    } else {
        cmd.arg("--load");
    }

    cmd.arg(ctx_dir);

    // Execute
    let status = cmd.status().context("Failed to run docker buildx")?;

    let duration = start.elapsed().as_secs();

    if !status.success() {
        bail!("Docker build failed with exit code: {:?}", status.code());
    }

    Ok(BuildResult {
        image_ref: image_name,
        hash: hash.to_string(),
        duration_secs: duration,
    })
}

/// Main entry point for `airis build --docker`
pub fn docker_build(root: &Path, config: BuildConfig) -> Result<BuildResult> {
    use colored::Colorize;

    // 1. Resolve runtime channel to toolchain
    let channel = RuntimeChannel::parse(&config.channel)?;
    let toolchain = resolve_channel(&channel)?;

    println!("{}", "==================================".bright_blue());
    println!("{}", "airis build --docker".bright_blue().bold());
    println!("Target:  {}", config.target.cyan());
    println!(
        "Channel: {} → {} ({})",
        config.channel.yellow(),
        toolchain.image.green(),
        format!("{:?}", toolchain.family).dimmed()
    );
    println!("{}", "==================================".bright_blue());

    // 2. Load pnpm-lock.yaml
    let lock_path = root.join("pnpm-lock.yaml");
    let lock = PnpmLock::load(&lock_path)?;

    // 3. Build workspace map and DAG
    let workspace_map = build_workspace_map(&lock);
    let dag = crate::dag::build_dag(&workspace_map);

    // 4. Verify target exists
    if !dag.nodes.contains_key(&config.target) {
        bail!(
            "Target '{}' not found in workspace. Available:\n{}",
            config.target,
            dag.nodes
                .keys()
                .map(|k| format!("  - {}", k))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    // 5. Build context
    let ctx_builder = ContextBuilder::new(root, &dag, &lock, &config.target);
    let ctx_dir = ctx_builder.build(config.context_out.as_deref())?;

    // 6. Compute hash (includes toolchain info for cache invalidation)
    let hash = compute_hash(&ctx_dir)?;
    // Append channel to hash for cache invalidation on channel change
    let full_hash = format!("{}-{}", hash, config.channel);
    let final_hash = blake3::hash(full_hash.as_bytes()).to_hex()[..12].to_string();
    println!("📋 Input hash: {}", final_hash.yellow());

    // 7. Check cache (skip if --no-cache)
    if !config.no_cache
        && let Some(cached) = cache_hit(&config.target, &final_hash)
    {
        println!();
        println!("{}", "==================================".bright_blue());
        println!("{}", "⚡ Cache hit! Skipping build.".green().bold());
        println!("   Image: {}", cached.image_ref);
        println!("   Hash:  {}", cached.hash);
        println!("   Built: {}", cached.built_at);
        println!("{}", "==================================".bright_blue());

        return Ok(BuildResult {
            image_ref: cached.image_ref,
            hash: cached.hash,
            duration_secs: 0,
        });
    }

    // 8. Generate Dockerfile based on runtime family
    let dockerfile =
        generate_dockerfile_for_toolchain(&config.target, &toolchain, &config.build_args);

    // 9. Run BuildKit
    let result = run_buildkit(&ctx_dir, &dockerfile, &config, &final_hash)?;

    // 10. Store in cache
    let artifact = CachedArtifact {
        image_ref: result.image_ref.clone(),
        hash: result.hash.clone(),
        built_at: chrono::Utc::now().to_rfc3339(),
        target: config.target.clone(),
    };
    if let Err(e) = cache_store(&config.target, &final_hash, &artifact) {
        eprintln!("⚠️  Warning: Failed to store cache: {}", e);
    }

    // 10. Print summary
    println!();
    println!("{}", "==================================".bright_blue());
    println!("{}", "✅ Build successful!".green());
    println!("   Image: {}", result.image_ref);
    println!("   Hash:  {}", result.hash);
    println!("   Time:  {}s", result.duration_secs);
    println!("{}", "==================================".bright_blue());

    Ok(result)
}
