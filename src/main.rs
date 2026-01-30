mod channel;
mod commands;
mod dag;
mod docker_build;
mod executor;
mod generators;
mod manifest;
mod ownership;
mod pnpm;
mod remote_cache;
mod safe_fs;
mod templates;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};

/// Get version string with dev suffix for non-release builds
fn get_version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let is_release = env!("IS_RELEASE");
    let git_hash = env!("GIT_HASH");

    if is_release == "true" {
        version.to_string()
    } else {
        format!("{}-dev (git: {})", version, git_hash)
    }
}

/// Resolve channel from CLI arg or manifest.toml
/// Priority: CLI --channel > manifest.toml [projects.<name>.runner.channel] > "lts"
fn resolve_channel_for_project(cli_channel: Option<String>, project_path: &str) -> String {
    // CLI takes precedence
    if let Some(ch) = cli_channel {
        return ch;
    }

    // Try to read from manifest.toml
    if let Ok(content) = std::fs::read_to_string("manifest.toml") {
        if let Ok(manifest) = toml::from_str::<toml::Value>(&content) {
            // Extract project name from path (e.g., "apps/web" -> "web")
            let project_name = project_path.rsplit('/').next().unwrap_or(project_path);

            // Look for [projects.<name>.runner.channel]
            if let Some(projects) = manifest.get("projects") {
                if let Some(project) = projects.get(project_name) {
                    if let Some(runner) = project.get("runner") {
                        // Check channel first
                        if let Some(channel) = runner.get("channel") {
                            if let Some(ch) = channel.as_str() {
                                return ch.to_string();
                            }
                        }
                        // Check version (mode=exact)
                        if let Some(version) = runner.get("version") {
                            if let Some(v) = version.as_str() {
                                return v.to_string();
                            }
                        }
                    }
                }
            }
        }
    }

    // Default to lts
    "lts".to_string()
}

/// Convert package name to project path
/// e.g., "@workspace/web" -> "apps/web", "@agiletec/api" -> "apps/api"
fn convert_package_to_path(package_name: &str) -> String {
    // Remove @ prefix and scope
    let name = package_name
        .trim_start_matches('@')
        .split('/')
        .last()
        .unwrap_or(package_name);

    // Try to find the actual path by checking directories
    for dir in &["apps", "libs", "packages"] {
        let path = format!("{}/{}", dir, name);
        if std::path::Path::new(&path).exists() {
            return path;
        }
    }

    // Default to apps/ if not found
    format!("apps/{}", name)
}

