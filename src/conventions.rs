//! Framework conventions — deterministic lookup tables.
//!
//! All defaults are derived from framework + environment, never hardcoded
//! at call sites. Manifest fields override these conventions.

/// Framework-specific default values.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct FrameworkDefaults {
    pub port: u16,
    pub health_path: &'static str,
    pub entrypoint: &'static str,
    pub dev_script: &'static str,
    pub build_script: &'static str,
    pub start_script: &'static str,
    /// Extra env vars needed for Docker dev (e.g., file-watcher polling)
    pub docker_env: &'static [(&'static str, &'static str)],
    /// Convention scripts for package.json (e.g., dev, build, start, lint, typecheck)
    pub default_scripts: &'static [(&'static str, &'static str)],
}

/// Lookup framework defaults. Pure lookup table, no IO.
pub fn framework_defaults(framework: &str) -> FrameworkDefaults {
    match framework {
        "nextjs" => FrameworkDefaults {
            port: 3000,
            health_path: "/api/health",
            entrypoint: "server.js",
            dev_script: "next dev",
            build_script: "next build",
            start_script: "next start",
            docker_env: &[("WATCHPACK_POLLING", "true")],
            default_scripts: &[
                ("dev", "next dev"),
                ("build", "NODE_ENV=production next build"),
                ("start", "next start"),
                ("lint", "biome check ."),
                ("typecheck", "tsc --noEmit"),
            ],
        },
        "react-vite" | "vite" => FrameworkDefaults {
            port: 5173,
            health_path: "/",
            entrypoint: "dist/index.html",
            dev_script: "vite",
            build_script: "vite build",
            start_script: "vite preview",
            docker_env: &[("CHOKIDAR_USEPOLLING", "true")],
            default_scripts: &[
                ("dev", "vite"),
                ("build", "vite build"),
                ("typecheck", "tsc --noEmit"),
            ],
        },
        "hono" => FrameworkDefaults {
            port: 3000,
            health_path: "/health",
            entrypoint: "dist/index.js",
            dev_script: "tsx watch src/index.ts",
            build_script: "tsup",
            start_script: "node dist/index.js",
            docker_env: &[],
            default_scripts: &[
                ("dev", "tsx watch src/index.ts"),
                ("build", "tsup"),
                ("start", "node dist/index.js"),
                ("lint", "biome check ."),
                ("typecheck", "tsc --noEmit"),
            ],
        },
        "node" => FrameworkDefaults {
            port: 3000,
            health_path: "/health",
            entrypoint: "dist/index.js",
            dev_script: "tsx watch src/index.ts",
            build_script: "tsup",
            start_script: "node dist/index.js",
            docker_env: &[],
            default_scripts: &[
                ("build", "tsup"),
                ("dev", "tsup --watch"),
                ("typecheck", "tsc --noEmit"),
            ],
        },
        "cloudflare-worker" => FrameworkDefaults {
            port: 8787,
            health_path: "/",
            entrypoint: "src/index.ts",
            dev_script: "wrangler dev",
            build_script: "wrangler deploy --dry-run",
            start_script: "wrangler dev",
            docker_env: &[],
            default_scripts: &[
                ("dev", "wrangler dev"),
                ("deploy", "wrangler deploy"),
                ("deploy:staging", "wrangler deploy --env staging"),
            ],
        },
        "rust" => FrameworkDefaults {
            port: 3000,
            health_path: "/health",
            entrypoint: "target/release/app",
            dev_script: "cargo watch -x run",
            build_script: "cargo build --release",
            start_script: "./target/release/app",
            docker_env: &[],
            default_scripts: &[
                ("dev", "cargo watch -x run"),
                ("build", "cargo build --release"),
            ],
        },
        "python" => FrameworkDefaults {
            port: 8000,
            health_path: "/health",
            entrypoint: "main.py",
            dev_script: "uvicorn main:app --reload",
            build_script: "echo 'no build step'",
            start_script: "uvicorn main:app",
            docker_env: &[],
            default_scripts: &[
                ("dev", "uvicorn main:app --reload"),
                ("start", "uvicorn main:app"),
            ],
        },
        // Unknown framework: sensible defaults
        _ => FrameworkDefaults {
            port: 3000,
            health_path: "/health",
            entrypoint: "dist/index.js",
            dev_script: "echo 'no dev script defined'",
            build_script: "echo 'no build script defined'",
            start_script: "node dist/index.js",
            docker_env: &[],
            default_scripts: &[],
        },
    }
}

