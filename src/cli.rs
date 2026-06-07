use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "airis")]
#[command(about = "The Docker-first environment orchestrator for the vibe coding era")]
#[command(long_about = "\
A workspace orchestrator for monorepos.

Generates compose.yaml, tsconfig.json, and AI rule files from manifest.toml. \
Drives Docker Compose for local dev. Stays out of your way for everything else.")]
#[command(after_help = "\
QUICK REFERENCE:
  airis up                  Start the environment (via manifest.toml or compose.yml)
  airis run <task>          Run a task (defined in manifest or delegated to Docker)
  airis shell               Enter workspace container shell
  airis doctor              Diagnose and fix workspace issues

CONVENTIONS:
  airis automatically discovers projects in apps/* and libs/*. Use manifest.toml
  only for overrides. If no manifest.toml is present, airis falls back to
  standard Docker Compose behavior.")]
pub struct Cli {
    /// Print version
    #[arg(short = 'V', long = "version")]
    pub version: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Test level for `airis test --level`
#[derive(Clone, Debug, clap::ValueEnum)]
pub enum TestLevel {
    Unit,
    Integration,
    E2e,
    Smoke,
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

    /// Execute a command defined in manifest.toml [commands], or delegate to Docker if compose.yml exists.
    Run {
        /// Task name (e.g., build, test)
        task: String,
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Start the entire Docker-first workspace (via manifest.toml or compose.yml)
    Up {
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Stop Docker services
    Down {
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Enter workspace container shell
    Shell {
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Run tests
    Test {
        /// Test level: unit, integration, e2e, smoke
        #[arg(long, value_enum)]
        level: Option<TestLevel>,
        /// Check coverage threshold
        #[arg(long)]
        coverage_check: bool,
        /// Minimum coverage percentage
        #[arg(long, default_value = "80")]
        min_coverage: u8,
        /// Extra arguments
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
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

    /// Show current workspace and guard status
    Status {
        /// Show a concise one-line status (for shell prompts)
        #[arg(long, short = 's')]
        short: bool,
    },

    /// Run linting
    Lint {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Run code formatting
    Format {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Run type checking
    Typecheck {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// Show Docker container status
    Ps {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        extra_args: Vec<String>,
    },

    /// View Docker logs
    Logs {
        service: Option<String>,
        #[arg(short, long)]
        follow: bool,
        #[arg(short = 'n', long)]
        tail: Option<u32>,
    },

    /// Execute a command in a workspace service container.
    ///
    /// Service is auto-resolved from the command's runtime family
    /// (pnpm/npm/node → workspace, python/uv → workspace, cargo → workspace).
    /// Override with `--service`, or pass a service name as the first
    /// positional argument for backward compatibility:
    ///
    /// ```text
    /// airis exec pnpm install              # auto-route
    /// airis exec --service api ls          # explicit
    /// airis exec workspace pnpm install    # legacy positional form
    /// ```
    Exec {
        /// Explicit service to exec into (takes precedence over auto-routing).
        #[arg(long, short = 's')]
        service: Option<String>,
        /// Skip the auto-up that runs when the resolved service is stopped.
        #[arg(long)]
        no_auto_up: bool,
        /// Command and its arguments.
        #[arg(trailing_var_arg = true, required = true, allow_hyphen_values = true)]
        cmd: Vec<String>,
    },

    /// Restart Docker services
    Restart { service: Option<String> },

    /// Docker network management
    Network {
        #[command(subcommand)]
        action: NetworkCommands,
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

    /// Upgrade airis
    Upgrade {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        version: Option<String>,
    },

    /// Initialize shell integration (prompt, etc.)
    InitShell {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
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
    /// Uninstall airis from the current workspace (removes shims, hooks, and generated files)
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
pub enum NetworkCommands {
    Init,
    Setup,
    List,
    #[command(name = "rm")]
    Remove,
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