#[derive(Parser)]
#[command(name = "airis")]
#[command(about = "Docker-first monorepo workspace manager", long_about = None)]
struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version")]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize MANIFEST.toml + workspace metadata
    Init {
        /// Force snapshot capture (default: auto on first run)
        #[arg(long)]
        snapshot: bool,
        /// Skip snapshot capture (for CI or repeated runs)
        #[arg(long)]
        no_snapshot: bool,
        /// Setup .npmrc symlinks for Docker-First enforcement
        #[arg(long)]
        setup_npmrc: bool,
        /// Actually write generated files (default: dry-run, shows what would be generated)
        #[arg(long)]
        write: bool,
        /// Skip project discovery and use empty template instead
        #[arg(long)]
        skip_discovery: bool,
    },

    /// Query MANIFEST.toml data (used by justfile)
    Manifest {
        #[command(subcommand)]
        action: ManifestCommands,
    },

    /// Command guard management
    Guards {
        #[command(subcommand)]
        action: GuardsCommands,
    },

    /// Git hooks management
    Hooks {
        #[command(subcommand)]
        action: HooksCommands,
    },

    /// Docker-First shim management (intercept commands → Docker)
    Shim {
        #[command(subcommand)]
        action: ShimCommands,
    },

    /// Documentation management (CLAUDE.md, .cursorrules, etc.)
    Docs {
        #[command(subcommand)]
        action: DocsCommands,
    },

    /// Validate workspace configuration
    Validate {
        #[command(subcommand)]
        action: ValidateCommands,
        /// Output results as JSON (for LLM integration)
        #[arg(long, global = true)]
        json: bool,
    },

    /// Run system health checks
    Verify,

    /// Diagnose and heal workspace configuration issues
    Doctor {
        /// Automatically fix detected issues
        #[arg(long)]
        fix: bool,
        /// Show startup truth (workspace root, compose files, commands)
        #[arg(long)]
        truth: bool,
        /// Output startup truth as JSON (for LLM/automation)
        #[arg(long)]
        truth_json: bool,
    },

    /// Sync dependencies: resolve catalog policies to actual versions
    #[command(name = "sync-deps")]
    SyncDeps {
        /// Migrate packages to use pnpm catalog references
        #[arg(long)]
        migrate: bool,
    },

    /// Run a command defined in manifest.toml [commands]
    Run {
        /// Task name from [commands] section
        task: String,
    },

    /// Start Docker services (alias for 'run up')
    Up,

    /// Stop Docker services (alias for 'run down')
    Down,

    /// Enter workspace shell (alias for 'run shell')
    Shell,

    /// Run development servers (alias for 'run dev')
    Dev,

    /// Run tests (alias for 'run test')
    Test {
        /// Check coverage threshold
        #[arg(long)]
        coverage_check: bool,
        /// Minimum coverage percentage (default: 80)
        #[arg(long, default_value = "80")]
        min_coverage: u8,
    },

    /// Install dependencies (alias for 'run install')
    Install,

    /// Build all apps (alias for 'run build')
    Build {
        /// Target project path (e.g., apps/web)
        project: Option<String>,
        /// Build only affected projects (based on git diff)
        #[arg(long)]
        affected: bool,
        /// Base branch/commit for --affected (default: origin/main)
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// Head branch/commit for --affected (default: HEAD)
        #[arg(long, default_value = "HEAD")]
        head: String,
        /// Build using Docker (hermetic build with auto-generated Dockerfile)
        #[arg(long)]
        docker: bool,
        /// Runtime channel: lts, current, edge, bun, deno, or version (e.g., 22.12.0)
        /// If not specified, reads from manifest.toml [projects.<name>.runner.channel]
        #[arg(long)]
        channel: Option<String>,
        /// Build for multiple targets (comma-separated: node,edge,bun,deno)
        #[arg(long, value_delimiter = ',')]
        targets: Option<Vec<String>>,
        /// Number of parallel build workers (default: CPU count)
        #[arg(long, short = 'j')]
        parallel: Option<usize>,
        /// Image name for Docker build (e.g., ghcr.io/org/app:tag)
        #[arg(long)]
        image: Option<String>,
        /// Push image to registry after build
        #[arg(long)]
        push: bool,
        /// Output directory for build context (for debugging)
        #[arg(long)]
        context_out: Option<std::path::PathBuf>,
        /// No cache for Docker build
        #[arg(long)]
        no_cache: bool,
        /// Remote cache URL (s3://bucket/prefix or oci://registry/image)
        #[arg(long)]
        remote_cache: Option<String>,
        /// Build production Docker image (legacy)
        #[arg(long)]
        prod: bool,
        /// Quick build test (standalone output check)
        #[arg(long)]
        quick: bool,
    },

    /// Clean build artifacts (node_modules, .next, dist, etc.)
    Clean {
        /// Preview what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,
    },

    /// Generate deployment bundle (image.tar, artifact.tar.gz, bundle.json)
    Bundle {
        /// Target project path (e.g., apps/web)
        project: String,
        /// Output directory (default: dist/)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Generate Kubernetes manifests (deployment.yaml, service.yaml)
        #[arg(long)]
        k8s: bool,
    },

    /// Run linting (alias for 'run lint')
    Lint,

    /// Run code formatting (alias for 'run format')
    Format,

    /// Run type checking (alias for 'run typecheck')
    Typecheck,

    /// Show Docker container status
    Ps,

    /// View Docker logs
    Logs {
        /// Service name (optional, defaults to all services)
        service: Option<String>,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show from the end
        #[arg(short = 'n', long)]
        tail: Option<u32>,
    },

    /// Execute command in a service container
    Exec {
        /// Service name
        service: String,
        /// Command to execute
        #[arg(trailing_var_arg = true)]
        cmd: Vec<String>,
    },

    /// Restart Docker services
    Restart {
        /// Service name (optional, defaults to all services)
        service: Option<String>,
    },

    /// Docker network management
    Network {
        #[command(subcommand)]
        action: NetworkCommands,
    },

    /// Create new app, service, or library from template
    New {
        #[command(subcommand)]
        template: NewCommands,
    },

    /// Bump version in manifest.toml and Cargo.toml
    #[command(name = "bump-version")]
    BumpVersion {
        /// Bump type (auto-detected if not specified)
        #[arg(long)]
        major: bool,
        #[arg(long)]
        minor: bool,
        #[arg(long)]
        patch: bool,
        /// Auto-detect from commit message (default)
        #[arg(long)]
        auto: bool,
    },

    /// Show affected packages based on git changes
    Affected {
        /// Base branch/commit to compare against (default: origin/main)
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// Head branch/commit (default: HEAD)
        #[arg(long, default_value = "HEAD")]
        head: String,
    },

    /// Generate code and types from various sources
    Generate {
        #[command(subcommand)]
        action: GenerateCommands,
    },

    /// Policy gates for pre-deployment validation
    Policy {
        #[command(subcommand)]
        action: PolicyCommands,
    },
}