/// Derive name from directory path (last component).
/// "apps/corporate" → "corporate"
/// "products/airis/voice-gateway" → "voice-gateway"
pub fn name_from_path(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Map Node.js major version to ECMAScript target.
/// Based on Node.js ECMAScript compatibility:
/// <https://node.green>
pub fn node_version_to_es_target(node_major: u32) -> &'static str {
    match node_major {
        24.. => "ES2024",
        22.. => "ES2023",
        20.. => "ES2022",
        18.. => "ES2021",
        16.. => "ES2020",
        _ => "ES2019",
    }
}

/// Parse Node.js major version from a Docker image string.
/// "node:24-bookworm" → Some(24)
/// "node:22-alpine" → Some(22)
/// "python:3.12" → None
pub fn parse_node_version_from_image(image: &str) -> Option<u32> {
    // Extract the part after "node:" and before any "-" suffix
    let tag = image.strip_prefix("node:")?;
    let version_str = tag.split('-').next()?;
    // Handle "22", "24", "lts" etc.
    version_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nextjs_defaults() {
        let d = framework_defaults("nextjs");
        assert_eq!(d.port, 3000);
        assert_eq!(d.health_path, "/api/health");
        assert_eq!(d.entrypoint, "server.js");
        assert_eq!(d.dev_script, "next dev");
    }

    #[test]
    fn test_hono_defaults() {
        let d = framework_defaults("hono");
        assert_eq!(d.port, 3000);
        assert_eq!(d.health_path, "/health");
        assert_eq!(d.entrypoint, "dist/index.js");
    }

    #[test]
    fn test_python_defaults() {
        let d = framework_defaults("python");
        assert_eq!(d.port, 8000);
    }

    #[test]
    fn test_cloudflare_worker_defaults() {
        let d = framework_defaults("cloudflare-worker");
        assert_eq!(d.port, 8787);
    }

    #[test]
    fn test_unknown_framework_has_sensible_defaults() {
        let d = framework_defaults("unknown-framework");
        assert_eq!(d.port, 3000);
        assert_eq!(d.health_path, "/health");
    }

    #[test]
    fn test_name_from_path() {
        assert_eq!(name_from_path("apps/corporate"), "corporate");
        assert_eq!(
            name_from_path("products/airis/voice-gateway"),
            "voice-gateway"
        );
        assert_eq!(name_from_path("libs/ui"), "ui");
        assert_eq!(name_from_path("standalone"), "standalone");
    }

    #[test]
    fn test_node_version_to_es_target() {
        assert_eq!(node_version_to_es_target(24), "ES2024");
        assert_eq!(node_version_to_es_target(22), "ES2023");
        assert_eq!(node_version_to_es_target(20), "ES2022");
        assert_eq!(node_version_to_es_target(18), "ES2021");
        assert_eq!(node_version_to_es_target(16), "ES2020");
        assert_eq!(node_version_to_es_target(14), "ES2019");
    }

    #[test]
    fn test_parse_node_version_from_image() {
        assert_eq!(parse_node_version_from_image("node:24-bookworm"), Some(24));
        assert_eq!(parse_node_version_from_image("node:22-alpine"), Some(22));
        assert_eq!(parse_node_version_from_image("node:18"), Some(18));
        assert_eq!(parse_node_version_from_image("python:3.12"), None);
        assert_eq!(parse_node_version_from_image("ubuntu:22.04"), None);
    }

    #[test]
    fn test_nextjs_has_watchpack_polling() {
        let d = framework_defaults("nextjs");
        assert!(d.docker_env.iter().any(|(k, _)| *k == "WATCHPACK_POLLING"));
    }

    #[test]
    fn test_vite_has_chokidar_polling() {
        let d = framework_defaults("vite");
        assert!(
            d.docker_env
                .iter()
                .any(|(k, _)| *k == "CHOKIDAR_USEPOLLING")
        );
    }

    #[test]
    fn test_node_has_no_polling_env() {
        let d = framework_defaults("node");
        assert!(d.docker_env.is_empty());
    }
}
