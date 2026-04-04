mod channel;
mod commands;
mod conventions;
mod dag;
mod docker_build;
mod executor;
mod generators;
mod import_scanner;
mod manifest;
mod ownership;
mod pnpm;
mod preset;
mod remote_cache;
mod safe_fs;
mod secrets;
mod templates;
#[cfg(test)]
mod test_lock;
mod version_resolver;

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

#[derive(Parser)]
#[command(name = "airis")]
#[command(about = "The Docker-first monorepo manager for the vibe coding era")]
#[command(long_about = "\
The Docker-first monorepo manager for the vibe coding era.

One manifest file. Every config generated. Your AI pair-programmer stays inside \
the container where it belongs.

airis generates compose.yml, package.json, pnpm-workspace.yaml, tsconfig, and \
CI/CD workflows from a single manifest.toml. Command guards keep AI agents from \
running package managers on the host or picking the wrong tool.

DESIGN: airis extends your existing stack — it doesn't replace it. Turborepo, NX, \
Doppler, Vercel, Railway — all your choice. airis handles the Docker layer that \
those tools leave to you.")]
#[command(after_help = "\
QUICK REFERENCE:
  airis init --write        Create manifest.toml from project discovery
  airis gen                 Regenerate all config files from manifest.toml
  airis up                  Start Docker services (local dev only)
  airis guards install      Block npm/yarn/pnpm on host
  airis guards install --hooks  Install Claude Code Docker-First hooks

CONFIG: All commands are defined in manifest.toml [commands] section.
  airis run <task>          Execute any command from [commands]
  airis up/down/shell/...   Built-in aliases for common [commands] entries

MANIFEST SECTIONS:
  [commands]    Command definitions (what 'airis run <task>' executes)
  [guards]      Host command blocking (deny, wrap, forbid)
  [remap]       Auto-translate blocked commands to safe alternatives
  [packages]    Dependency catalog and workspace config")]
struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version")]
    version: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

/// Test level for `airis test --level`
#[derive(Clone, Debug, clap::ValueEnum)]
enum TestLevel {
    Unit,
    Integration,
    E2e,
    Smoke,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize workspace by discovering projects and creating manifest.toml.
    ///
    /// Scans apps/, libs/ for projects, detects frameworks (Next.js, Vite,
    /// Hono, Rust, Python), and generates manifest.toml as single source of truth.
    /// Default is dry-run (preview only). Use --write to execute.
    /// NEVER overwrites existing manifest.toml.
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

    /// Diagnose workspace configuration and show actionable fixes.
    ///
    /// Checks: manifest.toml validity, Docker status, generated file sync,
    /// guard installation, environment variables.
    /// Use --truth for LLM-consumable workspace info (root, compose files,
    /// recommended commands).
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

