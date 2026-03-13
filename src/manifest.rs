use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Workspace mode (docker-first, hybrid, strict)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    #[default]
    DockerFirst,
    Hybrid,
    Strict,
}

pub const MANIFEST_FILE: &str = "manifest.toml";

fn default_version() -> u32 {
    1
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Manifest {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub mode: Mode,
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
    pub dev: DevSection,
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
}

impl Manifest {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", path.as_ref()))?;

        let manifest: Manifest =
            toml::from_str(&content).with_context(|| "Failed to parse manifest.toml")?;

        manifest.validate()?;

        Ok(manifest)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize manifest.toml contents")?;

        fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write {:?}", path.as_ref()))?;

        Ok(())
    }

    /// Validate manifest consistency.
    ///
    /// Checks:
    /// 1. No duplicate ports across service entries
    /// 2. Catalog follow references point to existing catalog keys
    /// 3. No command appears in both guards.deny and guards.wrap
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();

        // 1. Check for duplicate ports in service entries
        {
            let mut seen: std::collections::HashMap<u16, String> = std::collections::HashMap::new();
            for (name, svc) in &self.service {
                if let Some(port) = svc.port {
                    if let Some(prev) = seen.get(&port) {
                        errors.push(format!(
                            "Duplicate port {port}: services \"{prev}\" and \"{name}\" both bind to port {port}"
                        ));
                    } else {
                        seen.insert(port, name.clone());
                    }
                }
            }
        }

        // 2. Validate catalog follow references
        for (key, entry) in &self.packages.catalog {
            if let CatalogEntry::Follow(f) = entry {
                if !self.packages.catalog.contains_key(&f.follow) {
                    errors.push(format!(
                        "Catalog entry \"{key}\" follows \"{}\", which does not exist in packages.catalog",
                        f.follow
                    ));
                }
            }
        }

        // 3. Check for commands in both guards.deny and guards.wrap
        for cmd in &self.guards.deny {
            if self.guards.wrap.contains_key(cmd) {
                errors.push(format!(
                    "Guard conflict: \"{cmd}\" appears in both guards.deny and guards.wrap"
                ));
            }
        }

        if !errors.is_empty() {
            bail!("Manifest validation failed:\n{}", errors.join("\n"));
        }

        Ok(())
    }

    /// Collect all workspace directory paths from apps, libs, and packages.workspaces globs.
    ///
    /// Returns paths like "apps/corporate", "libs/ui", "products/my-app" etc.
    /// Uses the `path` field if set, otherwise defaults to "apps/{key}" or "libs/{key}".
    /// Also expands glob patterns from `packages.workspaces` (e.g. "products/*")
    /// relative to the given root directory.
    #[allow(dead_code)]
    pub fn all_workspace_paths(&self) -> Vec<String> {
        self.all_workspace_paths_in(".")
    }

    /// Like `all_workspace_paths` but resolves glob patterns relative to a specific root.
    pub fn all_workspace_paths_in(&self, root: &str) -> Vec<String> {
        let mut paths = Vec::new();

        for (key, app) in &self.apps {
            let path = app
                .path
                .clone()
                .unwrap_or_else(|| format!("apps/{}", key));
            paths.push(path);
        }

        for (key, lib) in &self.libs {
            let path = lib
                .path
                .clone()
                .unwrap_or_else(|| format!("libs/{}", key));
            paths.push(path);
        }

        // Expand glob patterns from packages.workspaces (e.g. "products/*", "packages/*")
        let root_path = Path::new(root);
        for pattern in &self.packages.workspaces {
            if pattern.starts_with('!') {
                continue; // skip exclude patterns
            }
            let full_pattern = root_path.join(pattern).to_string_lossy().to_string();
            if let Ok(entries) = glob::glob(&full_pattern) {
                for entry in entries.flatten() {
                    // Skip paths inside node_modules (transitive deps, not workspaces)
                    if entry.components().any(|c| c.as_os_str() == "node_modules") {
                        continue;
                    }
                    if entry.is_dir() && entry.join("package.json").exists() {
                        // Strip root prefix to get relative path
                        let p = entry
                            .strip_prefix(root_path)
                            .unwrap_or(&entry)
                            .to_string_lossy()
                            .to_string();
                        if !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
            }
        }

        paths
    }

    /// Create a default manifest with project name
    /// NOTE: This is kept as reference for MCP agent's manifest generation
    #[allow(dead_code)]
    pub fn default_with_project(name: &str) -> Self {
        // Rule definitions
        let mut rule = IndexMap::new();
        rule.insert(
            "verify".to_string(),
            RuleConfig {
                commands: vec!["airis lint".to_string(), "airis test".to_string()],
            },
        );
        rule.insert(
            "ci".to_string(),
            RuleConfig {
                commands: vec![
                    "airis lint".to_string(),
                    "airis test".to_string(),
                    "airis build".to_string(),
                ],
            },
        );

        // Catalog with common TypeScript/React dependencies
        let mut catalog = IndexMap::new();
        catalog.insert("react".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("react-dom".to_string(), CatalogEntry::Follow(FollowConfig { follow: "react".to_string() }));
        catalog.insert("next".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("typescript".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("tailwindcss".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("zod".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("vitest".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("eslint".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));
        catalog.insert("prettier".to_string(), CatalogEntry::Policy(VersionPolicy::Latest));

        // Packages section
        let packages = PackagesSection {
            workspaces: vec!["apps/*".to_string(), "libs/*".to_string(), "packages/*".to_string()],
            catalog,
            root: PackageDefinition {
                dependencies: IndexMap::new(),
                dev_dependencies: IndexMap::new(),
                optional_dependencies: IndexMap::new(),
                scripts: {
                    let mut scripts = IndexMap::new();
                    scripts.insert("dev".to_string(), "echo 'Run: airis dev'".to_string());
                    scripts.insert("build".to_string(), "echo 'Run: airis build'".to_string());
                    scripts.insert("lint".to_string(), "echo 'Run: airis lint'".to_string());
                    scripts.insert("test".to_string(), "echo 'Run: airis test'".to_string());
                    scripts
                },
                engines: IndexMap::new(),
                pnpm: PnpmConfig::default(),
            },
            app: vec![],
        };

        // Guards for Docker-first enforcement
        let guards = GuardsSection {
            deny: vec![
                "npm".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "bun".to_string(),
            ],
            wrap: IndexMap::new(),
            deny_with_message: IndexMap::new(),
            forbid: vec![
                "npm".to_string(),
                "yarn".to_string(),
                "pnpm".to_string(),
                "docker".to_string(),
                "docker-compose".to_string(),
            ],
            danger: vec![
                "rm -rf /".to_string(),
                "chmod -R 777".to_string(),
            ],
        };

        // Remap common commands to airis
        let mut remap = IndexMap::new();
        remap.insert("npm install".to_string(), "airis up".to_string());
        remap.insert("pnpm install".to_string(), "airis up".to_string());
        remap.insert("yarn install".to_string(), "airis up".to_string());
        remap.insert("npm run dev".to_string(), "airis up".to_string());
        remap.insert("pnpm dev".to_string(), "airis up".to_string());
        remap.insert("docker compose up".to_string(), "airis up".to_string());
        remap.insert("docker compose down".to_string(), "airis down".to_string());

        Manifest {
            version: 1,
            mode: Mode::DockerFirst,
            project: MetaSection {
                id: name.to_string(),
                binary_name: String::new(),
                version: "1.0.0".to_string(),
                description: format!("{} - Docker-first monorepo", name),
                authors: vec![],
                license: "MIT".to_string(),
                homepage: String::new(),
                repository: String::new(),
                keywords: vec!["monorepo".to_string(), "docker".to_string(), "typescript".to_string()],
                categories: vec!["development-tools".to_string()],
                rust_edition: String::new(),
            },
            workspace: WorkspaceSection {
                name: format!("airis-{}", name),  // Prefix to avoid Docker name collisions
                package_manager: "pnpm@10.22.0".to_string(),
                service: String::new(),
                image: "node:22-alpine".to_string(),
                workdir: "/app".to_string(),
                volumes: vec![format!("{}-node-modules:/app/node_modules", name)],
                clean: CleanSection::default(),
            },
            catalog: IndexMap::new(),
            workspaces: WorkspacesSection::default(),
            dev: DevSection::default(),
            apps: IndexMap::new(),
            libs: IndexMap::new(),
            docker: DockerSection {
                base_image: "node:22-alpine".to_string(),
                workdir: "/app".to_string(),
                workspace: Some(DockerWorkspaceSection {
                    service: "workspace".to_string(),
                    volumes: vec!["node_modules".to_string()],
                }),
                compose: default_compose_file(),
                service: String::new(),
                routes: vec![],
                shim_commands: default_shim_commands(),
            },
            just: None,
            service: IndexMap::new(),
            rule,
            packages,
            guards,
            app: vec![],
            orchestration: OrchestrationSection::default(),
            commands: {
                let mut cmds = IndexMap::new();
                cmds.insert("up".to_string(), "docker compose up -d --build --remove-orphans".to_string());
                cmds.insert("down".to_string(), "docker compose down --remove-orphans".to_string());
                cmds.insert("ps".to_string(), "docker compose ps".to_string());
                cmds
            },
            remap,
            versioning: VersioningSection {
                strategy: VersioningStrategy::Manual,
                source: "1.0.0".to_string(),
            },
            docs: DocsSection::default(),
            ci: CiSection::default(),
            templates: TemplatesSection::default(),
            runtimes: RuntimesSection::default(),
            env: EnvSection::default(),
        }
    }
}

/// Project metadata - Source of Truth for Cargo.toml, Homebrew formula, etc.
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct MetaSection {
    /// Project ID (e.g., "airis-monorepo")
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
    #[serde(default = "default_package_manager")]
    pub package_manager: String,
    /// Deprecated: workspace container has been removed. Kept for backwards compatibility with existing manifest.toml files.
    #[serde(default, skip_serializing)]
    pub service: String,
    #[serde(default = "default_workspace_image")]
    pub image: String,
    #[serde(default = "default_workspace_workdir")]
    pub workdir: String,
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
        "pnpm-lock.yaml".to_string(),
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
            package_manager: default_package_manager(),
            service: String::new(),
            image: default_workspace_image(),
            workdir: default_workspace_workdir(),
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
            && let Ok(path_str) = String::from_utf8(output.stdout) {
                let path = std::path::Path::new(path_str.trim());
                if let Some(name) = path.file_name()
                    && let Some(name_str) = name.to_str() {
                        return name_str.to_string();
                    }
            }

    // Fallback: use current directory name
    if let Ok(cwd) = std::env::current_dir()
        && let Some(name) = cwd.file_name()
            && let Some(name_str) = name.to_str() {
                return name_str.to_string();
            }

    "workspace".to_string()
}

fn default_package_manager() -> String {
    "pnpm@10.22.0".to_string()
}

fn default_workspace_image() -> String {
    "node:22-alpine".to_string()
}

fn default_workspace_workdir() -> String {
    "/app".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DevSection {
    /// Glob pattern for auto-discovering app docker-compose files
    /// Default: "apps/*/docker-compose.yml"
    #[serde(default = "default_apps_pattern")]
    pub apps_pattern: String,
    /// Supabase compose files (e.g., ["supabase/docker-compose.yml"])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase: Option<Vec<String>>,
    /// Traefik compose file (e.g., "traefik/docker-compose.yml")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traefik: Option<String>,
    /// URLs to display after `airis up` (optional, dynamic from apps if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urls: Option<DevUrls>,
    /// Commands to run after `airis up` (e.g., DB migration)
    #[serde(default)]
    pub post_up: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct DevUrls {
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

impl Default for DevSection {
    fn default() -> Self {
        DevSection {
            apps_pattern: default_apps_pattern(),
            supabase: None,
            traefik: None,
            urls: None,
            post_up: Vec::new(),
        }
    }
}

fn default_apps_pattern() -> String {
    "apps/*/docker-compose.yml".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct AppConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub app_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct LibConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct ServiceConfig {
    pub image: String,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DeployConfig {
    pub replicas: Option<u32>,
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

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PackagesSection {
    #[serde(default)]
    pub workspaces: Vec<String>,
    #[serde(default)]
    pub catalog: IndexMap<String, CatalogEntry>,
    #[serde(default)]
    pub root: PackageDefinition,
    #[serde(rename = "app", default)]
    pub app: Vec<AppPackageDefinition>,
}

/// Catalog entry can be:
/// - "latest" → resolve to latest npm version
/// - "lts" → resolve to LTS version
/// - "^5.0.0" → specific semver (used as-is)
/// - { follow = "react" } → follow another package's version
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CatalogEntry {
    Follow(FollowConfig),
    Policy(VersionPolicy),
    Version(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowConfig {
    pub follow: String,
}

impl Default for CatalogEntry {
    fn default() -> Self {
        CatalogEntry::Version("*".to_string())
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
    /// Compose file path (default: docker-compose.yml)
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

fn default_compose_file() -> String {
    "docker-compose.yml".to_string()
}

fn default_shim_commands() -> Vec<String> {
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

/// Project definition for full package.json generation
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProjectDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,  // "app" | "lib" | "service"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
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
    pub framework: Option<String>,  // "react-vite" | "nextjs" | "node" | "rust"
    /// Runtime configuration for Docker builds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner: Option<RuntimeConfig>,
    #[serde(default)]
    pub scripts: IndexMap<String, String>,
    #[serde(default)]
    pub deps: IndexMap<String, String>,
    #[serde(default)]
    pub dev_deps: IndexMap<String, String>,
    /// Kubernetes: container port
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Kubernetes: number of replicas
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,
    /// Kubernetes: resource requests and limits
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<K8sResources>,
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
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrchestrationDev {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supabase: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traefik: Option<String>,
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

/// Documentation management configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DocsSection {
    /// List of documentation files to manage (e.g., ["CLAUDE.md", ".cursorrules"])
    #[serde(default)]
    pub targets: Vec<String>,
    /// Overwrite mode: "warn" (default) or "backup"
    #[serde(default = "default_docs_mode")]
    pub mode: DocsMode,
}

impl Default for DocsSection {
    fn default() -> Self {
        DocsSection {
            targets: vec![],
            mode: default_docs_mode(),
        }
    }
}

fn default_docs_mode() -> DocsMode {
    DocsMode::Warn
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DocsMode {
    /// Warn and refuse to overwrite existing files
    Warn,
    /// Create .bak backup before overwriting
    Backup,
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
        }
    }
}

fn default_ci_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AutoMergeConfig {
    /// Enable auto-merge
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Source branch (default: "next")
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
    "next".to_string()
}

fn default_target_branch() -> String {
    "main".to_string()
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
#[derive(Debug, Deserialize, Serialize, Clone)]
#[derive(Default)]
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


/// Runtime aliases configuration
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct RuntimesSection {
    /// Short aliases for runtimes (e.g., "py" -> "fastapi", "ts" -> "hono")
    #[serde(default)]
    pub alias: IndexMap<String, String>,
}

// =============================================================================
// Global Configuration (~/.airis/global-config.toml)
// =============================================================================

/// Global guards section for ~/.airis/global-config.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalGuardsSection {
    /// Commands to block outside of airis projects
    #[serde(default = "default_global_deny")]
    pub deny: Vec<String>,
}

impl Default for GlobalGuardsSection {
    fn default() -> Self {
        GlobalGuardsSection {
            deny: default_global_deny(),
        }
    }
}

fn default_global_deny() -> Vec<String> {
    vec![
        "npm".to_string(),
        "yarn".to_string(),
        "pnpm".to_string(),
        "bun".to_string(),
        "npx".to_string(),
    ]
}

/// Global configuration stored in ~/.airis/global-config.toml
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GlobalConfig {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub guards: GlobalGuardsSection,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            version: 1,
            guards: GlobalGuardsSection::default(),
        }
    }
}

impl GlobalConfig {
    /// Get the path to the global config file (~/.airis/global-config.toml)
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("global-config.toml"))
    }

    /// Get the path to the global bin directory (~/.airis/bin)
    pub fn bin_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home.join(".airis").join("bin"))
    }

    /// Load global config from ~/.airis/global-config.toml
    /// Returns default config if file doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {:?}", config_path))?;

        let config: GlobalConfig = toml::from_str(&content)
            .with_context(|| "Failed to parse global-config.toml")?;

        Ok(config)
    }

    /// Save global config to ~/.airis/global-config.toml
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directory if needed
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {:?}", parent))?;
        }

        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize global-config.toml")?;

        fs::write(&config_path, content)
            .with_context(|| format!("Failed to write {:?}", config_path))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: create a minimal valid manifest TOML string
    fn minimal_manifest() -> String {
        r#"
version = 1
"#
        .to_string()
    }

    /// Helper: write a manifest string to a temp file and load it
    fn load_from_str(content: &str) -> Result<Manifest> {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(content.as_bytes()).unwrap();
        Manifest::load(tmp.path())
    }

    #[test]
    fn test_validate_passes_for_minimal_manifest() {
        let manifest = load_from_str(&minimal_manifest());
        assert!(manifest.is_ok());
    }

    #[test]
    fn test_validate_duplicate_ports() {
        let toml = r#"
version = 1

[service.redis]
image = "redis:7"
port = 6379

[service.cache]
image = "redis:7"
port = 6379
"#;
        let err = load_from_str(toml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Duplicate port 6379"), "got: {msg}");
        assert!(msg.contains("redis"), "got: {msg}");
        assert!(msg.contains("cache"), "got: {msg}");
    }

    #[test]
    fn test_validate_no_duplicate_when_ports_differ() {
        let toml = r#"
version = 1

[service.redis]
image = "redis:7"
port = 6379

[service.postgres]
image = "postgres:16"
port = 5432
"#;
        assert!(load_from_str(toml).is_ok());
    }

    #[test]
    fn test_validate_skip_none_ports() {
        let toml = r#"
version = 1

[service.redis]
image = "redis:7"

[service.cache]
image = "redis:7"
"#;
        assert!(load_from_str(toml).is_ok());
    }

    #[test]
    fn test_validate_catalog_follow_missing_reference() {
        let toml = r#"
version = 1

[packages.catalog]
react = "latest"

[packages.catalog.react-dom]
follow = "nonexistent"
"#;
        let err = load_from_str(toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("react-dom") && msg.contains("nonexistent"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_validate_catalog_follow_valid_reference() {
        let toml = r#"
version = 1

[packages.catalog]
react = "latest"

[packages.catalog.react-dom]
follow = "react"
"#;
        assert!(load_from_str(toml).is_ok());
    }

    #[test]
    fn test_validate_guard_deny_wrap_conflict() {
        let toml = r#"
version = 1

[guards]
deny = ["pnpm"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"
"#;
        let err = load_from_str(toml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("pnpm") && msg.contains("guards.deny") && msg.contains("guards.wrap"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_validate_guard_no_conflict() {
        let toml = r#"
version = 1

[guards]
deny = ["npm", "yarn"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"
"#;
        assert!(load_from_str(toml).is_ok());
    }

    #[test]
    fn test_validate_multiple_errors_collected() {
        let toml = r#"
version = 1

[service.a]
image = "redis:7"
port = 6379

[service.b]
image = "redis:7"
port = 6379

[packages.catalog.react-dom]
follow = "missing"

[guards]
deny = ["pnpm"]

[guards.wrap]
pnpm = "docker compose exec workspace pnpm"
"#;
        let err = load_from_str(toml).unwrap_err();
        let msg = err.to_string();
        // All three errors should be present
        assert!(msg.contains("Duplicate port"), "got: {msg}");
        assert!(msg.contains("missing"), "got: {msg}");
        assert!(msg.contains("Guard conflict"), "got: {msg}");
    }
}
