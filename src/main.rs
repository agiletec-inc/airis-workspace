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

    /// Validate workspace configuration
    Validate,

    /// Sync dependencies: resolve catalog policies to actual versions
    #[command(name = "sync-deps")]
    SyncDeps,
}

#[derive(Subcommand)]
enum GuardsCommands {
    /// Install command guards (.airis/bin/*)
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
        Commands::Validate => {
            println!("⚠️  Validate command not yet implemented");
        }
        Commands::SyncDeps => commands::sync_deps::run()?,
    }

    Ok(())
}
