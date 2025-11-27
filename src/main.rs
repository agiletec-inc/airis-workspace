mod commands;
mod config;
mod generators;
mod manifest;
mod ownership;
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
    },

    /// Run system health checks
    Verify,

    /// Diagnose and heal workspace configuration issues
    Doctor {
        /// Automatically fix detected issues
        #[arg(long)]
        fix: bool,
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
        /// Build production Docker image
        #[arg(long)]
        prod: bool,
        /// Quick build test (standalone output check)
        #[arg(long)]
        quick: bool,
        /// App name (required for --prod or --quick)
        app: Option<String>,
    },

    /// Clean build artifacts (alias for 'run clean')
    Clean,

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
}

#[derive(Subcommand)]
enum ValidateCommands {
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
    /// Regenerate workspace files from manifest.toml (package.json, compose.yml, etc.)
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
        Commands::Init { snapshot, no_snapshot, setup_npmrc, write } => {
            commands::init::run(snapshot, no_snapshot, write)?;
            if setup_npmrc {
                commands::init::setup_npmrc()?;
            }
        }
        Commands::Manifest { action } => {
            use commands::manifest_cmd::{self, ManifestAction};

            let manifest_action = match action {
                ManifestCommands::DevApps => ManifestAction::DevApps,
                ManifestCommands::Rule { name } => ManifestAction::Rule { name },
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
        Commands::Validate { action } => {
            use commands::validate_cmd::{self, ValidateAction};

            let validate_action = match action {
                ValidateCommands::Ports => ValidateAction::Ports,
                ValidateCommands::Networks => ValidateAction::Networks,
                ValidateCommands::Env => ValidateAction::Env,
                ValidateCommands::Dependencies => ValidateAction::Dependencies,
                ValidateCommands::Architecture => ValidateAction::Architecture,
                ValidateCommands::All => ValidateAction::All,
            };

            validate_cmd::run(validate_action)?;
        }
        Commands::Verify => commands::verify::run()?,
        Commands::Doctor { fix } => commands::doctor::run(fix)?,
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
        Commands::Build { prod, quick, app } => {
            if prod {
                let app_name = app.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("--prod requires --app <name>")
                })?;
                commands::run::run_build_prod(app_name)?;
            } else if quick {
                let app_name = app.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("--quick requires --app <name>")
                })?;
                commands::run::run_build_quick(app_name)?;
            } else {
                commands::run::run("build")?;
            }
        }
        Commands::Clean => commands::run::run("clean")?,
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
    }

    Ok(())
}
