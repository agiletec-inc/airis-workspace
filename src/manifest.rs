use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

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
    /// Value injection into user-owned files via `# airis:inject <key>` markers
    #[serde(default)]
    pub inject: IndexMap<String, InjectValue>,
    /// TypeScript configuration for tsconfig generation
    #[serde(default)]
    pub typescript: TypescriptSection,

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
}

impl Manifest {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", path.as_ref()))?;

        let mut manifest: Manifest =
            toml::from_str(&content).with_context(|| "Failed to parse manifest.toml")?;

        manifest.validate()?;
        manifest.resolve_conventions();

        // Post-resolve validation
        for (i, app) in manifest.app.iter().enumerate() {
            if app.name.is_empty() {
                bail!(
                    "[[app]] entry #{} has empty name and no path to derive from",
                    i + 1
                );
            }
        }

        Ok(manifest)
    }

    /// Apply convention-based defaults to all [[app]] entries.
    fn resolve_conventions(&mut self) {
        let workspace = self.workspace.clone();
        for app in &mut self.app {
            app.resolve(&workspace);
        }
    }

    /// Returns true if this manifest defines a Node.js workspace.
    /// Determined by whether package_manager is explicitly set in [workspace].
    pub fn has_workspace(&self) -> bool {
        !self.workspace.package_manager.is_empty()
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

        // 2. Validate catalog follow references (skip if default_policy can resolve the target)
        for (key, entry) in &self.packages.catalog {
            if let CatalogEntry::Follow(f) = entry
                && !self.packages.catalog.contains_key(&f.follow)
                && self.packages.default_policy.is_none() {
                    errors.push(format!(
                        "Catalog entry \"{key}\" follows \"{}\", which does not exist in packages.catalog (add it or set default_policy)",
                        f.follow
                    ));
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

    /// Get the effective Node.js version.
    /// Priority: [workspace].node > [ci].node_version > extracted from [workspace].image > "22"
    pub fn node_version(&self) -> String {
        // v2: explicit node field
        if let Some(ref v) = self.workspace.node {
            return v.clone();
        }
        // ci.node_version override
        if let Some(ref v) = self.ci.node_version {
            return v.clone();
        }
        // Extract from image string like "node:24-bookworm"
        let image = &self.workspace.image;
        if image.starts_with("node:")
            && let Some(version_part) = image.strip_prefix("node:") {
                let version = version_part.split('-').next().unwrap_or("22");
                return version.to_string();
            }
        "22".to_string()
    }

    /// Get deploy profiles from [profile] section.
    /// Returns profiles that have a branch (i.e., are deploy targets).
    pub fn deploy_profiles(&self) -> Vec<(&str, &ProfileSection)> {
        self.profile
            .iter()
            .filter(|(_, p)| p.branch.is_some())
            .map(|(name, p)| (name.as_str(), p))
            .collect()
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
                    // Skip paths inside node_modules or build artifact directories
                    // (transitive deps and generated files are not workspaces)
                    let skip_dirs = ["node_modules", ".next", "dist", ".turbo", ".swc", "build", "out"];
                    if entry.components().any(|c| {
                        skip_dirs.iter().any(|d| c.as_os_str() == *d)
                    }) {
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
            default_policy: None,
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
                scope: None,
                package_manager: "pnpm@10.33.0".to_string(),
                node: None,
                service: String::new(),
                image: crate::channel::defaults::NODE_LTS_IMAGE.to_string(),
                workdir: "/app".to_string(),
                workspaces: vec![],
                volumes: vec![format!("{}-node-modules:/app/node_modules", name)],
                clean: CleanSection::default(),
            },
            catalog: IndexMap::new(),
            workspaces: WorkspacesSection::default(),
            dev: HooksSection::default(),
            apps: IndexMap::new(),
            libs: IndexMap::new(),
            docker: DockerSection {
                base_image: crate::channel::defaults::NODE_LTS_IMAGE.to_string(),
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
            hooks: PreCommandHooks::default(),
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
            inject: IndexMap::new(),
            typescript: TypescriptSection::default(),
            profile: IndexMap::new(),
            dep_group: IndexMap::new(),
            env_group: IndexMap::new(),
            preset: IndexMap::new(),
            external: IndexMap::new(),
            root: None,
            overrides: IndexMap::new(),
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
        }
    }
}

fn default_apps_pattern() -> String {
    "apps/*/compose.yml".to_string()
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

fn default_compose_file() -> String {
    "compose.yml".to_string()
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

/// Project definition for package.json management.
/// In hybrid mode, airis manages only name/version/private/type.
#[derive(Debug, Default, Deserialize, Serialize, Clone)]
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
            && let Some(ref path) = self.path {
                self.name = crate::conventions::name_from_path(path).to_string();
            }

        // kind: derive from path
        if self.kind.is_none()
            && let Some(ref path) = self.path
                && path.starts_with("libs/") {
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
                dot_export.insert("types".to_string(), toml::Value::String("./dist/index.d.ts".to_string()));
                dot_export.insert("import".to_string(), toml::Value::String("./dist/index.js".to_string()));
                export_map.insert(".".to_string(), toml::Value::Table(dot_export));
                self.exports = Some(toml::Value::Table(export_map));
            }

            // default scripts
            if !self.scripts.contains_key("build") {
                self.scripts.insert("build".to_string(), "tsup src/index.ts --format esm --dts --clean".to_string());
            }
            if !self.scripts.contains_key("typecheck") {
                self.scripts.insert("typecheck".to_string(), "tsc --noEmit".to_string());
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
    /// Runner label for Cloudflare Workers deploy jobs. Default: "ubuntu-latest"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker_runner: Option<String>,
    /// GitHub Actions versions (checkout, pnpm, setup-node, cache)
    #[serde(default)]
    pub actions: ActionsVersions,
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
            actions: ActionsVersions::default(),
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
#[derive(Debug, Deserialize, Serialize, Clone)]
#[derive(Default)]
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


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ActionsVersions {
    /// actions/checkout version. Default: "v6"
    #[serde(default = "default_v6")]
    pub checkout: String,
    /// pnpm/action-setup version. Default: "v5"
    #[serde(default = "default_v5")]
    pub pnpm: String,
    /// actions/setup-node version. Default: "v6"
    #[serde(default = "default_v6")]
    pub setup_node: String,
    /// actions/cache version. Default: "v5"
    #[serde(default = "default_v5")]
    pub cache: String,
    /// dopplerhq/cli-action version. Default: "v3"
    #[serde(default = "default_v3")]
    pub doppler: String,
    /// actions/upload-artifact version. Default: "v7"
    #[serde(default = "default_v7")]
    pub upload_artifact: String,
    /// actions/download-artifact version. Default: "v7"
    #[serde(default = "default_v7")]
    pub download_artifact: String,
}

fn default_v7() -> String { "v7".to_string() }
fn default_v6() -> String { "v6".to_string() }
fn default_v5() -> String { "v5".to_string() }
fn default_v3() -> String { "v3".to_string() }

impl Default for ActionsVersions {
    fn default() -> Self {
        ActionsVersions {
            checkout: default_v6(),
            pnpm: default_v5(),
            setup_node: default_v6(),
            cache: default_v5(),
            doppler: default_v3(),
            upload_artifact: default_v7(),
            download_artifact: default_v7(),
        }
    }
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

// ── Value injection types ────────────────────────────────────

/// Value to inject into files via `# airis:inject <key>` markers.
///
/// Simple form:  `playwright_image = "mcr.microsoft.com/playwright:v1.58.0-noble"`
/// Template form: `playwright_image = { template = "mcr.microsoft.com/playwright:v{version}-noble", from_catalog = "@playwright/test" }`
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum InjectValue {
    Simple(String),
    Template {
        template: String,
        from_catalog: String,
    },
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

    #[test]
    fn test_resolve_name_from_path() {
        let ws = WorkspaceSection::default();
        let mut app = ProjectDefinition {
            path: Some("apps/corporate".to_string()),
            framework: Some("nextjs".to_string()),
            ..Default::default()
        };
        assert!(app.name.is_empty());
        app.resolve(&ws);
        assert_eq!(app.name, "corporate");
    }

    #[test]
    fn test_resolve_scope_from_workspace() {
        let mut ws = WorkspaceSection::default();
        ws.scope = Some("@myorg".to_string());
        let mut app = ProjectDefinition {
            name: "my-app".to_string(),
            ..Default::default()
        };
        assert!(app.scope.is_none());
        app.resolve(&ws);
        assert_eq!(app.scope.as_deref(), Some("@myorg"));
    }

    #[test]
    fn test_resolve_scope_not_overridden() {
        let mut ws = WorkspaceSection::default();
        ws.scope = Some("@myorg".to_string());
        let mut app = ProjectDefinition {
            name: "my-app".to_string(),
            scope: Some("@custom".to_string()),
            ..Default::default()
        };
        app.resolve(&ws);
        assert_eq!(app.scope.as_deref(), Some("@custom"));
    }

    #[test]
    fn test_resolve_port_from_framework() {
        let ws = WorkspaceSection::default();
        let mut app = ProjectDefinition {
            name: "my-app".to_string(),
            framework: Some("nextjs".to_string()),
            ..Default::default()
        };
        assert!(app.port.is_none());
        app.resolve(&ws);
        assert_eq!(app.port, Some(3000));
    }

    #[test]
    fn test_resolve_deploy_defaults_from_framework() {
        let ws = WorkspaceSection::default();
        let mut app = ProjectDefinition {
            name: "my-app".to_string(),
            framework: Some("nextjs".to_string()),
            deploy: Some(AppDeployConfig {
                enabled: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        app.resolve(&ws);
        let deploy = app.deploy.as_ref().unwrap();
        assert_eq!(deploy.variant.as_deref(), Some("nextjs"));
        assert_eq!(deploy.port, Some(3000));
        assert_eq!(deploy.health_path.as_deref(), Some("/api/health"));
    }

    #[test]
    fn test_resolve_deploy_explicit_not_overridden() {
        let ws = WorkspaceSection::default();
        let mut app = ProjectDefinition {
            name: "my-app".to_string(),
            framework: Some("nextjs".to_string()),
            deploy: Some(AppDeployConfig {
                enabled: true,
                port: Some(8080),
                health_path: Some("/custom-health".to_string()),
                variant: Some("node".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        app.resolve(&ws);
        let deploy = app.deploy.as_ref().unwrap();
        assert_eq!(deploy.variant.as_deref(), Some("node"));
        assert_eq!(deploy.port, Some(8080));
        assert_eq!(deploy.health_path.as_deref(), Some("/custom-health"));
    }
}
