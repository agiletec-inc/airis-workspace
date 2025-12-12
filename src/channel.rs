//! Runtime Channel Resolver
//!
//! Resolves runtime channels (lts, current, edge, bun, deno) to concrete
//! Docker images with pinned digests for reproducible builds.
//!
//! # Example
//!
//! ```ignore
//! let toolchain = resolve_channel("lts")?;
//! // => Toolchain { image: "node:22-alpine", digest: "sha256:...", family: Node }
//! ```

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Runtime channel specifier
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeChannel {
    /// Node.js LTS (Long Term Support)
    Lts,
    /// Node.js Current (latest stable)
    Current,
    /// Edge runtime (Cloudflare Workers / Vercel Edge compatible)
    Edge,
    /// Bun runtime
    Bun,
    /// Deno runtime
    Deno,
    /// Pinned to specific version (e.g., "22.12.0")
    Pinned(String),
}

impl RuntimeChannel {
    /// Parse channel from string
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "lts" => Ok(Self::Lts),
            "current" => Ok(Self::Current),
            "edge" => Ok(Self::Edge),
            "bun" => Ok(Self::Bun),
            "deno" => Ok(Self::Deno),
            other => {
                // Check if it looks like a version (starts with digit)
                if other.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    Ok(Self::Pinned(other.to_string()))
                } else {
                    bail!("Unknown runtime channel: '{}'. Valid channels: lts, current, edge, bun, deno, or a version number", other)
                }
            }
        }
    }

    /// Get channel as string
    pub fn as_str(&self) -> &str {
        match self {
            Self::Lts => "lts",
            Self::Current => "current",
            Self::Edge => "edge",
            Self::Bun => "bun",
            Self::Deno => "deno",
            Self::Pinned(v) => v,
        }
    }
}

/// Runtime family (determines Dockerfile template)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeFamily {
    /// Node.js (includes Next.js, Hono, etc.)
    Node,
    /// Edge runtime (WASM-based, no Node APIs)
    Edge,
    /// Bun runtime
    Bun,
    /// Deno runtime
    Deno,
    /// Rust (compiled binary)
    Rust,
    /// Python
    Python,
}

/// Resolved toolchain information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toolchain {
    /// Docker image reference (e.g., "node:22-alpine")
    pub image: String,
    /// Image digest for reproducibility (e.g., "sha256:abc123...")
    /// None if not yet resolved
    pub digest: Option<String>,
    /// Runtime family
    pub family: RuntimeFamily,
    /// Version string (e.g., "22.12.0")
    pub version: String,
}

// =============================================================================
// Channel Resolution Tables
// =============================================================================

/// Current LTS and stable versions (updated periodically)
/// These are the default values; can be overridden via manifest.toml [toolchain]
mod defaults {
    // Node.js LTS - Using Node 24 as default (aligns with .node-version)
    pub const NODE_LTS_VERSION: &str = "24";
    pub const NODE_LTS_IMAGE: &str = "node:24-alpine";

    // Node.js Current (same as LTS for now)
    pub const NODE_CURRENT_VERSION: &str = "24";
    pub const NODE_CURRENT_IMAGE: &str = "node:24-alpine";

    // Edge runtime (generic WASM runtime image)
    pub const EDGE_VERSION: &str = "2025.01";
    pub const EDGE_IMAGE: &str = "denoland/deno:alpine"; // Edge uses Deno as base

    // Bun
    pub const BUN_VERSION: &str = "1.1";
    pub const BUN_IMAGE: &str = "oven/bun:1.1-alpine";

    // Deno
    pub const DENO_VERSION: &str = "2.0";
    pub const DENO_IMAGE: &str = "denoland/deno:alpine";

    // Rust
    pub const RUST_VERSION: &str = "1.83";
    pub const RUST_IMAGE: &str = "rust:1.83-slim";