#[derive(Subcommand)]
enum PolicyCommands {
    /// Initialize .airis/policies.toml
    Init,
    /// Run policy checks
    Check {
        /// Target project (optional, checks entire workspace if not specified)
        project: Option<String>,
    },
    /// Enforce policies (fail on violations)
    Enforce {
        /// Target project (optional)
        project: Option<String>,
    },
}

#[derive(Subcommand)]
enum GuardsCommands {
    /// Install command guards (.airis/bin/*)
    Install,
    /// Check if running inside Docker container
    #[command(name = "check-docker")]
    CheckDocker,
    /// Show guard status
    Status,
}

#[derive(Subcommand)]
enum HooksCommands {
    /// Install Git hooks (pre-commit for version auto-bump)
    Install,
}

#[derive(Subcommand)]
enum ShimCommands {
    /// Install shims in ./bin (pnpm, npm, node, etc. → Docker)
    Install,
    /// List installed shims
    List,
    /// Remove all shims
    Uninstall,
    /// Execute a command through Docker (manual shim)
    Exec {
        /// Command to execute
        cmd: String,
        /// Arguments to pass
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
}

#[derive(Subcommand)]
enum DocsCommands {
    /// Wrap a documentation file to point to manifest.toml
    Wrap {
        /// File to wrap (CLAUDE.md, .cursorrules, GEMINI.md, AGENTS.md)
        target: String,
    },
    /// List managed documentation files
    List,
}

#[derive(Subcommand)]
enum ManifestCommands {
    /// Print newline-separated list of dev apps
    #[command(name = "dev-apps")]
    DevApps,

    /// Print newline-separated commands registered under [rule.<name>]
    #[command(name = "rule")]
    Rule {
        /// Rule name inside MANIFEST.toml (e.g. verify, ci)
        name: String,
    },

