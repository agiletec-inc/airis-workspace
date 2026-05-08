//! Build command: Docker builds with caching, multi-target, and parallelism.
//!
//! Extracted from main.rs to separate build concerns:
//! - `build_affected_docker()`: parallel build for --affected --docker
//! - `build_docker()`: single/multi-target Docker build
//! - --prod and --quick delegate to run::run_build_prod / run_build_quick

use std::path::PathBuf;

use anyhow::Result;
use colored::Colorize;

use crate::{dag, docker_build, executor, pnpm, remote_cache};

/// Options shared across Docker build modes.
pub struct DockerBuildOpts {
    pub channel: Option<String>,
    pub targets: Option<Vec<String>>,
    pub parallel: Option<usize>,
    pub image: Option<String>,
    pub push: bool,
    pub context_out: Option<PathBuf>,
    pub no_cache: bool,
    pub remote_cache: Option<String>,
}

/// Parallel Docker build for affected projects (--affected --docker).
pub fn build_affected_docker(base: &str, head: &str, opts: &DockerBuildOpts) -> Result<()> {
    let affected_projects = crate::commands::affected::run(base, head)?;

    if affected_projects.is_empty() {
        println!("{}", "✅ No affected projects to build".green());
        return Ok(());
    }

    let worker_count = opts.parallel.unwrap_or_else(executor::default_parallelism);
    let root = std::env::current_dir()?;
    let remote = opts
        .remote_cache
        .as_ref()
        .map(|url| remote_cache::Remote::parse(url))
        .transpose()?;

    let mut exec = executor::ParallelExecutor::new(worker_count);

    for proj in &affected_projects {
        let target = convert_package_to_path(proj);
        let resolved_channel = resolve_channel_for_project(opts.channel.clone(), &target);

        let deps: Vec<String> = {
            let lock_path = root.join("pnpm-lock.yaml");
            if let Ok(lock) = pnpm::PnpmLock::load(&lock_path) {
                let workspace_map = pnpm::build_workspace_map(&lock);
                let dag = dag::build_dag(&workspace_map);
                dag.nodes
                    .get(&target)
                    .map(|n| {
                        n.deps
                            .iter()
                            .filter(|d| {
                                affected_projects
                                    .iter()
                                    .any(|ap| convert_package_to_path(ap) == **d)
                            })
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                vec![]
            }
        };

        exec.add_task(executor::BuildTask {
            id: target.clone(),
            target: target.clone(),
            channel: resolved_channel,
            dependencies: deps,
        });
    }

    let push = opts.push;
    let no_cache = opts.no_cache;
    let root_clone = root.clone();
    let image_clone = opts.image.clone();
    let context_out_clone = opts.context_out.clone();
    let remote_clone = remote.clone();

    let rt = tokio::runtime::Runtime::new()?;
    let results = rt.block_on(async {
        exec.execute(move |task| {
            let root = root_clone.clone();
            let image = image_clone.clone();
            let context_out = context_out_clone.clone();
            let remote = remote_clone.clone();

            async move {
                let start = std::time::Instant::now();

                let hash = docker_build::compute_content_hash(&root, &task.target)?;

                if let Some(_artifact) = docker_build::cache_hit(&task.target, &hash) {
                    return Ok(executor::TaskResult {
                        task_id: task.id,
                        success: true,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: None,
                    });
                }

                if let Some(ref remote) = remote
                    && let Some(artifact) = remote_cache::remote_hit(&task.target, &hash, remote)?
                {
                    docker_build::cache_store(&task.target, &hash, &artifact)?;
                    return Ok(executor::TaskResult {
                        task_id: task.id,
                        success: true,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error: None,
                    });
                }

                let config = docker_build::BuildConfig {
                    target: task.target.clone(),
                    image_name: image,
                    push,
                    no_cache,
                    context_out,
                    channel: task.channel.clone(),
                    ..Default::default()
                };

                let result = docker_build::docker_build(&root, config)?;

                let artifact = docker_build::CachedArtifact {
                    image_ref: result.image_ref.clone(),
                    hash: hash.clone(),
                    built_at: chrono::Utc::now().to_rfc3339(),
                    target: task.target.clone(),
                };
                docker_build::cache_store(&task.target, &hash, &artifact)?;

                if let Some(ref remote) = remote {
                    remote_cache::remote_store(&task.target, &hash, &artifact, remote)?;
                }

                Ok(executor::TaskResult {
                    task_id: task.id,
                    success: true,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error: None,
                })
            }
        })
        .await
    })?;

    let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();
    if !failed.is_empty() {
        anyhow::bail!("{} build(s) failed", failed.len());
    }

    Ok(())
}

/// Single or multi-target Docker build (--docker).
///
/// Multiple targets (e.g. `--targets lts,current`) are built in parallel using tokio.
pub fn build_docker(project: &str, opts: &DockerBuildOpts) -> Result<()> {
    let build_targets: Vec<String> = if let Some(ref t) = opts.targets {
        t.clone()
    } else if let Some(ref ch) = opts.channel {
        vec![ch.clone()]
    } else {
        vec![resolve_channel_for_project(None, project)]
    };

    let root = std::env::current_dir()?;

    let remote = opts
        .remote_cache
        .as_ref()
        .map(|url| remote_cache::Remote::parse(url))
        .transpose()?;

    if build_targets.len() == 1 {
        return build_single_target(
            project,
            &build_targets[0],
            &root,
            opts,
            &remote,
            false,
        );
    }

    println!("{}", "==================================".bright_blue());
    println!(
        "{}",
        "airis build --docker (multi-target)".bright_blue().bold()
    );
    println!("Project: {}", project.cyan());
    println!("Targets: {}", build_targets.join(", ").yellow());
    println!("{}", "==================================".bright_blue());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let mut join_set: tokio::task::JoinSet<Result<()>> = tokio::task::JoinSet::new();

        for build_channel in build_targets.iter() {
            let project = project.to_string();
            let build_channel = build_channel.clone();
            let root = root.clone();
            let opts_image = opts.image.clone();
            let opts_push = opts.push;
            let opts_no_cache = opts.no_cache;
            let opts_context_out = opts.context_out.clone();
            let remote = remote.clone();

            join_set.spawn_blocking(move || {
                build_single_target(
                    &project,
                    &build_channel,
                    &root,
                    &DockerBuildOpts {
                        channel: Some(build_channel.clone()),
                        targets: None,
                        parallel: None,
                        image: opts_image.as_ref().map(|img| {
                            if img.contains(':') {
                                format!("{}-{}", img, build_channel)
                            } else {
                                format!("{}:{}", img, build_channel)
                            }
                        }),
                        push: opts_push,
                        context_out: opts_context_out,
                        no_cache: opts_no_cache,
                        remote_cache: None,
                    },
                    &remote,
                    true,
                )
            });
        }

        let mut errors: Vec<String> = vec![];
        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => errors.push(e.to_string()),
                Err(join_err) => errors.push(format!("task panicked: {join_err}")),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(errors.join("\n"))
        }
    })?;

    println!(
        "\n{}",
        format!("✅ Built {} target(s) for {}", build_targets.len(), project)
            .green()
            .bold()
    );

    Ok(())
}