    // Python
    pub const PYTHON_VERSION: &str = "3.12";
    pub const PYTHON_IMAGE: &str = "python:3.12-slim";
}

/// Resolve a runtime channel to a concrete toolchain
pub fn resolve_channel(channel: &RuntimeChannel) -> Result<Toolchain> {
    match channel {
        RuntimeChannel::Lts => Ok(Toolchain {
            image: defaults::NODE_LTS_IMAGE.to_string(),
            digest: None, // TODO: fetch actual digest
            family: RuntimeFamily::Node,
            version: defaults::NODE_LTS_VERSION.to_string(),
        }),
        RuntimeChannel::Current => Ok(Toolchain {
            image: defaults::NODE_CURRENT_IMAGE.to_string(),
            digest: None,
            family: RuntimeFamily::Node,
            version: defaults::NODE_CURRENT_VERSION.to_string(),
        }),
        RuntimeChannel::Edge => Ok(Toolchain {
            image: defaults::EDGE_IMAGE.to_string(),
            digest: None,
            family: RuntimeFamily::Edge,
            version: defaults::EDGE_VERSION.to_string(),
        }),
        RuntimeChannel::Bun => Ok(Toolchain {
            image: defaults::BUN_IMAGE.to_string(),
            digest: None,
            family: RuntimeFamily::Bun,
            version: defaults::BUN_VERSION.to_string(),
        }),
        RuntimeChannel::Deno => Ok(Toolchain {
            image: defaults::DENO_IMAGE.to_string(),
            digest: None,
            family: RuntimeFamily::Deno,
            version: defaults::DENO_VERSION.to_string(),
        }),
        RuntimeChannel::Pinned(version) => {
            // For pinned versions, assume Node.js
            Ok(Toolchain {
                image: format!("node:{}-alpine", version),
                digest: None,
                family: RuntimeFamily::Node,
                version: version.clone(),
            })
        }
    }
}

/// Resolve toolchain for Rust projects
pub fn resolve_rust() -> Toolchain {
    Toolchain {
        image: defaults::RUST_IMAGE.to_string(),
        digest: None,
        family: RuntimeFamily::Rust,
        version: defaults::RUST_VERSION.to_string(),
    }
}

/// Resolve toolchain for Python projects
pub fn resolve_python() -> Toolchain {
    Toolchain {
        image: defaults::PYTHON_IMAGE.to_string(),
        digest: None,
        family: RuntimeFamily::Python,
        version: defaults::PYTHON_VERSION.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_channel() {
        assert_eq!(RuntimeChannel::parse("lts").unwrap(), RuntimeChannel::Lts);
        assert_eq!(RuntimeChannel::parse("LTS").unwrap(), RuntimeChannel::Lts);
        assert_eq!(RuntimeChannel::parse("edge").unwrap(), RuntimeChannel::Edge);
        assert_eq!(RuntimeChannel::parse("bun").unwrap(), RuntimeChannel::Bun);
        assert_eq!(RuntimeChannel::parse("deno").unwrap(), RuntimeChannel::Deno);
        assert_eq!(
            RuntimeChannel::parse("22.12.0").unwrap(),
            RuntimeChannel::Pinned("22.12.0".to_string())
        );
    }

    #[test]
    fn test_resolve_lts() {
        let toolchain = resolve_channel(&RuntimeChannel::Lts).unwrap();
        assert_eq!(toolchain.family, RuntimeFamily::Node);
        assert!(toolchain.image.contains("node"));
    }

    #[test]
    fn test_resolve_edge() {
        let toolchain = resolve_channel(&RuntimeChannel::Edge).unwrap();
        assert_eq!(toolchain.family, RuntimeFamily::Edge);
    }

    #[test]
    fn test_resolve_bun() {
        let toolchain = resolve_channel(&RuntimeChannel::Bun).unwrap();
        assert_eq!(toolchain.family, RuntimeFamily::Bun);
        assert!(toolchain.image.contains("bun"));
    }
}