    /// Output workspace truth as JSON (for LLM consumption)
    #[command(name = "json")]
    Json,
}

#[derive(Subcommand)]
enum ValidateCommands {
    /// Validate manifest.toml syntax, app paths, port conflicts, required env vars
    Manifest,
    /// Check for ports: mapping in docker-compose files
    Ports,
    /// Check Traefik network wiring
    Networks,
    /// Check frontend environment variables
    Env,
    /// Check dependency architecture rules (apps -> libs only, no cross-app dependencies)
    #[command(name = "deps")]
    Dependencies,
    /// Check dependency architecture rules (alias for deps)
    #[command(name = "arch")]
    Architecture,
    /// Run all validations
    All,
}

#[derive(Subcommand)]
enum GenerateCommands {
    /// Regenerate workspace files from manifest.toml (package.json, docker-compose.yml, etc.)
    Files {
        /// Preview what would be generated (dry-run)
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate TypeScript types from Supabase PostgreSQL schema
    Types {
        /// Supabase PostgreSQL host (default: localhost)
        #[arg(long, default_value = "localhost")]
        host: String,
        /// Supabase PostgreSQL port (default: 54322)
        #[arg(long, default_value = "54322")]
        port: String,
        /// Database name (default: postgres)
        #[arg(long, default_value = "postgres")]
        database: String,
        /// Output directory (default: libs/types)
        #[arg(short, long, default_value = "libs/types")]
        output: String,
    },
}

#[derive(Subcommand)]
enum NetworkCommands {
    /// Initialize Docker networks for the workspace
    Init,
    /// Setup development networks and start Traefik
    Setup,
    /// List Docker networks for the workspace
    List,
    /// Remove Docker networks for the workspace
    #[command(name = "rm")]
    Remove,
}

#[derive(Subcommand)]
enum NewCommands {
    /// Create a new API service
    Api {
        /// Name of the new service
        name: String,
        /// Runtime/framework (e.g., hono, fastapi, rust-axum)
        #[arg(short, long, default_value = "hono")]
        runtime: String,
    },
    /// Create a new web application
    Web {
        /// Name of the new app
        name: String,
        /// Runtime/framework (e.g., nextjs, vite)
        #[arg(short, long, default_value = "nextjs")]
        runtime: String,
    },
    /// Create a new library
    Lib {
        /// Name of the new library
        name: String,
        /// Runtime/language (e.g., ts)
        #[arg(short, long, default_value = "ts")]
        runtime: String,
    },
    /// Create a new Supabase Edge Function
    Edge {
        /// Name of the new edge function
        name: String,
    },
    /// Create a new Supabase database trigger
    #[command(name = "supabase-trigger")]
    SupabaseTrigger {
        /// Name of the trigger function
        name: String,
    },
    /// Create a new Supabase Realtime handler
    #[command(name = "supabase-realtime")]
    SupabaseRealtime {
        /// Name of the realtime handler
        name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle version flag
    if cli.version {
        println!("airis {}", get_version());
        return Ok(());
    }

    // Require a command if not printing version
    let command = cli.command.unwrap_or_else(|| {
        Cli::command().print_help().unwrap();
        std::process::exit(0);
    });

    match command {
        Commands::Init { snapshot, no_snapshot, setup_npmrc, write, skip_discovery } => {
            commands::init::run(snapshot, no_snapshot, write, skip_discovery)?;
            if setup_npmrc {
                commands::init::setup_npmrc()?;
            }
        }
        Commands::Manifest { action } => {
            use commands::manifest_cmd::{self, ManifestAction};

            let manifest_action = match action {
                ManifestCommands::DevApps => ManifestAction::DevApps,
                ManifestCommands::Rule { name } => ManifestAction::Rule { name },
                ManifestCommands::Json => ManifestAction::Json,
            };

            manifest_cmd::run(manifest_action)?;
        }
        Commands::Guards { action } => match action {
            GuardsCommands::Install => commands::guards::install()?,
            GuardsCommands::CheckDocker => commands::guards::check_docker()?,
            GuardsCommands::Status => commands::guards::status()?,
        },
        Commands::Hooks { action } => match action {
            HooksCommands::Install => commands::hooks::install()?,
        },
        Commands::Shim { action } => match action {
            ShimCommands::Install => commands::shim::install()?,
            ShimCommands::List => commands::shim::list()?,
            ShimCommands::Uninstall => commands::shim::uninstall()?,
            ShimCommands::Exec { cmd, args } => commands::shim::exec(&cmd, &args)?,
        },
        Commands::Docs { action } => match action {
            DocsCommands::Wrap { target } => commands::docs::wrap(&target)?,
            DocsCommands::List => commands::docs::list()?,
        },
        Commands::Validate { action, json } => {
            use commands::validate_cmd::{self, ValidateAction};

            let validate_action = match action {
                ValidateCommands::Manifest => ValidateAction::Manifest,
                ValidateCommands::Ports => ValidateAction::Ports,
                ValidateCommands::Networks => ValidateAction::Networks,
                ValidateCommands::Env => ValidateAction::Env,
                ValidateCommands::Dependencies => ValidateAction::Dependencies,
                ValidateCommands::Architecture => ValidateAction::Architecture,
                ValidateCommands::All => ValidateAction::All,
            };

            validate_cmd::run(validate_action, json)?;
        }
        Commands::Verify => commands::verify::run()?,
        Commands::Doctor { fix, truth, truth_json } => {
            if truth || truth_json {
                commands::doctor::run_truth(truth_json)?;
            } else {
                commands::doctor::run(fix)?;
            }
        }
        Commands::SyncDeps { migrate } => {
            if migrate {
                commands::sync_deps::run_migrate()?;
            } else {
                commands::sync_deps::run()?;
            }
        }
        Commands::Run { task } => commands::run::run(&task)?,
        Commands::Up => commands::run::run("up")?,
        Commands::Down => commands::run::run("down")?,
        Commands::Shell => commands::run::run("shell")?,
        Commands::Dev => commands::run::run("dev")?,
        Commands::Test { coverage_check, min_coverage } => {
            if coverage_check {
                commands::run::run_test_coverage(min_coverage)?;
            } else {
                commands::run::run("test")?;
            }
        }
        Commands::Install => commands::run::run("install")?,
        Commands::Build { project, affected, base, head, docker, channel, targets, parallel, image, push, context_out, no_cache, remote_cache, prod, quick } => {
            if affected && docker {
                // Parallel build for affected projects
                use colored::Colorize;
                let affected_projects = commands::affected::run(&base, &head)?;

                if affected_projects.is_empty() {
                    println!("{}", "✅ No affected projects to build".green());
                } else {
                    let worker_count = parallel.unwrap_or_else(executor::default_parallelism);
                    let root = std::env::current_dir()?;
                    let remote = remote_cache.as_ref().map(|url| remote_cache::Remote::parse(url)).transpose()?;

                    // Build task list
                    let mut exec = executor::ParallelExecutor::new(worker_count);

                    for proj in &affected_projects {
                        let target = convert_package_to_path(proj);
                        let resolved_channel = resolve_channel_for_project(channel.clone(), &target);

                        // Get dependencies from DAG
                        let deps: Vec<String> = {
                            let lock_path = root.join("pnpm-lock.yaml");
                            if let Ok(lock) = pnpm::PnpmLock::load(&lock_path) {
                                let workspace_map = pnpm::build_workspace_map(&lock);
                                let dag = dag::build_dag(&workspace_map);
                                dag.nodes.get(&target)
                                    .map(|n| n.deps.iter()
                                        .filter(|d| affected_projects.iter().any(|ap| convert_package_to_path(ap) == **d))
                                        .cloned()
                                        .collect())
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

                    // Execute in parallel
                    let root_clone = root.clone();
                    let image_clone = image.clone();
                    let context_out_clone = context_out.clone();
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

                                // Check cache first
                                let hash = docker_build::compute_content_hash(&root, &task.target)?;

                                if let Some(_artifact) = docker_build::cache_hit(&task.target, &hash) {
                                    return Ok(executor::TaskResult {
                                        task_id: task.id,
                                        success: true,
                                        duration_ms: start.elapsed().as_millis() as u64,
                                        error: None,
                                    });
                                }

                                // Check remote cache
                                if let Some(ref remote) = remote {
                                    if let Some(artifact) = remote_cache::remote_hit(&task.target, &hash, remote)? {
                                        docker_build::cache_store(&task.target, &hash, &artifact)?;
                                        return Ok(executor::TaskResult {
                                            task_id: task.id,
                                            success: true,
                                            duration_ms: start.elapsed().as_millis() as u64,
                                            error: None,
                                        });
                                    }
                                }

                                // Build
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

                                // Store cache
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
                        }).await
                    })?;

                    let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();
                    if !failed.is_empty() {
                        anyhow::bail!("{} build(s) failed", failed.len());
                    }
                }
            } else if docker {
                // Hermetic Docker build for single project
                let target = project.ok_or_else(|| {
                    anyhow::anyhow!("--docker requires a project path (e.g., apps/web)")
                })?;

                // Multi-target build support
                let build_targets: Vec<String> = if let Some(ref t) = targets {
                    t.clone()
                } else if let Some(ch) = channel.clone() {
                    vec![ch]
                } else {
                    vec![resolve_channel_for_project(None, &target)]
                };

                use colored::Colorize;
                let root = std::env::current_dir()?;

                // Parse remote cache URL if provided
                let remote = remote_cache.as_ref().map(|url| remote_cache::Remote::parse(url)).transpose()?;

                if build_targets.len() > 1 {
                    println!("{}", "==================================".bright_blue());
                    println!("{}", "airis build --docker (multi-target)".bright_blue().bold());
                    println!("Project: {}", target.cyan());
                    println!("Targets: {}", build_targets.join(", ").yellow());
                    println!("{}", "==================================".bright_blue());
                }

                for (idx, build_channel) in build_targets.iter().enumerate() {
                    if build_targets.len() > 1 {
                        println!("\n{}", format!("▶ [{}/{}] Building for target: {}", idx + 1, build_targets.len(), build_channel).bright_blue());
                    }

                    // Calculate content hash for cache lookup (includes channel in hash)
                    let base_hash = docker_build::compute_content_hash(&root, &target)?;
                    let hash = format!("{}-{}", base_hash, build_channel);
                    let final_hash = blake3::hash(hash.as_bytes()).to_hex()[..12].to_string();

                    // Check local cache first
                    if let Some(artifact) = docker_build::cache_hit(&target, &final_hash) {
                        println!("{}", format!("  ✅ Local cache hit: {}", artifact.image_ref).green());
                        continue;
                    }

                    // Check remote cache if configured
                    if let Some(ref remote) = remote {
                        if let Some(artifact) = remote_cache::remote_hit(&target, &final_hash, remote)? {
                            println!("{}", format!("  ✅ Remote cache hit: {}", artifact.image_ref).green());
                            // Store to local cache for next time
                            docker_build::cache_store(&target, &final_hash, &artifact)?;
                            continue;
                        }
                    }

                    // Generate image name with target suffix for multi-target
                    let target_image_name = if build_targets.len() > 1 {
                        image.as_ref().map(|img| {
                            if img.contains(':') {
                                format!("{}-{}", img, build_channel)
                            } else {
                                format!("{}:{}", img, build_channel)
                            }
                        })
                    } else {
                        image.clone()
                    };

                    let config = docker_build::BuildConfig {
                        target: target.clone(),
                        image_name: target_image_name,
                        push,
                        no_cache,
                        context_out: context_out.clone(),
                        channel: build_channel.clone(),
                        ..Default::default()
                    };
                    let result = docker_build::docker_build(&root, config)?;

                    // Store to local cache
                    let artifact = docker_build::CachedArtifact {
                        image_ref: result.image_ref.clone(),
                        hash: final_hash.clone(),
                        built_at: chrono::Utc::now().to_rfc3339(),
                        target: target.clone(),
                    };
                    docker_build::cache_store(&target, &final_hash, &artifact)?;

                    // Store to remote cache if configured
                    if let Some(ref remote) = remote {
                        println!("{}", "  📤 Pushing to remote cache...".cyan());
                        remote_cache::remote_store(&target, &final_hash, &artifact, remote)?;
                    }
                }

                if build_targets.len() > 1 {
                    println!("\n{}", format!("✅ Built {} target(s) for {}", build_targets.len(), target).green().bold());
                }
            } else if prod {
                let app_name = project.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("--prod requires a project path")
                })?;
                commands::run::run_build_prod(app_name)?;
            } else if quick {
                let app_name = project.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("--quick requires a project path")
                })?;
                commands::run::run_build_quick(app_name)?;
            } else {
                commands::run::run("build")?;
            }
        }
        Commands::Clean { dry_run } => commands::clean::run(dry_run)?,
        Commands::Bundle { project, output, k8s } => {
            commands::bundle::run(&project, output.as_deref(), k8s)?;
        }
        Commands::Lint => commands::run::run("lint")?,
        Commands::Format => commands::run::run("format")?,
        Commands::Typecheck => commands::run::run("typecheck")?,
        Commands::Ps => commands::run::run("ps")?,
        Commands::Logs { service, follow, tail } => {
            commands::run::run_logs(service.as_deref(), follow, tail)?
        }
        Commands::Exec { service, cmd } => {
            commands::run::run_exec(&service, &cmd)?
        }
        Commands::Restart { service } => {
            commands::run::run_restart(service.as_deref())?
        }
        Commands::Network { action } => match action {
            NetworkCommands::Init => commands::network::init()?,
            NetworkCommands::Setup => commands::network::setup()?,
            NetworkCommands::List => commands::network::list()?,
            NetworkCommands::Remove => commands::network::remove()?,
        },
        Commands::New { template } => {
            match template {
                NewCommands::Api { name, runtime } => {
                    commands::new_cmd::run_with_runtime("api", &name, &runtime)?;
                }
                NewCommands::Web { name, runtime } => {
                    commands::new_cmd::run_with_runtime("web", &name, &runtime)?;
                }
                NewCommands::Lib { name, runtime } => {
                    commands::new_cmd::run_with_runtime("lib", &name, &runtime)?;
                }
                NewCommands::Edge { name } => {
                    commands::new_cmd::run_with_runtime("edge", &name, "deno")?;
                }
                NewCommands::SupabaseTrigger { name } => {
                    commands::new_cmd::run_with_runtime("supabase-trigger", &name, "plpgsql")?;
                }
                NewCommands::SupabaseRealtime { name } => {
                    commands::new_cmd::run_with_runtime("supabase-realtime", &name, "deno")?;
                }
            }
        }
        Commands::Affected { base, head } => {
            commands::affected::run(&base, &head)?;
        }
        Commands::Generate { action } => match action {
            GenerateCommands::Files { dry_run } => {
                commands::generate::run(dry_run)?;
            }
            GenerateCommands::Types {
                host,
                port,
                database,
                output,
            } => {
                commands::generate_types::run(&host, &port, &database, &output)?;
            }
        },
        Commands::BumpVersion {
            major,
            minor,
            patch,
            auto: _,  // unused but kept for clarity
        } => {
            use commands::bump_version::{self, BumpMode};

            let mode = if major {
                BumpMode::Major
            } else if minor {
                BumpMode::Minor
            } else if patch {
                BumpMode::Patch
            } else {
                // Default to auto
                BumpMode::Auto
            };

            bump_version::run(mode)?;
        }
        Commands::Policy { action } => match action {
            PolicyCommands::Init => commands::policy::init()?,
            PolicyCommands::Check { project } => {
                commands::policy::check(project.as_deref())?;
            }
            PolicyCommands::Enforce { project } => {
                commands::policy::enforce(project.as_deref())?;
            }
        },
    }

    Ok(())
}
