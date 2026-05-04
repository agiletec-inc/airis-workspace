use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "airis")]
#[command(about = "The Docker-first environment orchestrator for the vibe coding era")]
#[command(long_about = "\
The Docker-first environment orchestrator for the vibe coding era.

One manifest file (optional). Every config generated. Your AI pair-programmer \
stays inside the container where it belongs.

airis provides transparent command-level redirection. When you run a guarded \
command (like pnpm or python) in a directory containing compose.yml, it \
automatically executes inside the Docker workspace.

DESIGN: airis acts as a transparent proxy for your existing tools. It ensures \
environment hygiene by isolating build artifacts and dependencies within Docker \
volumes while maintaining a native-like CLI experience.")]
#[command(after_help = "\
QUICK REFERENCE:
  airis up                  Start the environment (via manifest.toml or compose.yml)
  airis run <task>          Run a task (defined in manifest or delegated to Docker)
  airis shell               Enter workspace container shell
  airis doctor              Diagnose and fix workspace issues

SMART SHIMS:
  Run `airis guards install --global` to enable transparent redirection for
  pnpm, npm, uv, python, and more. When a compose.yml is detected, these
  commands automatically execute inside Docker.

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

    /// Manage global command shims (~/.airis/bin)
    Guards {
        #[command(subcommand)]
        action: GuardsCommands,
    },

    /// Project-level cleanup and management
    Workspace(WorkspaceArgs),

    /// Git hooks management (internal)
    #[command(hide = true)]
    Hooks {
        #[command(subcommand)]
        action: HooksCommands,
    },

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

    /// Install dependencies inside Docker container
    Install {
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
        /// Scan test files
        #[arg(long)]
        scan: bool,
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

    /// Build projects
    Build {
        /// Target project path
        project: Option<String>,
        /// Build only affected projects
        #[arg(long)]
        affected: bool,
        /// Base branch/commit for --affected
        #[arg(long, default_value = "origin/main")]
        base: String,
        /// Head branch/commit
        #[arg(long, default_value = "HEAD")]
        head: String,
        /// Build Docker image
        #[arg(long)]
        docker: bool,
        /// Runtime channel
        #[arg(long)]
        channel: Option<String>,
        /// Build for multiple targets (comma-separated, e.g. node,edge,bun,deno)
        #[arg(long, value_delimiter = ',')]
        targets: Option<Vec<String>>,
        /// Number of parallel workers
        #[arg(long, short = 'j')]
        parallel: Option<usize>,
        /// Image name
        #[arg(long)]
        image: Option<String>,
        /// Push image
        #[arg(long)]
        push: bool,
        /// Output directory for build context
        #[arg(long)]
        context_out: Option<std::path::PathBuf>,
        /// No cache
        #[arg(long)]
        no_cache: bool,
        /// Remote cache URL (e.g. s3://bucket/key or oci://registry/repo)
        #[arg(long)]
        remote_cache: Option<String>,
        /// Build production image
        #[arg(long)]
        prod: bool,
        /// Quick build test
        #[arg(long)]
        quick: bool,
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

    /// Generate deployment bundle (image.tar + artifact.tar.gz + bundle.json)
    Bundle {
        /// Target project path
        project: String,
        /// Output directory (defaults to .airis/bundles/<project>/)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
        /// Generate Kubernetes manifests
        #[arg(long)]
        k8s: bool,
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

    /// Show affected packages
    Affected {
        #[arg(long, default_value = "origin/main")]
        base: String,
        #[arg(long, default_value = "HEAD")]
        head: String,
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
pub enum GuardsCommands {
    Install {
        /// Install global guards (~/.airis/bin/) that block commands outside airis projects
        #[arg(long)]
        global: bool,
        /// Guard preset (balanced, strict, permissive)
        #[arg(long, value_enum)]
        preset: Option<crate::manifest::GuardPreset>,
        /// Deprecated: use `airis claude setup` instead
        #[arg(long, hide = true)]
        hooks: bool,
    },
    #[command(name = "check-docker")]
    CheckDocker,
    Status {
        #[arg(long)]
        global: bool,
        #[arg(long, hide = true)]
        hooks: bool,
    },
    Uninstall {
        #[arg(long)]
        global: bool,
        #[arg(long, hide = true)]
        hooks: bool,
    },
    Verify,
    #[command(name = "check-allow")]
    CheckAllow {
        cmd: String,
    },
}

#[derive(Subcommand)]
pub enum ClaudeCommands {
    Setup,
    Status,
    Uninstall,
}

#[derive(Subcommand)]
pub enum HooksCommands {
    Install,
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