    /// Execute a command defined in manifest.toml [commands] section.
    ///
    /// Commands are shell strings defined in manifest.toml. airis does not
    /// interpret arguments — the entire command string is executed as-is.
    /// To change what a command does, edit manifest.toml [commands].
    Run {
        /// Task name from manifest.toml [commands] section (e.g., up, down, shell, build, test)
        task: String,
        /// Extra arguments passed after `--` (forwarded to the underlying command)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Build and start all services.
    ///
    /// Rebuilds Docker images and starts containers.
    /// Extra args are forwarded to docker compose (e.g., --no-cache, --force-recreate).
    Up {
        /// Extra arguments forwarded to docker compose (e.g., --no-cache, --force-recreate)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Install dependencies inside Docker container.
    ///
    /// Runs the package manager specified in manifest.toml inside the
    /// workspace container. This is the only way to install dependencies
    /// while keeping the host clean.
    Install {
        /// Extra arguments passed to the package manager
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Stop Docker services (alias for 'airis run down').
    ///
    /// Executes the 'down' command from manifest.toml [commands].
    Down {
        /// Extra arguments forwarded to docker compose (e.g., --volumes, --rmi all)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Enter workspace container shell (alias for 'airis run shell').
    ///
    /// Executes the 'shell' command from manifest.toml [commands].
    /// Inside the container, you can run package manager commands directly.
    Shell {
        /// Extra arguments forwarded to the shell command
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Run tests (alias for 'run test')
    Test {
        /// Test level: unit, integration, e2e, smoke (resolves to [commands].test:<level>)
        #[arg(long, value_enum)]
        level: Option<TestLevel>,
        /// Check coverage threshold
        #[arg(long)]
        coverage_check: bool,
        /// Minimum coverage percentage (default: 80)
        #[arg(long, default_value = "80")]
        min_coverage: u8,
        /// Extra arguments passed after `--` (forwarded to the underlying command)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Build projects (alias for 'airis run build', or Docker build with --docker).
    ///
    /// Without --docker: executes 'build' from manifest.toml [commands].
    /// With --docker: builds the prod target of the service Dockerfile.
    /// With --affected: only builds projects changed since --base.
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
        /// Build Docker image using the prod target of the service Dockerfile
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
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
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
    Lint {
        /// Extra arguments passed after `--` (forwarded to the underlying command)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Run code formatting (alias for 'run format')
    Format {
        /// Extra arguments passed after `--` (forwarded to the underlying command)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Run type checking (alias for 'run typecheck')
    Typecheck {
        /// Extra arguments passed after `--` (forwarded to the underlying command)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Show Docker container status
    Ps {
        /// Extra arguments forwarded to docker compose ps
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

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

    /// Regenerate workspace files from manifest.toml.
    ///
    /// Generates: package.json, pnpm-workspace.yaml, compose.yml,
    /// per-service Dockerfile (multi-stage dev/prod), CI workflows.
    /// All generated files include DO NOT EDIT markers.
    /// Safe to run repeatedly — always produces the same output
    /// from the same manifest.toml.
    #[command(name = "gen")]
    Gen {
        /// Preview what would be generated (dry-run)
        #[arg(long)]
        dry_run: bool,
        /// Force generation even if legacy compose files exist
        #[arg(long)]
        force: bool,
        /// Migrate legacy compose files (docker-compose.yml etc.) to compose.yml
        #[arg(long)]
        migrate: bool,
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

    /// Dependency graph visualization and analysis
    Deps {
        #[command(subcommand)]
        action: DepsCommands,
    },

    /// Preview changes between manifest.toml and generated files
    Diff {
        /// Output as JSON (for CI/automation)
        #[arg(long)]
        json: bool,
        /// Show statistics only (file count, line changes)
        #[arg(long)]
        stat: bool,
    },

    /// Upgrade airis to the latest version
    Upgrade {
        /// Only check for updates (don't install)
        #[arg(long)]
        check: bool,
        /// Install specific version (e.g., 1.60.0)
        #[arg(long)]
        version: Option<String>,
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
enum DepsCommands {
    /// Display ASCII dependency tree
    Tree,
    /// Output dependency graph as JSON (for LLM/automation)
    Json,
    /// Show dependencies for a specific package
    Show {
        /// Package path or name (e.g., apps/web, libs/ui)
        package: String,
    },
    /// Check for circular dependencies and architecture violations
    Check,
}

#[derive(Subcommand)]
enum GuardsCommands {
    /// Install command guards that block package managers on host.
    ///
    /// Creates shell scripts in .airis/bin/ (or ~/.airis/bin/ with --global)
    /// that intercept denied commands. When a blocked command runs, it shows
    /// an error with the correct airis alternative.
    /// Guard rules come from manifest.toml [guards] section.
    Install {
        /// Install global guards (~/.airis/bin/) that block commands outside airis projects
        #[arg(long)]
        global: bool,
        /// Install Claude Code hooks (~/.claude/) for Docker-First enforcement
        #[arg(long)]
        hooks: bool,
    },
    /// Check if running inside Docker container
    #[command(name = "check-docker")]
    CheckDocker,
    /// Show guard status
    Status {
        /// Show global guards status
        #[arg(long)]
        global: bool,
        /// Show Claude Code hooks status
        #[arg(long)]
        hooks: bool,
    },
    /// Uninstall command guards
    Uninstall {
        /// Uninstall global guards
        #[arg(long)]
        global: bool,
        /// Remove Claude Code hooks
        #[arg(long)]
        hooks: bool,
    },
    /// Verify global guards are properly installed and active
    Verify,
    /// Check if a command is allowed in the current repo (used by global guard scripts)
    #[command(name = "check-allow")]
    CheckAllow {
        /// Command name to check
        cmd: String,
    },
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
    /// Generate a vendor adapter file from shared AI docs
    Wrap {
        /// Adapter file to generate (CLAUDE.md, .cursorrules, GEMINI.md, AGENTS.md)
        target: String,
    },
    /// Generate all configured AI documentation adapters
    Sync,
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

    /// Output workspace configuration as JSON for LLM/automation consumption.
    ///
    /// Includes: workspace_root, compose_files, compose_command, service,
    /// workdir, package_manager, recommended_commands.
    #[command(name = "json")]
    Json,
}

#[derive(Subcommand)]
enum ValidateCommands {
    /// Validate manifest.toml: syntax, port conflicts, catalog references, guard consistency.
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

    dispatch(command)
}

/// Dispatch a parsed CLI command to the appropriate handler.
fn dispatch(command: Commands) -> Result<()> {
    match command {
        Commands::Init {
            snapshot,
            no_snapshot,
            setup_npmrc,
            write,
            skip_discovery,
        } => {
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
            GuardsCommands::Install { global, hooks } => {
                if hooks {
                    commands::claude_setup::setup_global()?;
                } else if global {
                    commands::guards::install_global()?;
                } else {
                    commands::guards::install()?;
                }
            }
            GuardsCommands::CheckDocker => commands::guards::check_docker()?,
            GuardsCommands::Status { global, hooks } => {
                if hooks {
                    commands::claude_setup::status()?;
                } else if global {
                    commands::guards::status_global()?;
                } else {
                    commands::guards::status()?;
                }
            }
            GuardsCommands::Uninstall { global, hooks } => {
                if hooks {
                    commands::claude_setup::uninstall()?;
                } else if global {
                    commands::guards::uninstall_global()?;
                } else {
                    commands::guards::uninstall()?;
                }
            }
            GuardsCommands::Verify => commands::guards::verify_global()?,
            GuardsCommands::CheckAllow { cmd } => commands::guards::check_allow(&cmd)?,
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
            DocsCommands::Sync => commands::docs::sync()?,
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
        Commands::Doctor {
            fix,
            truth,
            truth_json,
        } => {
            if truth || truth_json {
                commands::doctor::run_truth(truth_json)?;
            } else {
                commands::doctor::run(fix)?;
            }
        }
        Commands::Run { task, extra_args } => commands::run::run(&task, &extra_args)?,
        Commands::Up { extra_args } => commands::run::run("up", &extra_args)?,
        Commands::Install { extra_args } => commands::install::run(&extra_args)?,
        Commands::Down { extra_args } => commands::run::run("down", &extra_args)?,
        Commands::Shell { extra_args } => commands::run::run("shell", &extra_args)?,
        Commands::Test {
            level,
            coverage_check,
            min_coverage,
            extra_args,
        } => {
            if let Some(lvl) = level {
                let task = match lvl {
                    TestLevel::Unit => "test:unit",
                    TestLevel::Integration => "test:integration",
                    TestLevel::E2e => "test:e2e",
                    TestLevel::Smoke => "test:smoke",
                };
                commands::run::run(task, &extra_args)?;
            } else if coverage_check {
                commands::run::run_test_coverage(min_coverage)?;
            } else {
                commands::run::run("test", &extra_args)?;
            }
        }
        Commands::Build {
            project,
            affected,
            base,
            head,
            docker,
            channel,
            targets,
            parallel,
            image,
            push,
            context_out,
            no_cache,
            remote_cache,
            prod,
            quick,
        } => {
            let opts = commands::build::DockerBuildOpts {
                channel,
                targets,
                parallel,
                image,
                push,
                context_out,
                no_cache,
                remote_cache,
            };

            if affected && docker {
                commands::build::build_affected_docker(&base, &head, &opts)?;
            } else if docker {
                let target = project.ok_or_else(|| {
                    anyhow::anyhow!("--docker requires a project path (e.g., apps/web)")
                })?;
                commands::build::build_docker(&target, &opts)?;
            } else if prod {
                let app_name = project
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("--prod requires a project path"))?;
                commands::run::run_build_prod(app_name)?;
            } else if quick {
                let app_name = project
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("--quick requires a project path"))?;
                commands::run::run_build_quick(app_name)?;
            } else {
                commands::run::run("build", &[])?;
            }
        }
        Commands::Clean {
            dry_run,
            extra_args: _,
        } => commands::clean::run(dry_run)?,
        Commands::Bundle {
            project,
            output,
            k8s,
        } => {
            commands::bundle::run(&project, output.as_deref(), k8s)?;
        }
        Commands::Lint { extra_args } => commands::run::run("lint", &extra_args)?,
        Commands::Format { extra_args } => commands::run::run("format", &extra_args)?,
        Commands::Typecheck { extra_args } => commands::run::run("typecheck", &extra_args)?,
        Commands::Ps { extra_args } => {
            if extra_args.is_empty() {
                commands::run::run_ps()?;
            } else {
                commands::run::run("ps", &extra_args)?;
            }
        }
        Commands::Logs {
            service,
            follow,
            tail,
        } => commands::run::run_logs(service.as_deref(), follow, tail)?,
        Commands::Exec { service, cmd } => commands::run::run_exec(&service, &cmd)?,
        Commands::Restart { service } => commands::run::run_restart(service.as_deref())?,
        Commands::Network { action } => match action {
            NetworkCommands::Init => commands::network::init()?,
            NetworkCommands::Setup => commands::network::setup()?,
            NetworkCommands::List => commands::network::list()?,
            NetworkCommands::Remove => commands::network::remove()?,
        },
        Commands::New { template } => match template {
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
        },
        Commands::Affected { base, head } => {
            commands::affected::run(&base, &head)?;
        }
        Commands::Gen {
            dry_run,
            force,
            migrate,
        } => {
            commands::generate::run(dry_run, force, migrate)?;
        }
        Commands::Generate { action } => match action {
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
            auto: _, // unused but kept for clarity
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
        Commands::Deps { action } => match action {
            DepsCommands::Tree => commands::deps::tree()?,
            DepsCommands::Json => commands::deps::json()?,
            DepsCommands::Show { package } => commands::deps::show(&package)?,
            DepsCommands::Check => commands::deps::check()?,
        },
        Commands::Diff { json, stat } => {
            use commands::diff::DiffFormat;
            let format = if json {
                DiffFormat::Json
            } else if stat {
                DiffFormat::Stat
            } else {
                DiffFormat::Unified
            };
            commands::diff::run(format)?;
        }
        Commands::Upgrade { check, version } => {
            if check {
                commands::upgrade::run_check()?;
            } else {
                commands::upgrade::run(version)?;
            }
        }
    }

    Ok(())
}
