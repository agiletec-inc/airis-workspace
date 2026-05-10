//! Type definitions for project discovery.

use indexmap::IndexMap;

use serde::{Deserialize, Serialize};

/// Detected framework for an app
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Framework {
    NextJs,
    Vite,
    Hono,
    Node,
    Rust,
    Python,
    Unknown,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Framework::NextJs => write!(f, "nextjs"),
            Framework::Vite => write!(f, "vite"),
            Framework::Hono => write!(f, "hono"),
            Framework::Node => write!(f, "node"),
            Framework::Rust => write!(f, "rust"),
            Framework::Python => write!(f, "python"),
            Framework::Unknown => write!(f, "unknown"),
        }
    }
}

/// Location category for docker-compose files
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComposeLocation {
    Root,      // ./compose.yml (should be moved)
    Workspace, // workspace/compose.yml
    Supabase,  // supabase/compose.yml
    Traefik,   // traefik/compose.yml
    App,       // apps/*/compose.yml
}

impl std::fmt::Display for ComposeLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposeLocation::Root => write!(f, "root"),
            ComposeLocation::Workspace => write!(f, "workspace"),
            ComposeLocation::Supabase => write!(f, "supabase"),
            ComposeLocation::Traefik => write!(f, "traefik"),
            ComposeLocation::App => write!(f, "app"),
        }
    }
}

/// Lightweight project info discovered from workspace patterns.
/// Used by `airis gen` to auto-discover apps/libs without explicit [[app]] entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProject {
    pub name: String,
    pub path: String,
    pub framework: Framework,
}

/// Detected application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedApp {
    pub name: String,
    pub path: String,
    pub framework: Framework,
    pub has_dockerfile: bool,
    #[allow(dead_code)]
    pub package_name: Option<String>,
    /// Scripts from package.json
    pub scripts: IndexMap<String, String>,
    /// Dependencies from package.json
    pub deps: IndexMap<String, String>,
    /// Dev dependencies from package.json
    pub dev_deps: IndexMap<String, String>,
}

/// Detected library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedLib {
    pub name: String,
    pub path: String,
    #[allow(dead_code)]
    pub package_name: Option<String>,
    /// Scripts from package.json
    pub scripts: IndexMap<String, String>,
    /// Dependencies from package.json
    pub deps: IndexMap<String, String>,
    /// Dev dependencies from package.json
    pub dev_deps: IndexMap<String, String>,
}

/// Detected docker-compose file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedCompose {
    pub path: String,
    pub location: ComposeLocation,
}

/// Result of project discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub apps: Vec<DetectedApp>,
    pub libs: Vec<DetectedLib>,
    pub compose_files: Vec<DetectedCompose>,
    pub catalog: IndexMap<String, String>,
}

impl DiscoveryResult {
    pub fn is_empty(&self) -> bool {
        self.apps.is_empty() && self.libs.is_empty() && self.compose_files.is_empty()
    }
}

/// Package info extracted from package.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageInfo {
    pub scripts: IndexMap<String, String>,
    pub deps: IndexMap<String, String>,
    pub dev_deps: IndexMap<String, String>,
}
