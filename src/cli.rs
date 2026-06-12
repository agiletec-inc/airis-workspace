use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "airis-workspace")]
#[command(about = "Convention engine for polyglot monorepos")]
#[command(long_about = "\
A workspace orchestrator for monorepos.

Generates compose.yaml, tsconfig.json, and AI rule files from manifest.toml. \
Stays out of your way for everything else.

Invoked through the airis dispatcher as `airis workspace <cmd>`.")]
#[command(after_help = "\
QUICK REFERENCE:
  airis workspace gen           Regenerate workspace files from manifest.toml
  airis workspace doctor        Diagnose and fix workspace issues
  airis workspace clean         Remove build artifacts (dry-run by default)
  airis workspace validate all  Validate workspace configuration

CONVENTIONS:
  airis-workspace automatically discovers projects in apps/* and libs/*.
  Use manifest.toml only for overrides.")]
pub struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Query MANIFEST.toml data
    Manifest {
        #[command(subcommand)]
        action: ManifestCommands,
    },

    /// Claude Code / MCP integration
    Claude {
        #[command(subcommand)]
        action: ClaudeCommands,
    },

    /// Project-level cleanup and management
    Workspace(WorkspaceArgs),

    /// Documentation management
    Docs {
        #[command(subcommand)]
        action: DocsCommands,
    },

    /// Validate workspace configuration
    Validate {
        #[command(subcommand)]
        action: ValidateCommands,
        /// Output results as JSON
        #[arg(long, global = true)]
        json: bool,
    },

    /// Run system health checks
    Verify,

    /// Diagnose workspace configuration and show actionable fixes.
    Doctor {
        /// Automatically fix detected issues
        #[arg(long)]
        fix: bool,
        /// Show startup truth
        #[arg(long)]
        truth: bool,
        /// Output startup truth as JSON
        #[arg(long)]
        truth_json: bool,
    },

    /// Clean build artifacts
    Clean {
        /// Preview only (default)
        #[arg(long, default_value_t = false)]
        dry_run: bool,
        /// Remove orphaned or legacy config files (e.g., docker-compose.yml).
        /// Requires manifest.toml so user-managed compose files can be protected.
        #[arg(long)]
        purge: bool,
        /// Actually execute deletions
        #[arg(long)]
        force: bool,
        /// Skip the project-root safety check (run even without
        /// manifest.toml / package.json / Cargo.toml / pyproject.toml / go.mod
        /// in the current directory)
        #[arg(long)]
        allow_anywhere: bool,
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Create new app, service, or library
    New {
        #[command(subcommand)]
        template: NewCommands,
    },

    /// Bump version
    #[command(name = "bump-version")]
    BumpVersion {
        #[arg(long)]
        major: bool,
        #[arg(long)]
        minor: bool,
        #[arg(long)]
        patch: bool,
        #[arg(long)]
        auto: bool,
    },

    /// Regenerate workspace files
    #[command(name = "gen")]
    Gen {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        migrate: bool,
    },

    /// Generate code and types
    Generate {
        #[command(subcommand)]
        action: GenerateCommands,
    },

    /// Policy gates
    Policy {
        #[command(subcommand)]
        action: PolicyCommands,
    },

    /// Dependency graph visualization
    Deps {
        #[command(subcommand)]
        action: DepsCommands,
    },

    /// Preview changes
    Diff {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        stat: bool,
    },

    /// Upgrade airis-workspace
    Upgrade {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        version: Option<String>,
    },

    /// Generate shell completion scripts
    Completion {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Start the MCP server
    Mcp,
}

#[derive(Args)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub action: WorkspaceCommands,
}

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// Uninstall airis from the current workspace (removes hooks and generated files)
    Uninstall,
}

#[derive(Subcommand)]
pub enum PolicyCommands {
    Init,
    Check { project: Option<String> },
    Enforce { project: Option<String> },
}

#[derive(Subcommand)]
pub enum DepsCommands {
    Tree,
    Json,
    Show { package: String },
    Check,
}

#[derive(Subcommand)]
pub enum ClaudeCommands {
    Setup,
    Status,
    Uninstall,
}

#[derive(Subcommand)]
pub enum DocsCommands {
    Wrap {
        target: String,
        /// Overwrite existing target files even when [docs.mode = "warn"].
        #[arg(long)]
        force: bool,
    },
    Sync {
        /// Overwrite existing adapter files even when [docs.mode = "warn"].
        #[arg(long)]
        force: bool,
    },
    List,
}

#[derive(Subcommand)]
pub enum ManifestCommands {
    #[command(name = "dev-apps")]
    DevApps,
    #[command(name = "rule")]
    Rule { name: String },
    #[command(name = "json")]
    Json,
}

#[derive(Subcommand)]
pub enum ValidateCommands {
    Manifest,
    Ports,
    Networks,
    Env,
    #[command(name = "deps")]
    Dependencies,
    #[command(name = "arch")]
    Architecture,
    All,
}

#[derive(Subcommand)]
pub enum GenerateCommands {
    Types {
        #[arg(long, default_value = "localhost")]
        host: String,
        #[arg(long, default_value = "54322")]
        port: String,
        #[arg(long, default_value = "postgres")]
        database: String,
        #[arg(short, long, default_value = "libs/types")]
        output: String,
    },
}

#[derive(Subcommand)]
pub enum NewCommands {
    Api {
        name: String,
        #[arg(short, long, default_value = "hono")]
        runtime: String,
    },
    Web {
        name: String,
        #[arg(short, long, default_value = "nextjs")]
        runtime: String,
    },
    Lib {
        name: String,
        #[arg(short, long, default_value = "ts")]
        runtime: String,
    },
    Edge {
        name: String,
    },
    #[command(name = "supabase-trigger")]
    SupabaseTrigger {
        name: String,
    },
    #[command(name = "supabase-realtime")]
    SupabaseRealtime {
        name: String,
    },
}
