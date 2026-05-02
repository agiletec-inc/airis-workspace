use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_version() -> u32 {
    1
}

pub(crate) fn schema_default_version() -> u32 {
    default_version()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Manifest {
    #[serde(default = "default_version")]
    pub version: u32,
    /// Project metadata (SoT for Cargo.toml, Homebrew, etc.)
    #[serde(default)]
    pub project: MetaSection,
    #[serde(default)]
    pub workspace: WorkspaceSection,
    #[serde(default)]
    pub catalog: IndexMap<String, String>,
    #[serde(default)]
    pub workspaces: WorkspacesSection,
    #[serde(default)]
    pub dev: HooksSection,
    #[serde(default)]
    pub apps: IndexMap<String, AppConfig>,
    #[serde(default)]
    pub libs: IndexMap<String, LibConfig>,
    #[serde(default)]
    pub docker: DockerSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub just: Option<JustSection>,
    #[serde(default)]
    pub service: IndexMap<String, ServiceConfig>,
    #[serde(default)]
    pub rule: IndexMap<String, RuleConfig>,
    #[serde(default)]
    pub packages: PackagesSection,
    #[serde(default)]
    pub guards: GuardsSection,
    /// App definitions (for package.json generation)
    #[serde(default)]
    pub app: Vec<ProjectDefinition>,
    #[serde(default)]
    pub orchestration: OrchestrationSection,
    /// Pre-command hooks (e.g., auto-install before test/build)
    #[serde(default)]
    pub hooks: PreCommandHooks,
    /// User-defined commands (airis run <task>)
    #[serde(default)]
    pub commands: IndexMap<String, String>,
    /// LLM command remapping (e.g., "npm install" → "airis install")
    #[serde(default)]
    pub remap: IndexMap<String, String>,
    /// Version management configuration
    #[serde(default)]
    pub versioning: VersioningSection,
    /// Documentation management (CLAUDE.md, .cursorrules, etc.)
    #[serde(default)]
    pub docs: DocsSection,
    /// CI/CD configuration
    #[serde(default)]
    pub ci: CiSection,
    /// Template definitions for airis new
    #[serde(default)]
    pub templates: TemplatesSection,
    /// Runtime aliases for airis new
    #[serde(default)]
    pub runtimes: RuntimesSection,
    /// Environment variable validation
    #[serde(default)]
    pub env: EnvSection,
    /// Secret provider configuration (e.g., Doppler)
    #[serde(default)]
    pub secrets: Option<SecretsSection>,
    /// TypeScript configuration for tsconfig generation
    #[serde(default)]
    pub typescript: TypescriptSection,

    /// User-defined technology stacks (artifacts, verify commands, images)
    #[serde(default)]
    pub stack: IndexMap<String, StackDefinition>,

    /// Reusable dependency groups (e.g., shadcn radix-ui components)
    #[serde(default)]
    pub dep_group: IndexMap<String, IndexMap<String, String>>,
    /// Reusable environment variable groups (e.g., supabase-full, supabase-backend)
    #[serde(default)]
    pub env_group: IndexMap<String, IndexMap<String, String>>,

    // ── v2 fields (ignored when version = 1) ──
    /// Environment profiles: local, stg, prd, etc.
    #[serde(default)]
    pub profile: IndexMap<String, ProfileSection>,
    /// Reusable app presets (deps/scripts/deploy defaults)
    #[serde(default)]
    pub preset: IndexMap<String, PresetSection>,
    /// External third-party services (not built from source)
    #[serde(default)]
    pub external: IndexMap<String, ExternalServiceConfig>,
    /// Root package.json config (v2: replaces packages.root)
    #[serde(default)]
    pub root: Option<RootSection>,
    /// pnpm overrides (v2: replaces packages.root.pnpm.overrides)
    #[serde(default)]
    pub overrides: IndexMap<String, String>,
    /// MCP Gateway configuration for this project
    #[serde(default)]
    pub mcp: McpSection,
    /// Testing governance configuration (deprecated: use [policy.testing])
    #[serde(default)]
    pub testing: TestingSection,
    /// Code governance policy
    #[serde(default)]
    pub policy: PolicySection,
}

/// Project metadata - Source of Truth for Cargo.toml, Homebrew formula, etc.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MetaSection {
    /// Project ID (e.g., "airis-workspace")
    #[serde(default)]
    pub id: String,
    /// CLI binary name (e.g., "airis")
    #[serde(default)]
    pub binary_name: String,
    /// Semantic version (e.g., "1.4.0")
    #[serde(default)]
    pub version: String,
    /// Short description
    #[serde(default)]
    pub description: String,
    /// Authors list
    #[serde(default)]
    pub authors: Vec<String>,
    /// License (e.g., "MIT")
    #[serde(default)]
    pub license: String,
    /// Project homepage URL
    #[serde(default)]
    pub homepage: String,
    /// Repository URL
    #[serde(default)]
    pub repository: String,
    /// Keywords for discovery
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Categories for classification
    #[serde(default)]
    pub categories: Vec<String>,
    /// Rust edition (e.g., "2024")
    #[serde(default)]
    pub rust_edition: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceSection {
    #[serde(default = "default_workspace_name")]
    pub name: String,
    /// Default npm scope for packages (e.g., "@agiletec")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default = "default_package_manager")]
    pub package_manager: String,
    /// Node.js version (v2: preferred over extracting from image string)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    /// Deprecated: workspace container has been removed. Kept for backwards compatibility with existing manifest.toml files.
    #[serde(default, skip_serializing)]
    pub service: String,
    #[serde(default = "default_workspace_image")]
    pub image: String,
    #[serde(default = "default_workspace_workdir")]
    pub workdir: String,
    /// Workspace patterns (v2: replaces [packages].workspaces)
    #[serde(default)]
    pub workspaces: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub clean: CleanSection,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CleanSection {
    /// Root directories to remove (e.g., ".next", "dist", "build")
    #[serde(default = "default_clean_dirs")]
    pub dirs: Vec<String>,
    /// Patterns to find and remove recursively (e.g., "node_modules")
    #[serde(default = "default_clean_recursive")]
    pub recursive: Vec<String>,
}

impl Default for CleanSection {
    fn default() -> Self {
        CleanSection {
            dirs: default_clean_dirs(),
            recursive: default_clean_recursive(),
        }
    }
}

fn default_clean_dirs() -> Vec<String> {
    vec![
        ".next".to_string(),
        "dist".to_string(),
        "build".to_string(),
        "out".to_string(),
        ".turbo".to_string(),
        ".swc".to_string(),
        ".cache".to_string(),
    ]
}

fn default_clean_recursive() -> Vec<String> {
    vec![
        "node_modules".to_string(),
        ".pnpm".to_string(),
        ".pnpm-store".to_string(),
    ]
}

impl Default for WorkspaceSection {
    fn default() -> Self {
        WorkspaceSection {
            name: default_workspace_name(),
            scope: None,
            package_manager: default_package_manager(),
            node: None,
            service: String::new(),
            image: default_workspace_image(),
            workdir: default_workspace_workdir(),
            workspaces: vec![],
            volumes: vec!["workspace-node-modules:/app/node_modules".to_string()],
            clean: CleanSection::default(),
        }
    }
}

fn default_workspace_name() -> String {
    // Try to get git repo root directory name
    if let Ok(output) = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        && output.status.success()
        && let Ok(path_str) = String::from_utf8(output.stdout)
    {
        let path = std::path::Path::new(path_str.trim());
        if let Some(name) = path.file_name()
            && let Some(name_str) = name.to_str()
        {
            return name_str.to_string();
        }
    }

    // Fallback: use current directory name
    if let Ok(cwd) = std::env::current_dir()
        && let Some(name) = cwd.file_name()
        && let Some(name_str) = name.to_str()
    {
        return name_str.to_string();
    }

    "workspace".to_string()
}

fn default_package_manager() -> String {
    String::new()
}

fn default_workspace_image() -> String {
    crate::channel::defaults::NODE_LTS_IMAGE.to_string()
}

fn default_workspace_workdir() -> String {
    "/app".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HooksSection {
    /// Glob pattern for auto-discovering app docker-compose files
    /// Default: "apps/*/compose.yml"
    #[serde(default = "default_apps_pattern")]
    pub apps_pattern: String,
    /// Supabase compose files (e.g., ["supabase/compose.yml"])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase: Option<Vec<String>>,
    /// Traefik compose file (e.g., "traefik/compose.yml")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traefik: Option<String>,
    /// URLs to display after `airis up` (optional, dynamic from apps if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<ServiceUrls>,
    /// Commands to run after `airis up` (e.g., DB migration)
    #[serde(default)]
    pub post_up: Vec<String>,
    /// Timeout in seconds for service reachability checks after `airis up`.
    /// Services are polled every 2s until reachable or this timeout expires.
    /// Default: 30 seconds. Set to 0 to skip waiting.
    #[serde(default = "default_reachability_timeout")]
    pub reachability_timeout: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServiceUrls {
    /// Infrastructure URLs (e.g., Supabase Studio, Traefik Dashboard)
    #[serde(default)]
    pub infra: Vec<UrlEntry>,
    /// Application URLs
    #[serde(default)]
    pub apps: Vec<UrlEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UrlEntry {
    /// Display name (e.g., "Dashboard", "Supabase Studio")
    pub name: String,
    /// URL (e.g., "http://localhost:3000")
    pub url: String,
}

impl Default for HooksSection {
    fn default() -> Self {
        HooksSection {
            apps_pattern: default_apps_pattern(),
            supabase: None,
            traefik: None,
            urls: None,
            post_up: Vec::new(),
            reachability_timeout: default_reachability_timeout(),
        }
    }
}

fn default_reachability_timeout() -> u64 {
    30
}

fn default_apps_pattern() -> String {
    "apps/*/compose.yml".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub app_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub scripts: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub deps: IndexMap<String, String>,
    #[serde(
        rename = "devDeps",
        default,
        skip_serializing_if = "IndexMap::is_empty"
    )]
    pub dev_deps: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
pub struct LibConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub scripts: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub deps: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServiceConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Build configuration. When set, compose uses `build:` instead of `image:`.
    /// Format: { context = ".", dockerfile = "apps/web/Dockerfile", target = "dev" }
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shm_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub extra_hosts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deploy: Option<DeployConfig>,
    #[serde(default)]
    pub watch: Vec<WatchConfig>,
    /// Device mappings (e.g., "/dev/dri:/dev/dri")
    #[serde(default)]
    pub devices: Vec<String>,
    /// Container runtime (e.g., "nvidia")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<String>,
    /// GPU resource reservation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu: Option<GpuConfig>,
    /// Health check path (e.g., "/api/health", "/healthz")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
    /// Network mode (e.g., "host", "bridge")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_mode: Option<String>,
    /// Labels (e.g., traefik routing labels)
    #[serde(default)]
    pub labels: Vec<String>,
    /// Networks this service joins (e.g., ["default", "proxy"])
    #[serde(default)]
    pub networks: Vec<String>,
    /// References to env_group names for DRY env configuration
    #[serde(default)]
    pub env_groups: Vec<String>,
    /// Memory limit for Docker container (e.g., "2g", "4g")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mem_limit: Option<String>,
    /// CPU limit for Docker container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployConfig {
    pub replicas: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    /// Build context directory (default: ".")
    #[serde(default = "default_dot")]
    pub context: String,
    /// Path to Dockerfile relative to context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dockerfile: Option<String>,
    /// Multi-stage target (e.g., "dev", "prod")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

fn default_dot() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// GPU driver (default: "nvidia")
    #[serde(default = "default_gpu_driver")]
    pub driver: String,
    /// Number of GPUs ("all" or "1", "2", etc.)
    #[serde(default = "default_gpu_count")]
    pub count: String,
    /// Capabilities (default: ["gpu"])
    #[serde(default = "default_gpu_capabilities")]
    pub capabilities: Vec<String>,
}

fn default_gpu_driver() -> String {
    "nvidia".to_string()
}
fn default_gpu_count() -> String {
    "all".to_string()
}
fn default_gpu_capabilities() -> Vec<String> {
    vec!["gpu".to_string()]
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WatchConfig {
    pub path: String,
    pub action: String,
    pub target: String,
    #[serde(default)]
    pub initial_sync: bool,
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuleConfig {
    #[serde(default)]
    pub commands: Vec<String>,
}

/// Pre-command hooks configuration.
/// Runs a command before each `airis run <task>` invocation.
/// Cache key avoids re-running when dependencies haven't changed.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PreCommandHooks {
    /// Shell command to run before each airis command (e.g., "pnpm install")
    #[serde(default)]
    pub pre_command: Option<String>,
    /// Commands that skip the pre_command hook (e.g., ["up", "down", "ps"])
    #[serde(default)]
    pub skip: Vec<String>,
    /// Cache config: only run hook when key file changes
    #[serde(default)]
    pub cache: Option<HookCache>,
}

/// Cache configuration for pre-command hooks.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HookCache {
    /// File whose SHA256 hash determines whether to run the hook
    pub key: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackagesSection {
    #[serde(default)]
    pub workspaces: Vec<String>,
    /// Default version policy for packages not explicitly listed in catalog.
    /// When set (e.g., "latest"), import-scanned packages that don't match
    /// any catalog entry or wildcard pattern will use this policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_policy: Option<String>,
    #[serde(default)]
    pub catalog: IndexMap<String, CatalogEntry>,
    #[serde(default)]
    pub root: PackageDefinition,
    #[serde(rename = "app", default)]
    pub app: Vec<AppPackageDefinition>,
}

/// Catalog entry can be:
/// - "latest" → resolve to latest npm version
/// - "lts" → resolve to LTS version (treated same as latest for npm packages)
/// - {} → empty table, treated as "latest" (shorthand for just registering a key)
/// - "^5.0.0" → specific semver (used as-is)
/// - { follow = "react" } → follow another package's version
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogEntry {
    Follow(FollowConfig),
    Policy(VersionPolicy),
    Empty(EmptyTable),
    Version(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowConfig {
    pub follow: String,
}

/// Empty table `{}` — treated as "latest"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmptyTable {}

impl Default for CatalogEntry {
    fn default() -> Self {
        CatalogEntry::Policy(VersionPolicy::Latest)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionPolicy {
    Latest,
    Lts,
}

impl VersionPolicy {
    pub fn as_str(&self) -> &str {
        match self {
            VersionPolicy::Latest => "latest",
            VersionPolicy::Lts => "lts",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackageDefinition {
    #[serde(default)]
    pub dependencies: IndexMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: IndexMap<String, String>,
    #[serde(rename = "optionalDependencies", default)]
    pub optional_dependencies: IndexMap<String, String>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(default)]
    pub engines: IndexMap<String, String>,
    #[serde(default)]
    pub pnpm: PnpmConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AppPackageDefinition {
    pub pattern: String,
    #[serde(default)]
    pub dependencies: IndexMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: IndexMap<String, String>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PnpmConfig {
    #[serde(default)]
    pub overrides: IndexMap<String, String>,
    #[serde(rename = "peerDependencyRules", default)]
    pub peer_dependency_rules: PeerDependencyRules,
    #[serde(rename = "onlyBuiltDependencies", default)]
    pub only_built_dependencies: Vec<String>,
    #[serde(rename = "allowedScripts", default)]
    pub allowed_scripts: IndexMap<String, bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PeerDependencyRules {
    #[serde(rename = "ignoreMissing", default)]
    pub ignore_missing: Vec<String>,
    #[serde(rename = "allowedVersions", default)]
    pub allowed_versions: IndexMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct GuardsSection {
    /// Commands to completely deny (e.g., ["npm", "yarn"])
    #[serde(default)]
    pub deny: Vec<String>,

    /// Commands to allow (opt-out from global deny list for this repo)
    #[serde(default)]
    pub allow: Vec<String>,

    /// Commands to wrap with Docker execution
    /// e.g., {"pnpm": "docker compose exec workspace pnpm"}
    #[serde(default)]
    pub wrap: IndexMap<String, String>,

    /// Commands to deny with custom messages
    #[serde(default)]
    pub deny_with_message: IndexMap<String, String>,

    /// LLM-specific: completely forbid these commands
    #[serde(default)]
    pub forbid: Vec<String>,

    /// LLM-specific: dangerous commands (warn humans, block LLMs)
    #[serde(default)]
    pub danger: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct WorkspacesSection {
    #[serde(default)]
    pub apps: Vec<WorkspaceAppMeta>,
    #[serde(default)]
    pub libs: Vec<WorkspaceLibMeta>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceAppMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub app_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkspaceLibMeta {
    pub name: String,
    #[serde(rename = "type")]
    pub lib_type: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DockerSection {
    #[serde(rename = "baseImage", default)]
    pub base_image: String,
    #[serde(default)]
    pub workdir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<DockerWorkspaceSection>,
    /// Compose file path (default: compose.yml)
    #[serde(default = "default_compose_file")]
    pub compose: String,
    /// Deprecated: workspace container removed
    #[serde(default, skip_serializing)]
    pub service: String,
    /// Command routing rules (glob pattern → service/workdir)
    #[serde(default)]
    pub routes: Vec<DockerRoute>,
    /// Commands to shim (default: pnpm, npm, node, npx, bun, tsx, next, eslint, vitest, etc.)
    #[serde(default = "default_shim_commands")]
    pub shim_commands: Vec<String>,
}

pub(crate) fn default_compose_file() -> String {
    "compose.yml".to_string()
}

pub(crate) fn default_shim_commands() -> Vec<String> {
    vec![
        "pnpm".to_string(),
        "npm".to_string(),
        "node".to_string(),
        "npx".to_string(),
        "bun".to_string(),
        "tsx".to_string(),
        "next".to_string(),
        "eslint".to_string(),
        "vitest".to_string(),
        "tsc".to_string(),
        "turbo".to_string(),
    ]
}

/// Route configuration for Docker command execution
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DockerRoute {
    /// Glob pattern to match (e.g., "apps/*", "packages/*")
    pub glob: String,
    /// Service to execute in
    pub service: String,
    /// Working directory template (supports {match} placeholder)
    pub workdir: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DockerWorkspaceSection {
    pub service: String,
    #[serde(default)]
    pub volumes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct JustSection {
    pub output: String,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Environment variable validation section
/// Example:
/// ```toml
/// [env]
/// required = ["DATABASE_URL", "API_KEY"]
/// optional = ["SENTRY_DSN", "DEBUG"]
///
/// [env.validation.DATABASE_URL]
/// pattern = "^postgresql://"
/// description = "PostgreSQL connection string"
/// ```
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct EnvSection {
    /// Required environment variables (must be set)
    #[serde(default)]
    pub required: Vec<String>,
    /// Optional environment variables
    #[serde(default)]
    pub optional: Vec<String>,
    /// Validation rules for specific variables
    #[serde(default)]
    pub validation: IndexMap<String, EnvValidation>,
}

// ── Secret provider configuration ────────────────────────────

/// Secret provider configuration for injecting env vars into compose services.
///
/// ```toml
/// [secrets]
/// provider = "doppler"
///
/// [secrets.doppler]
/// project = "my-project"
/// config = "dev"
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SecretsSection {
    /// Provider name: "doppler", etc.
    pub provider: String,
    /// Doppler-specific configuration (required when provider == "doppler")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doppler: Option<DopplerSecretsConfig>,
}

/// Doppler provider configuration.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DopplerSecretsConfig {
    /// Doppler project name
    pub project: String,
    /// Doppler config name (e.g., "dev", "stg", "prd")
    pub config: String,
}

/// Validation rules for an environment variable
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvValidation {
    /// Regex pattern to validate the value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Example value (used in .env.example)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// TypeScript configuration for tsconfig generation by `airis gen`.
///
/// Controls generation of `tsconfig.base.json` (shared compilerOptions),
/// `tsconfig.json` (IDE paths), and per-package tsconfig.json files.
///
/// All fields are optional — smart defaults are derived from the Node version
/// and package framework. Users can override any field in manifest.toml:
///
/// ```toml
/// [typescript]
/// target = "ES2024"
/// lib = ["ES2024"]
/// types = ["node"]
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TypescriptSection {
    /// Override TS major version (auto-detected from catalog if omitted)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<u32>,
    /// ES target (e.g. "ES2024"). Default: auto-detected from Node version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Module system (e.g. "ESNext"). Default: "ESNext".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Module resolution strategy. Default: "bundler".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_resolution: Option<String>,
    /// Lib entries (e.g. ["ES2024"]). Default: [target].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lib: Option<Vec<String>>,
    /// Type packages to auto-include (e.g. ["node"]). Default: ["node"].
    /// TS6 changed the default from "all @types" to "none", so this is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    /// Extra compilerOptions merged into tsconfig.base.json
    #[serde(default)]
    pub compiler_options: IndexMap<String, toml::Value>,
    /// Extra path aliases merged into root tsconfig.json (IDE)
    #[serde(default)]
    pub paths: IndexMap<String, String>,
    /// Disable tsconfig generation (default: false)
    #[serde(default)]
    pub skip: bool,
    /// Generate per-package tsconfig.json files (default: true)
    #[serde(default = "default_true")]
    pub generate_per_package: bool,
}

impl Default for TypescriptSection {
    fn default() -> Self {
        Self {
            version: None,
            target: None,
            module: None,
            module_resolution: None,
            lib: None,
            types: None,
            compiler_options: IndexMap::new(),
            paths: IndexMap::new(),
            skip: false,
            generate_per_package: true,
        }
    }
}

/// Per-package tsconfig overrides in [[app]] definitions.
///
/// ```toml
/// [[app]]
/// name = "dashboard"
/// framework = "nextjs"
/// [app.tsconfig]
/// lib = ["ES2024", "DOM"]
/// ```
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackageTsconfigOverride {
    /// Override lib entries for this package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lib: Option<Vec<String>>,
    /// Override type packages for this package
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<String>>,
    /// JSX transform mode (e.g. "preserve", "react-jsx")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jsx: Option<String>,
    /// Additional compilerOptions for this package
    #[serde(default)]
    pub compiler_options: IndexMap<String, toml::Value>,
}

/// Runtime configuration for Docker builds
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuntimeConfig {
    /// Runtime mode: "channel" (default) or "exact"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// Channel: lts, current, edge, bun, deno
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Exact version (when mode="exact")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Kubernetes resource specifications
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ResourceSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// Kubernetes resource requests and limits
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct K8sResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ResourceSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<ResourceSpec>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct StackDefinition {
    /// Docker image (e.g., "node:22-bookworm", "nvidia/cuda:12.4-runtime-ubuntu22.04")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Artifact directories to isolate in named volumes
    #[serde(default)]
    pub artifacts: Vec<String>,
    /// Quality check commands for airis verify
    #[serde(default)]
    pub verify: Vec<String>,
    /// Whether this stack needs NVIDIA GPU access
    #[serde(default)]
    pub gpu: bool,
    /// Default scripts to inject into package.json
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
}

/// Project definition for package.json management.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct ProjectDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>, // "app" | "lib" | "service"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Reference to a [stack.*] definition
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub use_stack: Option<String>,
    /// Package name scope (e.g., "@agiletec"). Overrides default @workspace scope.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Package description for package.json
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// CLI entry points (e.g., { "akm" = "dist/cli.js" })
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub bin: IndexMap<String, String>,
    /// Main entry point (e.g., "dist/index.js")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    /// TypeScript declaration entry point (e.g., "dist/index.d.ts")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<String>,
    /// Package version (default: "0.1.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Whether the package is private (default: true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    /// Module type for package.json "type" field (default: "module")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_type: Option<String>,
    /// Package exports — free-form structure, converted to JSON as-is
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exports: Option<toml::Value>,
    /// peerDependencies
    #[serde(default)]
    pub peer_deps: IndexMap<String, String>,
    /// peerDependenciesMeta (e.g., optional markers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_deps_meta: Option<toml::Value>,
    /// Tags for package.json and turbo.tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Files to include in published package
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>, // "react-vite" | "nextjs" | "node" | "rust" | "python"
    /// Python version (e.g., "3.12")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<String>,
    /// CUDA version (e.g., "12.4")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cuda: Option<String>,
    /// Runtime configuration for Docker builds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner: Option<RuntimeConfig>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(default)]
    pub deps: IndexMap<String, String>,
    #[serde(default)]
    pub dev_deps: IndexMap<String, String>,
    /// References to dep_group names for DRY dependency grouping
    #[serde(default)]
    pub dep_groups: Vec<String>,
    /// References to dep_group names for DRY devDependency grouping
    #[serde(default)]
    pub dev_dep_groups: Vec<String>,
    /// Kubernetes: container port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Kubernetes: number of replicas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,
    /// Kubernetes: resource requests and limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<K8sResources>,
    /// Production Dockerfile generation config
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy: Option<AppDeployConfig>,

    // ── v2 fields ──
    /// Preset name(s) to inherit deps/scripts/deploy defaults from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<PresetRef>,
    /// Docker compose profiles this service belongs to (e.g., ["web", "api"])
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<String>>,
    /// Services this app depends on (e.g., ["steel-browser", "paddleocr"])
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    /// Memory limit for Docker container (e.g., "4g")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem_limit: Option<String>,
    /// CPU limit for Docker container
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<f32>,
    /// Inline service config (env, profile-specific overrides)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<ServiceInlineConfig>,
    /// Per-package tsconfig overrides
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tsconfig: Option<PackageTsconfigOverride>,
}

/// Preset reference: single string or array of strings
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum PresetRef {
    Single(String),
    Multiple(Vec<String>),
}

impl PresetRef {
    #[allow(dead_code)] // Used by preset.rs (v2 feature, not yet in main codepath)
    pub fn as_list(&self) -> Vec<&str> {
        match self {
            PresetRef::Single(s) => vec![s.as_str()],
            PresetRef::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

/// Inline service configuration within [[app]]
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServiceInlineConfig {
    /// Extra environment variables for the service
    #[serde(default)]
    pub env: IndexMap<String, String>,
    /// Profile-specific overrides (e.g., local vs stg vs prd)
    #[serde(default)]
    pub profile: IndexMap<String, ServiceProfileOverride>,
}

/// Per-profile service overrides
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServiceProfileOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
}

/// Configuration for auto-generating production Dockerfiles per service.
/// When `enabled = true`, `airis gen` generates `{path}/Dockerfile` using turbo prune.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AppDeployConfig {
    /// Enable Dockerfile generation for this app (default: false)
    #[serde(default)]
    pub enabled: bool,
    /// Dockerfile variant: "node" | "nextjs" | "worker" (default: inferred from framework)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Container port (EXPOSE + ENV PORT)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Entrypoint for CMD (e.g., "products/airis/agent/dist/index.js")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Health check path (e.g., "/healthz"). Derived from framework if not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
    /// Health check interval (e.g., "30s")
    #[serde(default = "default_health_interval")]
    pub health_interval: String,
    /// Build-time ARGs for Next.js NEXT_PUBLIC_* variables
    #[serde(default)]
    pub build_args: Vec<String>,
    /// Extra apk packages for native modules (e.g., ["python3", "make", "g++"])
    #[serde(default)]
    pub extra_apk: Vec<String>,
    /// Base Docker image override (e.g., "python:3.12-slim", "nvidia/cuda:12.4.1-runtime-ubuntu22.04")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_image: Option<String>,
    /// Deploy target: "docker" (self-hosted compose) or "worker" (Cloudflare Workers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deploy_target: Option<String>,
    /// Traefik Host rule template v2 (e.g., "{profile.domain}", "dashboard.{profile.domain}")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Traefik Host rule v1 compat (e.g., "${CORPORATE_DOMAIN}", "dashboard.${CORPORATE_DOMAIN}")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_rule: Option<String>,
    /// Runtime environment variables for deploy compose
    #[serde(default)]
    pub env: Vec<String>,
    /// Environment variable groups for deploy compose (references [env_group] names)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_groups: Vec<String>,
    /// Deploy job timeout in minutes. Default: 15 (docker), 10 (worker).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u8>,
    /// Health check retry count. Default: 3 (Dockerfile), 6 (CI)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_retries: Option<u8>,
    /// Health check retry interval in seconds. Default: 10
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_retry_interval: Option<u8>,
    /// Docker HEALTHCHECK --timeout value (e.g., "10s"). Default: "10s"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_timeout: Option<String>,
    /// Docker HEALTHCHECK --start-period value (e.g., "30s"). Default: "30s"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_start_period: Option<String>,
    /// Cloudflare Workers domain suffix (e.g., "myorg.workers.dev").
    /// Required when deploy_target = "worker".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workers_domain: Option<String>,
}

impl ProjectDefinition {
    /// Fill convention-based defaults into empty/None fields.
    /// Explicit manifest values always win.
    pub fn resolve(&mut self, workspace: &WorkspaceSection) {
        // name: derive from path
        if self.name.is_empty()
            && let Some(ref path) = self.path
        {
            self.name = crate::conventions::name_from_path(path).to_string();
        }

        // kind: derive from path
        if self.kind.is_none()
            && let Some(ref path) = self.path
            && path.starts_with("libs/")
        {
            self.kind = Some("lib".to_string());
        }

        // framework: default to "node" for libs
        if self.framework.is_none() && self.kind.as_deref() == Some("lib") {
            self.framework = Some("node".to_string());
        }

        // Now that kind/framework are resolved, get framework defaults
        let framework = self.framework.as_deref().unwrap_or("node");
        let defaults = crate::conventions::framework_defaults(framework);

        // scope: derive from workspace
        if self.scope.is_none() {
            self.scope = workspace.scope.clone();
        }

        // port: derive from framework
        if self.port.is_none() {
            self.port = Some(defaults.port);
        }

        // Lib-specific conventions
        if self.kind.as_deref() == Some("lib") {
            // main
            if self.main.is_none() {
                self.main = Some("./dist/index.js".to_string());
            }

            // exports
            if self.exports.is_none() {
                // Default: "." = { types = "./dist/index.d.ts", import = "./dist/index.js" }
                let mut export_map = toml::map::Map::new();
                let mut dot_export = toml::map::Map::new();
                dot_export.insert(
                    "types".to_string(),
                    toml::Value::String("./dist/index.d.ts".to_string()),
                );
                dot_export.insert(
                    "import".to_string(),
                    toml::Value::String("./dist/index.js".to_string()),
                );
                export_map.insert(".".to_string(), toml::Value::Table(dot_export));
                self.exports = Some(toml::Value::Table(export_map));
            }

            // default scripts
            if !self.scripts.contains_key("build") {
                self.scripts.insert(
                    "build".to_string(),
                    "tsup src/index.ts --format esm --dts --clean".to_string(),
                );
            }
            if !self.scripts.contains_key("typecheck") {
                self.scripts
                    .insert("typecheck".to_string(), "tsc --noEmit".to_string());
            }
        }

        // deploy defaults from framework
        if let Some(ref mut deploy) = self.deploy {
            if deploy.variant.is_none() {
                deploy.variant = Some(
                    match framework {
                        "nextjs" => "nextjs",
                        "cloudflare-worker" => "worker",
                        _ => "node",
                    }
                    .to_string(),
                );
            }
            if deploy.port.is_none() {
                deploy.port = Some(defaults.port);
            }
            if deploy.health_path.is_none() {
                deploy.health_path = Some(defaults.health_path.to_string());
            }
        }
    }

    /// Check if this app deploys via Cloudflare Workers (not Docker).
    #[allow(dead_code)]
    pub fn is_worker_deploy(&self) -> bool {
        self.deploy
            .as_ref()
            .map(|d| {
                d.deploy_target.as_deref() == Some("worker")
                    || d.variant.as_deref() == Some("worker")
            })
            .unwrap_or(false)
    }
}

fn default_health_interval() -> String {
    "30s".to_string()
}

/// Orchestration configuration for multi-compose setup
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct OrchestrationSection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev: Option<OrchestrationDev>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub networks: Option<NetworksConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NetworksConfig {
    /// External proxy network name (e.g., "coolify", "traefik-public")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
    /// Whether default network should be external
    #[serde(default)]
    pub default_external: bool,
    /// Additional named networks (e.g., proxy with external: true)
    #[serde(default)]
    pub define: IndexMap<String, NetworkDef>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NetworkDef {
    #[serde(default)]
    pub external: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrchestrationDev {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traefik: Option<String>,
    /// Global restart policy override for dev compose (e.g., "no").
    /// When set, all services in dev compose use this instead of
    /// their per-service `restart` value (which is for deploy).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
}

/// Version management configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VersioningSection {
    /// Version bump strategy
    #[serde(default = "default_versioning_strategy")]
    pub strategy: VersioningStrategy,
    /// Source of truth version (manually maintained or auto-updated)
    #[serde(default = "default_version_source")]
    pub source: String,
}

impl Default for VersioningSection {
    fn default() -> Self {
        VersioningSection {
            strategy: default_versioning_strategy(),
            source: default_version_source(),
        }
    }
}

fn default_versioning_strategy() -> VersioningStrategy {
    VersioningStrategy::Manual
}

fn default_version_source() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum VersioningStrategy {
    /// Manual version bumps only
    Manual,
    /// Auto-increment minor version on every commit
    Auto,
    /// Use Conventional Commits to determine bump type
    ConventionalCommits,
}

/// MCP Gateway configuration for this project
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct McpSection {
    /// MCP Gateway endpoint (e.g., "http://localhost:9400/sse")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,
    /// Active MCP server names for this project (e.g., ["context7", "supabase", "stripe"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DocsSection {
    /// List of documentation files to manage (e.g., ["CLAUDE.md", ".cursorrules"])
    #[serde(default)]
    pub targets: Vec<String>,
    /// Overwrite mode: "warn" (default) or "backup"
    #[serde(default = "default_docs_mode")]
    pub mode: DocsMode,
    /// Shared AI instruction files used as the source of truth.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Vendor adapters to generate from shared sources.
    #[serde(default)]
    pub vendors: Vec<DocsVendor>,
    /// Optional shared playbook directory for skills or task-specific docs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_source: Option<String>,
    /// Optional policy file that describes portable hook intent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks_policy: Option<String>,
}

impl Default for DocsSection {
    fn default() -> Self {
        DocsSection {
            targets: vec![],
            mode: default_docs_mode(),
            sources: vec![],
            vendors: vec![],
            skills_source: None,
            hooks_policy: None,
        }
    }
}

fn default_docs_mode() -> DocsMode {
    DocsMode::Warn
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DocsMode {
    /// Warn and refuse to overwrite existing files
    #[default]
    Warn,
    /// Create .bak backup before overwriting
    Backup,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DocsVendor {
    Codex,
    Claude,
    Gemini,
}

/// CI/CD configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CiSection {
    /// Enable CI workflow generation
    #[serde(default = "default_ci_enabled")]
    pub enabled: bool,
    /// Auto-merge from development branch to main
    #[serde(default)]
    pub auto_merge: AutoMergeConfig,
    /// Auto-versioning using Conventional Commits
    #[serde(default = "default_true")]
    pub auto_version: bool,
    /// GitHub repository owner/name (e.g., "agiletec-inc/my-project")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Homebrew tap repository (e.g., "agiletec-inc/homebrew-tap")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homebrew_tap: Option<String>,
    /// CI runner label (e.g., "self-hosted"). Default: "ubuntu-latest"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
    /// Node.js version (e.g., "24"). Default: "22"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_version: Option<String>,
    /// Use turbo --affected for incremental builds
    #[serde(default)]
    pub affected: bool,
    /// Enable concurrency cancel-in-progress
    #[serde(default = "default_true")]
    pub concurrency_cancel: bool,
    /// Enable GitHub Actions cache for pnpm. Disable for self-hosted runners
    /// that use persistent pnpm store volumes.
    #[serde(default = "default_true")]
    pub cache: bool,
    /// Path to persistent pnpm store (for self-hosted runners with volumes)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pnpm_store_path: Option<String>,
    /// Runner label for Cloudflare Workers deploy jobs. Default: "ubuntu-latest"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_runner: Option<String>,
    /// CI validate job timeout in minutes. Default: 5
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validate_timeout: Option<u8>,
    /// CI check job timeouts. Key = turbo task name, Value = timeout minutes.
    /// Default: {"lint": 10, "typecheck": 10, "test": 15}
    #[serde(default = "default_ci_jobs")]
    pub jobs: IndexMap<String, u8>,
    /// E2E staging workflow configuration
    #[serde(default)]
    pub e2e: E2eSection,
}

impl Default for CiSection {
    fn default() -> Self {
        CiSection {
            enabled: default_ci_enabled(),
            auto_merge: AutoMergeConfig::default(),
            auto_version: true,
            repository: None,
            homebrew_tap: None,
            runner: None,
            node_version: None,
            affected: false,
            concurrency_cancel: true,
            cache: true,
            pnpm_store_path: None,
            worker_runner: None,
            validate_timeout: None,
            jobs: default_ci_jobs(),
            e2e: E2eSection::default(),
        }
    }
}

fn default_ci_jobs() -> IndexMap<String, u8> {
    let mut m = IndexMap::new();
    m.insert("lint".into(), 10);
    m.insert("typecheck".into(), 10);
    m.insert("test".into(), 15);
    m
}

/// E2E staging workflow configuration
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct E2eSection {
    /// Enable E2E staging workflow generation
    #[serde(default)]
    pub enabled: bool,
    /// Timeout in minutes. Default: 15
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u8>,
    /// Default test filter for manual trigger. Default: "staging-smoke"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_filter: Option<String>,
    /// Trigger workflow name (workflow_run). Default: "Deploy"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_workflow: Option<String>,
}

fn default_ci_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AutoMergeConfig {
    /// Enable auto-merge
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Source branch (default: "stg")
    #[serde(default = "default_source_branch")]
    pub from: String,
    /// Target branch (default: "main")
    #[serde(default = "default_target_branch")]
    pub to: String,
}

impl Default for AutoMergeConfig {
    fn default() -> Self {
        AutoMergeConfig {
            enabled: true,
            from: default_source_branch(),
            to: default_target_branch(),
        }
    }
}

fn default_source_branch() -> String {
    "stg".to_string()
}

fn default_target_branch() -> String {
    "main".to_string()
}

// =============================================================================
// v2: Profile Section
// =============================================================================

/// Environment profile (local, stg, prd, etc.)
/// Each profile defines a deployment environment.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProfileSection {
    /// Branch that activates this profile (e.g., "stg", "main")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Environment variable source
    #[serde(default)]
    pub env_source: EnvSource,
    /// Base domain for services (e.g., "stg.agiletec.net")
    #[serde(default)]
    pub domain: String,
    /// NODE_ENV value
    #[serde(default = "default_node_env_dev")]
    pub node_env: String,
    /// Docker compose profiles to activate
    #[serde(default)]
    pub compose_profiles: Vec<String>,
    /// Inherit from another profile (override only what differs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits: Option<String>,
    /// Profile role: "production" | "staging" | "local".
    /// Overrides name-based inference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

impl ProfileSection {
    /// Resolve the effective role of this profile.
    /// Explicit `role` field takes priority; otherwise inferred from profile name.
    #[allow(dead_code)]
    pub fn effective_role(&self, name: &str) -> &str {
        if let Some(ref role) = self.role {
            return role.as_str();
        }
        match name {
            "prd" | "prod" | "production" => "production",
            "local" | "dev" | "development" => "local",
            _ => "staging",
        }
    }
}

fn default_node_env_dev() -> String {
    "development".to_string()
}

impl Default for ProfileSection {
    fn default() -> Self {
        ProfileSection {
            branch: None,
            env_source: EnvSource::default(),
            domain: "localhost".to_string(),
            node_env: default_node_env_dev(),
            compose_profiles: vec![],
            inherits: None,
            role: None,
        }
    }
}

/// How environment variables are sourced
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum EnvSource {
    /// Simple string: "dotenv"
    Simple(String),
    /// Doppler config: { doppler = { config = "stg", secret = "DOPPLER_TOKEN_STG" } }
    Doppler { doppler: DopplerConfig },
}

impl Default for EnvSource {
    fn default() -> Self {
        EnvSource::Simple("dotenv".to_string())
    }
}

impl EnvSource {
    /// Get Doppler config if available
    #[allow(dead_code)]
    pub fn doppler_config(&self) -> Option<&DopplerConfig> {
        match self {
            EnvSource::Doppler { doppler } => Some(doppler),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DopplerConfig {
    pub config: String,
    pub secret: String,
}

// =============================================================================
// v2: Preset Section
// =============================================================================

/// Reusable preset for app definitions.
/// When an app specifies `preset = "nextjs-app"`, the preset's deps, dev_deps,
/// scripts, and deploy defaults are merged (app values override preset values).
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PresetSection {
    /// Framework hint (e.g., "nextjs", "node")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub framework: Option<String>,
    /// Mark package as private
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    /// Default scripts
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    /// Default dependencies
    #[serde(default)]
    pub deps: IndexMap<String, String>,
    /// Default devDependencies
    #[serde(default)]
    pub dev_deps: IndexMap<String, String>,
    /// References to dep_group names for DRY dependency grouping
    #[serde(default)]
    pub dep_groups: Vec<String>,
    /// References to dep_group names for DRY devDependency grouping
    #[serde(default)]
    pub dev_dep_groups: Vec<String>,
    /// Default npm scope (e.g., "@agiletec")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Default deploy settings
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deploy: Option<PresetDeployDefaults>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PresetDeployDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_path: Option<String>,
}

// =============================================================================
// v2: External Service Config
// =============================================================================

/// Third-party service not built from source (e.g., steel-browser, paddleocr)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ExternalServiceConfig {
    pub image: String,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shm_size: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu: Option<bool>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
}

// =============================================================================
// v2: Root Section (replaces packages.root in v1)
// =============================================================================

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RootSection {
    #[serde(default)]
    pub engines: IndexMap<String, String>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(rename = "devDependencies", default)]
    pub dev_dependencies: IndexMap<String, String>,
}

// VersioningSection methods removed - using bump_version.rs instead

// =============================================================================
// Templates Section (for airis new)
// =============================================================================

/// Templates configuration for airis new
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TemplatesSection {
    /// API templates (e.g., hono, fastapi, rust-axum)
    #[serde(default)]
    pub api: IndexMap<String, TemplateConfig>,
    /// Web templates (e.g., nextjs, vite)
    #[serde(default)]
    pub web: IndexMap<String, TemplateConfig>,
    /// Worker templates (e.g., node-worker, rust-worker)
    #[serde(default)]
    pub worker: IndexMap<String, TemplateConfig>,
    /// CLI templates
    #[serde(default)]
    pub cli: IndexMap<String, TemplateConfig>,
    /// Library templates
    #[serde(default)]
    pub lib: IndexMap<String, TemplateConfig>,
    /// Supabase Edge Function templates
    #[serde(default)]
    pub edge: IndexMap<String, TemplateConfig>,
    /// Supabase trigger templates
    #[serde(rename = "supabase-trigger", default)]
    pub supabase_trigger: IndexMap<String, TemplateConfig>,
    /// Supabase realtime templates
    #[serde(rename = "supabase-realtime", default)]
    pub supabase_realtime: IndexMap<String, TemplateConfig>,
}

/// Template configuration
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct TemplateConfig {
    /// Entry point file (e.g., "src/index.ts", "src/main.rs")
    #[serde(default)]
    pub entry: String,
    /// Dockerfile template path
    #[serde(default)]
    pub dockerfile: String,
    /// Runtime/language identifier
    #[serde(default)]
    pub runtime: String,
    /// Dependencies to inject
    #[serde(default)]
    pub deps: Vec<String>,
    /// Dev dependencies to inject
    #[serde(default)]
    pub dev_deps: Vec<String>,
    /// Features/modules to inject into the template
    #[serde(default)]
    pub inject: Vec<String>,
    /// Package manager config file (package.json, pyproject.toml, Cargo.toml)
    #[serde(default)]
    pub package_config: String,
}

/// Runtime declarations.
///
/// Two responsibilities live here:
/// 1. `alias` — short aliases consumed by `airis new` (e.g., "py" -> "fastapi").
/// 2. `node` / `python` / `rust` — declarative runtime versions consumed by
///    workspace Dockerfile generation and `airis exec` cmd→service routing
///    (Phase 1 onward; see docs/ai/IDEAL_STATE.md §2 and the eager-floating-book plan).
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuntimesSection {
    /// Short aliases for `airis new` templates (e.g., "py" -> "fastapi", "ts" -> "hono")
    #[serde(default)]
    pub alias: IndexMap<String, String>,
    /// Node runtime to provision inside the workspace container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<RuntimeSpec>,
    /// Python runtime to provision inside the workspace container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python: Option<RuntimeSpec>,
    /// Rust toolchain to provision inside the workspace container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rust: Option<RuntimeSpec>,
}

/// A runtime declaration. Accepts either a bare version string or a detailed table.
///
/// ```toml
/// [runtimes]
/// node = "24"                     # short form
///
/// [runtimes.python]
/// version = "3.13"
/// package_manager = "uv"
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum RuntimeSpec {
    /// Bare version string, e.g., `node = "24"`
    Short(String),
    /// Detailed table with version + optional image / package_manager / components
    Detailed(RuntimeDetail),
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuntimeDetail {
    pub version: String,
    /// Override the resolved base image (e.g., `python:3.13-slim`). Default: derived from version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Package manager hint for ecosystems with multiple options (Python: uv|pip|poetry).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
    /// Toolchain components (e.g., Rust: ["clippy", "rustfmt"]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub toolchain_components: Vec<String>,
}

impl RuntimeSpec {
    /// Returns the version string for this runtime spec.
    pub fn version(&self) -> &str {
        match self {
            RuntimeSpec::Short(v) => v.as_str(),
            RuntimeSpec::Detailed(d) => d.version.as_str(),
        }
    }

    /// Returns the explicitly overridden image, if any.
    pub fn image_override(&self) -> Option<&str> {
        match self {
            RuntimeSpec::Short(_) => None,
            RuntimeSpec::Detailed(d) => d.image.as_deref(),
        }
    }

    /// Returns the explicit package manager hint, if any.
    pub fn package_manager(&self) -> Option<&str> {
        match self {
            RuntimeSpec::Short(_) => None,
            RuntimeSpec::Detailed(d) => d.package_manager.as_deref(),
        }
    }

    /// Returns toolchain components (empty for `Short`).
    pub fn toolchain_components(&self) -> &[String] {
        match self {
            RuntimeSpec::Short(_) => &[],
            RuntimeSpec::Detailed(d) => &d.toolchain_components,
        }
    }
}

// =============================================================================
// Testing Governance
// =============================================================================

/// Mock policy for external service dependencies
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum MockPolicy {
    /// Mocks allowed everywhere
    Allowed,
    /// Mocks of external services forbidden everywhere (default when [testing] is present)
    #[default]
    Forbidden,
    /// Mocks allowed in unit tests only, forbidden in integration/e2e
    UnitOnly,
}

/// Testing governance — declares test strategy, mock policy, and AI rules.
/// Feeds into CLAUDE.md/AGENTS.md generation and (future) CI/hook enforcement.
#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
pub struct TestingSection {
    /// Global mock policy
    #[serde(default)]
    pub mock_policy: MockPolicy,

    /// Coverage thresholds per test level (0 = no threshold)
    #[serde(default)]
    pub coverage: TestingCoverage,

    /// Which test levels are enabled
    #[serde(default)]
    pub levels: TestingLevels,

    /// Regex patterns that indicate forbidden mocking in integration/e2e tests
    #[serde(default)]
    pub forbidden_patterns: Vec<String>,

    /// Type enforcement: require generated types in DB-touching tests
    #[serde(default)]
    pub type_enforcement: Option<TypeEnforcement>,

    /// Custom rules injected into AI assistant docs (CLAUDE.md, AGENTS.md)
    #[serde(default)]
    pub ai_rules: Vec<String>,

    /// Smoke test definitions (run post-deploy)
    #[serde(default)]
    pub smoke: Vec<SmokeTest>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
pub struct TestingCoverage {
    #[serde(default)]
    pub unit: u8,
    #[serde(default)]
    pub integration: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct TestingLevels {
    #[serde(default = "default_true")]
    pub unit: bool,
    #[serde(default)]
    pub integration: bool,
    #[serde(default)]
    pub e2e: bool,
    #[serde(default)]
    pub smoke: bool,
}

impl Default for TestingLevels {
    fn default() -> Self {
        Self {
            unit: true,
            integration: false,
            e2e: false,
            smoke: false,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default, PartialEq)]
pub struct TypeEnforcement {
    /// Path to generated types file (e.g., "libs/database/src/types.ts")
    #[serde(default)]
    pub generated_types_path: String,
    /// Import patterns that must appear in DB-touching tests
    #[serde(default)]
    pub required_imports: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct SmokeTest {
    pub name: String,
    pub command: String,
    #[serde(default = "default_smoke_timeout")]
    pub timeout: u16,
}

fn default_smoke_timeout() -> u16 {
    30
}

// =============================================================================
// Policy Section
// =============================================================================

/// Code governance policy — SSoT for all quality, security, and workflow rules.
/// Absorbs [testing] and replaces .airis/policies.toml.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PolicySection {
    /// Testing governance (migrated from top-level [testing])
    #[serde(default)]
    pub testing: TestingSection,

    /// Security policy — banned env vars, secret scanning
    #[serde(default)]
    pub security: SecurityPolicy,
}

impl Manifest {
    /// Check if the manifest contains explicit orchestration or application configuration
    /// that warrants generating a compose.yaml file.
    pub fn has_orchestration_config(&self) -> bool {
        !self.app.is_empty()
            || self.orchestration.dev.is_some()
            || self.docker.workspace.is_some()
            || !self.service.is_empty()
    }
}

/// Security policy for source code governance.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SecurityPolicy {
    /// Environment variable name patterns banned from source code
    #[serde(default)]
    pub banned_env_vars: Vec<String>,

    /// Paths where banned_env_vars are allowed (server-side code).
    /// Glob patterns (e.g., "supabase/functions/", "products/*/worker/").
    #[serde(default)]
    pub allowed_paths: Vec<String>,

    /// Scan for hardcoded secrets in source files
    #[serde(default)]
    pub scan_secrets: bool,

    /// Max file size in MB for scanning (default: 50)
    #[serde(default = "default_security_max_file_size")]
    pub max_file_size_mb: u64,
}

fn default_security_max_file_size() -> u64 {
    50
}
