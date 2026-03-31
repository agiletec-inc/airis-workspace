//! Docker build: ContextBuilder + Dockerfile generator + BuildKit runner
//!
//! Implements `airis build --docker <app>` functionality

pub mod cache;
pub mod context;
pub mod dockerfile;
pub mod hash;
pub mod buildkit;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;

// Re-export public API
pub use cache::{cache_hit, cache_store};
pub use hash::compute_content_hash;
pub use buildkit::docker_build;

/// Build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub target: String, // e.g., "apps/focustoday-api"
    pub image_name: Option<String>,
    pub push: bool,
    pub no_cache: bool,
    pub build_args: BTreeMap<String, String>,
    pub context_out: Option<PathBuf>,
    /// Runtime channel (lts, current, edge, bun, deno, or pinned version)
    pub channel: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            target: String::new(),
            image_name: None,
            push: false,
            no_cache: false,
            build_args: BTreeMap::new(),
            context_out: None,
            channel: "lts".to_string(),
        }
    }
}

/// Build result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildResult {
    pub image_ref: String,
    pub hash: String,
    pub duration_secs: u64,
}

/// Cached artifact metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedArtifact {
    pub image_ref: String,
    pub hash: String,
    pub built_at: String,
    pub target: String,
}