fn build_single_target(
    project: &str,
    build_channel: &str,
    root: &std::path::Path,
    opts: &DockerBuildOpts,
    remote: &Option<remote_cache::Remote>,
    show_header: bool,
) -> Result<()> {
    if show_header {
        println!(
            "{}",
            format!("▶ Building target: {}", build_channel).bright_blue()
        );
    }

    let base_hash = docker_build::compute_content_hash(root, project)?;
    let hash = format!("{}-{}", base_hash, build_channel);
    let final_hash = blake3::hash(hash.as_bytes()).to_hex()[..12].to_string();

    if let Some(artifact) = docker_build::cache_hit(project, &final_hash) {
        println!(
            "{}",
            format!("  ✅ Local cache hit: {}", artifact.image_ref).green()
        );
        return Ok(());
    }

    if let Some(remote) = remote
        && let Some(artifact) = remote_cache::remote_hit(project, &final_hash, remote)?
    {
        println!(
            "{}",
            format!("  ✅ Remote cache hit: {}", artifact.image_ref).green()
        );
        docker_build::cache_store(project, &final_hash, &artifact)?;
        return Ok(());
    }

    let config = docker_build::BuildConfig {
        target: project.to_string(),
        image_name: opts.image.clone(),
        push: opts.push,
        no_cache: opts.no_cache,
        context_out: opts.context_out.clone(),
        channel: build_channel.to_string(),
        ..Default::default()
    };
    let result = docker_build::docker_build(root, config)?;

    let artifact = docker_build::CachedArtifact {
        image_ref: result.image_ref.clone(),
        hash: final_hash.clone(),
        built_at: chrono::Utc::now().to_rfc3339(),
        target: project.to_string(),
    };
    docker_build::cache_store(project, &final_hash, &artifact)?;

    if let Some(remote) = remote {
        println!("{}", "  📤 Pushing to remote cache...".cyan());
        remote_cache::remote_store(project, &final_hash, &artifact, remote)?;
    }

    Ok(())
}

/// Resolve channel from CLI arg or manifest.toml.
/// Priority: CLI --channel > manifest.toml [projects.<name>.runner.channel] > "lts"
fn resolve_channel_for_project(cli_channel: Option<String>, project_path: &str) -> String {
    if let Some(ch) = cli_channel {
        return ch;
    }

    if let Ok(content) = std::fs::read_to_string("manifest.toml")
        && let Ok(manifest) = toml::from_str::<toml::Value>(&content)
    {
        let project_name = project_path.rsplit('/').next().unwrap_or(project_path);

        if let Some(projects) = manifest.get("projects")
            && let Some(project) = projects.get(project_name)
            && let Some(runner) = project.get("runner")
        {
            if let Some(channel) = runner.get("channel")
                && let Some(ch) = channel.as_str()
            {
                return ch.to_string();
            }
            if let Some(version) = runner.get("version")
                && let Some(v) = version.as_str()
            {
                return v.to_string();
            }
        }
    }

    "lts".to_string()
}

/// Convert package name to project path.
/// e.g., "@workspace/web" -> "apps/web", "@agiletec/api" -> "apps/api"
fn convert_package_to_path(package_name: &str) -> String {
    let name = package_name
        .trim_start_matches('@')
        .split('/')
        .next_back()
        .unwrap_or(package_name);

    for dir in &["apps", "libs", "packages"] {
        let path = format!("{}/{}", dir, name);
        if std::path::Path::new(&path).exists() {
            return path;
        }
    }

    format!("apps/{}", name)
}
