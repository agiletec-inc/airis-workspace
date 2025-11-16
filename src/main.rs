mod commands;
mod config;
mod generators;
mod manifest;
mod templates;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "airis")]
#[command(version)]
#[command(about = "Docker-first monorepo workspace manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize MANIFEST.toml + workspace metadata
    Init,

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

    /// Validate workspace configuration
    Validate,

    /// Sync dependencies: resolve catalog policies to actual versions
    #[command(name = "sync-deps")]
    SyncDeps,

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
    Test,

    /// Install dependencies (alias for 'run install')
    Install,

    /// Build all apps (alias for 'run build')
    Build,

    /// Clean build artifacts (alias for 'run clean')
    Clean,

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
}

#[derive(Subcommand)]
enum GuardsCommands {
    /// Install command guards (.airis/bin/*)
    Install,
}

#[derive(Subcommand)]
enum HooksCommands {
    /// Install Git hooks (pre-commit for version auto-bump)
    Install,
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => commands::init::run()?,
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
        },
        Commands::Hooks { action } => match action {
            HooksCommands::Install => commands::hooks::install()?,
        },
        Commands::Validate => {
            println!("⚠️  Validate command not yet implemented");
        }
        Commands::SyncDeps => commands::sync_deps::run()?,
        Commands::Run { task } => commands::run::run(&task)?,
        Commands::Up => commands::run::run("up")?,
        Commands::Down => commands::run::run("down")?,
        Commands::Shell => commands::run::run("shell")?,
        Commands::Dev => commands::run::run("dev")?,
        Commands::Test => commands::run::run("test")?,
        Commands::Install => commands::run::run("install")?,
        Commands::Build => commands::run::run("build")?,
        Commands::Clean => commands::run::run("clean")?,
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
