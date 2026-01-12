//! Docker build: ContextBuilder + Dockerfile generator + BuildKit runner
//!
//! Implements `airis build --docker <app>` functionality

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::channel::{resolve_channel, RuntimeChannel, RuntimeFamily, Toolchain};
use crate::dag::Dag;
use crate::pnpm::{build_workspace_map, PnpmLock};

/// Build configuration
#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub target: String,          // e.g., "apps/focustoday-api"
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

// =============================================================================
// Cache functions
// =============================================================================

/// Get cache directory path: ~/.airis/.cache/<project>/<hash>/
fn cache_dir(project: &str, hash: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let project_safe = project.replace('/', "_");
    PathBuf::from(home)
        .join(".airis")
        .join(".cache")
        .join(project_safe)
        .join(hash)
}

/// Check if cache hit exists for given project and hash
pub fn cache_hit(project: &str, hash: &str) -> Option<CachedArtifact> {
    let artifact_path = cache_dir(project, hash).join("artifact.json");
    if artifact_path.exists() {
        let content = fs::read_to_string(&artifact_path).ok()?;
        serde_json::from_str(&content).ok()
    } else {
        None
    }
}

/// Store artifact in cache
pub fn cache_store(project: &str, hash: &str, artifact: &CachedArtifact) -> Result<()> {
    let dir = cache_dir(project, hash);
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create cache directory: {}", dir.display()))?;

    let artifact_path = dir.join("artifact.json");
    let content = serde_json::to_string_pretty(artifact)
        .context("Failed to serialize artifact")?;

    fs::write(&artifact_path, content)
        .with_context(|| format!("Failed to write cache: {}", artifact_path.display()))?;

    Ok(())
}

/// Context builder - creates minimal Docker build context
pub struct ContextBuilder<'a> {
    root: &'a Path,
    dag: &'a Dag,
    #[allow(dead_code)]
    lock: &'a PnpmLock,
    target: &'a str,
}

impl<'a> ContextBuilder<'a> {
    pub fn new(root: &'a Path, dag: &'a Dag, lock: &'a PnpmLock, target: &'a str) -> Self {
        Self {
            root,
            dag,
            lock,
            target,
        }
    }

    /// Build minimal context directory
    /// Returns path to context dir (temp or specified)
    pub fn build(&self, out_dir: Option<&Path>) -> Result<PathBuf> {
        // Get dependency order
        let dep_paths = self.dag.get_dep_paths(self.target)?;

        // Create context directory
        let ctx_dir = match out_dir {
            Some(p) => {
                fs::create_dir_all(p)?;
                p.to_path_buf()
            }
            None => {
                let temp = tempfile::tempdir()?;
                let path = temp.path().to_path_buf();
                // Keep the temp directory alive (don't delete on drop)
                std::mem::forget(temp);
                path
            }
        };

        println!("📦 Building context for {} ({} packages)", self.target, dep_paths.len());

        // 1. Copy root files
        self.copy_root_files(&ctx_dir)?;

        // 2. Copy each dependency in order
        for dep_path in &dep_paths {
            self.copy_package(&ctx_dir, dep_path)?;
        }

        // 3. Generate inputs manifest for hash verification
        self.write_inputs_manifest(&ctx_dir, &dep_paths)?;

        Ok(ctx_dir)
    }

    fn copy_root_files(&self, ctx: &Path) -> Result<()> {
        // Essential root files for pnpm workspace
        let root_files = [
            "package.json",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            ".npmrc",
            "tsconfig.base.json",
            "tsconfig.json",
        ];

        for file in &root_files {
            let src = self.root.join(file);
            if src.exists() {
                let dst = ctx.join(file);
                fs::copy(&src, &dst)
                    .with_context(|| format!("Failed to copy {}", file))?;
            }
        }

        Ok(())
    }

    fn copy_package(&self, ctx: &Path, pkg_path: &str) -> Result<()> {
        let src_dir = self.root.join(pkg_path);
        let dst_dir = ctx.join(pkg_path);

        if !src_dir.exists() {
            bail!("Package directory not found: {}", pkg_path);
        }

        fs::create_dir_all(&dst_dir)?;

        // Files to copy for each package
        let essential_files = ["package.json", "tsconfig.json", "tsconfig.build.json", ".npmrc"];

        for file in &essential_files {
            let src = src_dir.join(file);
            if src.exists() {
                fs::copy(&src, dst_dir.join(file))?;
            }
        }

        // Copy src/ directory
        let src_src = src_dir.join("src");
        if src_src.exists() {
            copy_dir_recursive(&src_src, &dst_dir.join("src"))?;
        }

        // Copy public/ directory (for Next.js apps)
        let src_public = src_dir.join("public");
        if src_public.exists() {
            copy_dir_recursive(&src_public, &dst_dir.join("public"))?;
        }

        // Copy config files (next.config.*, tailwind.config.*, tsup.config.*, postcss.config.*)
        for entry in fs::read_dir(&src_dir)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("next.config")
                || name_str.starts_with("tailwind.config")
                || name_str.starts_with("tsup.config")
                || name_str.starts_with("postcss.config")
            {
                fs::copy(entry.path(), dst_dir.join(&name))?;
            }
        }

        Ok(())
    }

    fn write_inputs_manifest(&self, ctx: &Path, dep_paths: &[String]) -> Result<()> {
        let manifest = serde_json::json!({
            "target": self.target,
            "dependencies": dep_paths,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let airis_dir = ctx.join(".airis");
        fs::create_dir_all(&airis_dir)?;
        fs::write(
            airis_dir.join("inputs.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;

        Ok(())
    }
}

/// Detect if a Node.js project uses Next.js by checking package.json
fn detect_nextjs(target: &str) -> bool {
    let pkg_json_path = Path::new(target).join("package.json");
    if !pkg_json_path.exists() {
        return false;
    }

    let content = match fs::read_to_string(&pkg_json_path) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(j) => j,
        Err(_) => return false,
    };

    // Check dependencies and devDependencies for "next"
    let has_next_dep = json["dependencies"]
        .as_object()
        .is_some_and(|deps| deps.contains_key("next"));
    let has_next_dev_dep = json["devDependencies"]
        .as_object()
        .is_some_and(|deps| deps.contains_key("next"));

    has_next_dep || has_next_dev_dep
}

/// Generate Dockerfile based on runtime family
pub fn generate_dockerfile_for_toolchain(
    target: &str,
    toolchain: &Toolchain,
    build_args: &BTreeMap<String, String>,
) -> String {
    match toolchain.family {
        RuntimeFamily::Node => {
            // Detect if this is a Next.js project from package.json
            let is_nextjs = detect_nextjs(target);
            if is_nextjs {
                generate_nextjs_dockerfile(target, &toolchain.version, build_args)
            } else {
                generate_node_dockerfile(target, &toolchain.version, build_args)
            }
        }
        RuntimeFamily::Bun => generate_bun_dockerfile(target, &toolchain.image, build_args),
        RuntimeFamily::Deno => generate_deno_dockerfile(target, &toolchain.image, build_args),
        RuntimeFamily::Edge => generate_edge_dockerfile(target, &toolchain.image, build_args),
        RuntimeFamily::Rust => generate_rust_dockerfile(target, build_args),
        RuntimeFamily::Python => generate_python_dockerfile(target, build_args),
    }
}

/// Generate Dockerfile for Node.js app (legacy, kept for compatibility)
#[allow(dead_code)]
pub fn generate_dockerfile(
    target: &str,
    node_version: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    // Detect if this is a Next.js project from package.json
    let is_nextjs = detect_nextjs(target);

    if is_nextjs {
        generate_nextjs_dockerfile(target, node_version, build_args)
    } else {
        generate_node_dockerfile(target, node_version, build_args)
    }
}

fn generate_nextjs_dockerfile(
    target: &str,
    node_version: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker
# Target: {target}

# ---- base ----
FROM node:{node_version}-alpine AS base
RUN apk add --no-cache libc6-compat
RUN corepack enable pnpm

# ---- deps ----
FROM base AS deps
WORKDIR /app
COPY pnpm-lock.yaml package.json pnpm-workspace.yaml .npmrc* ./
COPY apps/ apps/
COPY libs/ libs/
RUN pnpm install --frozen-lockfile

# ---- build ----
FROM base AS build
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY --from=deps /app/apps ./apps
COPY --from=deps /app/libs ./libs
COPY . .
{extra_args}ARG NODE_ENV=production
ENV NODE_ENV=${{NODE_ENV}}
ENV NEXT_TELEMETRY_DISABLED=1
RUN corepack enable pnpm \
 && pnpm -r build \
 && mkdir -p {target}/public

# ---- runtime ----
FROM node:{node_version}-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV NEXT_TELEMETRY_DISABLED=1

RUN addgroup --system --gid 1001 nodejs \
 && adduser --system --uid 1001 nextjs

COPY --from=build /app/{target}/public ./public
COPY --from=build --chown=nextjs:nodejs /app/{target}/.next/standalone ./
COPY --from=build --chown=nextjs:nodejs /app/{target}/.next/static ./{target}/.next/static

USER nextjs
EXPOSE 3000
ENV PORT=3000 HOSTNAME="0.0.0.0"

CMD ["node", "{target}/server.js"]
"#,
        target = target,
        node_version = node_version,
        extra_args = extra_args,
    )
}

fn generate_node_dockerfile(
    target: &str,
    node_version: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker
# Target: {target}

# ---- base ----
FROM node:{node_version}-alpine AS base
RUN apk add --no-cache libc6-compat
RUN corepack enable pnpm

# ---- deps ----
FROM base AS deps
WORKDIR /app
COPY pnpm-lock.yaml package.json pnpm-workspace.yaml ./
COPY apps/ apps/
COPY libs/ libs/
RUN pnpm fetch

# ---- build ----
FROM base AS build
WORKDIR /app
COPY --from=deps /root/.local/share/pnpm/store /root/.local/share/pnpm/store
COPY . .
{extra_args}ARG NODE_ENV=production
ENV NODE_ENV=${{NODE_ENV}}
RUN corepack enable pnpm \
 && pnpm install --offline --frozen-lockfile \
 && pnpm -r --filter='./{target}...' build

# ---- runtime ----
FROM node:{node_version}-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production

RUN addgroup --system --gid 1001 nodejs \
 && adduser --system --uid 1001 appuser

COPY --from=build --chown=appuser:nodejs /app/{target}/dist ./dist
COPY --from=build --chown=appuser:nodejs /app/{target}/package.json ./

USER appuser
EXPOSE 3000
ENV PORT=3000

CMD ["node", "dist/index.js"]
"#,
        target = target,
        node_version = node_version,
        extra_args = extra_args,
    )
}

// =============================================================================
// Bun / Deno / Edge / Rust / Python Dockerfile templates
// =============================================================================

fn generate_bun_dockerfile(
    target: &str,
    image: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker (Bun)
# Target: {target}

FROM {image} AS deps
WORKDIR /app
COPY package.json bun.lockb ./
RUN bun install --frozen-lockfile

FROM {image} AS build
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
{extra_args}ARG NODE_ENV=production
ENV NODE_ENV=${{NODE_ENV}}
RUN bun run build

FROM {image} AS runtime
WORKDIR /app
ENV NODE_ENV=production
COPY --from=build /app/dist ./dist
COPY --from=build /app/package.json ./
EXPOSE 3000
CMD ["bun", "run", "dist/index.js"]
"#,
        target = target,
        image = image,
        extra_args = extra_args,
    )
}

fn generate_deno_dockerfile(
    target: &str,
    image: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker (Deno)
# Target: {target}

FROM {image} AS build
WORKDIR /app
COPY . .
{extra_args}RUN deno cache src/main.ts
RUN deno compile --allow-net --allow-env --output=app src/main.ts

FROM gcr.io/distroless/cc AS runtime
COPY --from=build /app/app /app
EXPOSE 3000
CMD ["/app"]
"#,
        target = target,
        image = image,
        extra_args = extra_args,
    )
}

fn generate_edge_dockerfile(
    target: &str,
    image: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker (Edge/WASM)
# Target: {target}
# Note: Edge builds typically deploy to Cloudflare Workers / Vercel Edge
# This Dockerfile is for local testing only

FROM {image} AS build
WORKDIR /app
COPY . .
{extra_args}RUN deno task build

FROM {image} AS runtime
WORKDIR /app
COPY --from=build /app/dist ./dist
EXPOSE 8787
CMD ["deno", "run", "--allow-net", "--allow-env", "dist/index.js"]
"#,
        target = target,
        image = image,
        extra_args = extra_args,
    )
}

fn generate_rust_dockerfile(
    target: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    let app_name = target.rsplit('/').next().unwrap_or(target);

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker (Rust)
# Target: {target}

FROM rust:1.83-slim AS deps
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {{}}" > src/main.rs
RUN cargo fetch

FROM rust:1.83-slim AS build
WORKDIR /app
{extra_args}COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc AS runtime
COPY --from=build /app/target/release/{app_name} /app
EXPOSE 3000
CMD ["/app"]
"#,
        target = target,
        app_name = app_name,
        extra_args = extra_args,
    )
}

fn generate_python_dockerfile(
    target: &str,
    build_args: &BTreeMap<String, String>,
) -> String {
    let extra_args: String = build_args
        .iter()
        .map(|(k, _)| format!("ARG {}\n", k))
        .collect();

    format!(
        r#"# syntax=docker/dockerfile:1.7
# Auto-generated by airis build --docker (Python)
# Target: {target}

FROM python:3.12-slim AS deps
WORKDIR /app
COPY requirements*.txt ./
RUN pip install --no-cache-dir -r requirements.txt

FROM python:3.12-slim AS runtime
WORKDIR /app
{extra_args}COPY --from=deps /usr/local /usr/local
COPY . .
EXPOSE 8000
CMD ["python", "-m", "uvicorn", "main:app", "--host", "0.0.0.0", "--port", "8000"]
"#,
        target = target,
        extra_args = extra_args,
    )
}

/// Compute content hash directly from source files (fast path for cache lookup)
/// This avoids building the full context directory when checking for cache hits
pub fn compute_content_hash(root: &Path, target: &str) -> Result<String> {
    use std::io::Read;
    use walkdir::WalkDir;

    let mut hasher = blake3::Hasher::new();

    // Hash essential root files
    let root_files = [
        "package.json",
        "pnpm-lock.yaml",
        "pnpm-workspace.yaml",
        "tsconfig.base.json",
        "tsconfig.json",
    ];

    for file in &root_files {
        let path = root.join(file);
        if path.exists() {
            hasher.update(file.as_bytes());
            let content = fs::read(&path)?;
            hasher.update(&content);
        }
    }

    // Hash target directory
    let target_dir = root.join(target);
    if target_dir.exists() {
        let mut files: Vec<_> = WalkDir::new(&target_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                // Skip build artifacts and dependencies
                !name.contains("node_modules")
                    && !name.contains(".next")
                    && !name.contains("dist")
                    && !name.contains(".turbo")
            })
            .map(|e| e.path().to_path_buf())
            .collect();

        files.sort();

        for path in files {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            hasher.update(rel.to_string_lossy().as_bytes());

            let mut file = fs::File::open(&path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            hasher.update(&buf);
        }
    }

    let hash = hasher.finalize();
    Ok(hash.to_hex()[..12].to_string())
}

/// Compute BLAKE3 hash of inputs
pub fn compute_hash(ctx_dir: &Path) -> Result<String> {
    use std::io::Read;
    use walkdir::WalkDir;

    let mut hasher = blake3::Hasher::new();

    // Hash all files in context (sorted for determinism)
    let mut files: Vec<_> = WalkDir::new(ctx_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_path_buf())
        .collect();

    files.sort();

    for path in files {
        // Hash relative path
        let rel = path.strip_prefix(ctx_dir).unwrap_or(&path);
        hasher.update(rel.to_string_lossy().as_bytes());

        // Hash content
        let mut file = fs::File::open(&path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        hasher.update(&buf);
    }

    let hash = hasher.finalize();
    Ok(hash.to_hex()[..12].to_string())
}

/// Run BuildKit build
pub fn run_buildkit(
    ctx_dir: &Path,
    dockerfile_content: &str,
    config: &BuildConfig,
    hash: &str,
) -> Result<BuildResult> {
    use std::time::Instant;

    let start = Instant::now();

    // Write Dockerfile to context
    let dockerfile_path = ctx_dir.join("Dockerfile");
    fs::write(&dockerfile_path, dockerfile_content)?;

    // Determine image name
    let app_name = config.target.rsplit('/').next().unwrap_or(&config.target);
    let image_name = config
        .image_name
        .clone()
        .unwrap_or_else(|| format!("{}:airis-{}", app_name, hash));

    println!("🐳 Building image: {}", image_name);
    println!("   Context: {}", ctx_dir.display());

    // Build command
    let mut cmd = Command::new("docker");
    cmd.arg("buildx")
        .arg("build")
        .arg("--progress=plain")
        .arg("-t")
        .arg(&image_name)
        .arg("-f")
        .arg(&dockerfile_path);

    // Add build args
    for (key, value) in &config.build_args {
        cmd.arg("--build-arg").arg(format!("{}={}", key, value));
    }

    if config.no_cache {
        cmd.arg("--no-cache");
    }

    if config.push {
        cmd.arg("--push");
    } else {
        cmd.arg("--load");
    }

    cmd.arg(ctx_dir);

    // Execute
    let status = cmd.status().context("Failed to run docker buildx")?;

    let duration = start.elapsed().as_secs();

    if !status.success() {
        bail!("Docker build failed with exit code: {:?}", status.code());
    }

    Ok(BuildResult {
        image_ref: image_name,
        hash: hash.to_string(),
        duration_secs: duration,
    })
}

/// Main entry point for `airis build --docker`
pub fn docker_build(root: &Path, config: BuildConfig) -> Result<BuildResult> {
    use colored::Colorize;

    // 1. Resolve runtime channel to toolchain
    let channel = RuntimeChannel::parse(&config.channel)?;
    let toolchain = resolve_channel(&channel)?;

    println!("{}", "==================================".bright_blue());
    println!("{}", "airis build --docker".bright_blue().bold());
    println!("Target:  {}", config.target.cyan());
    println!("Channel: {} → {} ({})", config.channel.yellow(), toolchain.image.green(), format!("{:?}", toolchain.family).dimmed());
    println!("{}", "==================================".bright_blue());

    // 2. Load pnpm-lock.yaml
    let lock_path = root.join("pnpm-lock.yaml");
    let lock = PnpmLock::load(&lock_path)?;

    // 3. Build workspace map and DAG
    let workspace_map = build_workspace_map(&lock);
    let dag = crate::dag::build_dag(&workspace_map);

    // 4. Verify target exists
    if !dag.nodes.contains_key(&config.target) {
        bail!(
            "Target '{}' not found in workspace. Available:\n{}",
            config.target,
            dag.nodes.keys().map(|k| format!("  - {}", k)).collect::<Vec<_>>().join("\n")
        );
    }

    // 5. Build context
    let ctx_builder = ContextBuilder::new(root, &dag, &lock, &config.target);
    let ctx_dir = ctx_builder.build(config.context_out.as_deref())?;

    // 6. Compute hash (includes toolchain info for cache invalidation)
    let hash = compute_hash(&ctx_dir)?;
    // Append channel to hash for cache invalidation on channel change
    let full_hash = format!("{}-{}", hash, config.channel);
    let final_hash = blake3::hash(full_hash.as_bytes()).to_hex()[..12].to_string();
    println!("📋 Input hash: {}", final_hash.yellow());

    // 7. Check cache (skip if --no-cache)
    if !config.no_cache {
        if let Some(cached) = cache_hit(&config.target, &final_hash) {
            println!();
            println!("{}", "==================================".bright_blue());
            println!("{}", "⚡ Cache hit! Skipping build.".green().bold());
            println!("   Image: {}", cached.image_ref);
            println!("   Hash:  {}", cached.hash);
            println!("   Built: {}", cached.built_at);
            println!("{}", "==================================".bright_blue());

            return Ok(BuildResult {
                image_ref: cached.image_ref,
                hash: cached.hash,
                duration_secs: 0,
            });
        }
    }

    // 8. Generate Dockerfile based on runtime family
    let dockerfile = generate_dockerfile_for_toolchain(&config.target, &toolchain, &config.build_args);

    // 9. Run BuildKit
    let result = run_buildkit(&ctx_dir, &dockerfile, &config, &final_hash)?;

    // 10. Store in cache
    let artifact = CachedArtifact {
        image_ref: result.image_ref.clone(),
        hash: result.hash.clone(),
        built_at: chrono::Utc::now().to_rfc3339(),
        target: config.target.clone(),
    };
    if let Err(e) = cache_store(&config.target, &final_hash, &artifact) {
        eprintln!("⚠️  Warning: Failed to store cache: {}", e);
    }

    // 10. Print summary
    println!();
    println!("{}", "==================================".bright_blue());
    println!("{}", "✅ Build successful!".green());
    println!("   Image: {}", result.image_ref);
    println!("   Hash:  {}", result.hash);
    println!("   Time:  {}s", result.duration_secs);
    println!("{}", "==================================".bright_blue());

    Ok(result)
}

/// Recursively copy directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_build_config_default() {
        let config = BuildConfig::default();
        assert_eq!(config.channel, "lts");
        assert!(!config.push);
        assert!(!config.no_cache);
        assert!(config.image_name.is_none());
    }

    #[test]
    fn test_cache_dir_structure() {
        let dir = cache_dir("apps/web", "abc123");
        assert!(dir.to_string_lossy().contains(".airis"));
        assert!(dir.to_string_lossy().contains(".cache"));
        assert!(dir.to_string_lossy().contains("apps_web"));
        assert!(dir.to_string_lossy().contains("abc123"));
    }

    #[test]
    fn test_cache_hit_miss() {
        // Non-existent cache should return None
        let result = cache_hit("nonexistent/project", "nonexistent_hash_12345");
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_store_and_hit() {
        let project = "test_project_cache";
        let hash = "test_hash_abc123";

        let artifact = CachedArtifact {
            image_ref: "test:latest".to_string(),
            hash: hash.to_string(),
            built_at: "2025-01-01T00:00:00Z".to_string(),
            target: project.to_string(),
        };

        // Store
        cache_store(project, hash, &artifact).unwrap();

        // Hit
        let cached = cache_hit(project, hash);
        assert!(cached.is_some());

        let cached = cached.unwrap();
        assert_eq!(cached.image_ref, "test:latest");
        assert_eq!(cached.hash, hash);

        // Cleanup
        let dir = cache_dir(project, hash);
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn test_compute_hash_deterministic() {
        let dir = tempdir().unwrap();
        let test_file = dir.path().join("test.txt");
        std::fs::write(&test_file, "hello world").unwrap();

        let hash1 = compute_hash(dir.path()).unwrap();
        let hash2 = compute_hash(dir.path()).unwrap();

        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 12); // 12 hex chars
    }

    #[test]
    fn test_compute_hash_changes_with_content() {
        let dir = tempdir().unwrap();
        let test_file = dir.path().join("test.txt");

        std::fs::write(&test_file, "content v1").unwrap();
        let hash1 = compute_hash(dir.path()).unwrap();

        std::fs::write(&test_file, "content v2").unwrap();
        let hash2 = compute_hash(dir.path()).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_generate_nextjs_dockerfile() {
        let dockerfile = generate_nextjs_dockerfile(
            "apps/web",
            "22",
            &BTreeMap::new(),
        );

        assert!(dockerfile.contains("FROM node:22-alpine"));
        assert!(dockerfile.contains("apps/web"));
        assert!(dockerfile.contains("pnpm"));
        assert!(dockerfile.contains("NEXT_TELEMETRY_DISABLED"));
    }

    #[test]
    fn test_generate_bun_dockerfile() {
        let dockerfile = generate_bun_dockerfile(
            "apps/api",
            "oven/bun:1.1-alpine",
            &BTreeMap::new(),
        );

        assert!(dockerfile.contains("FROM oven/bun:1.1-alpine"));
        assert!(dockerfile.contains("bun install"));
        assert!(dockerfile.contains("bun run"));
    }

    #[test]
    fn test_generate_rust_dockerfile() {
        let dockerfile = generate_rust_dockerfile(
            "apps/cli",
            &BTreeMap::new(),
        );

        assert!(dockerfile.contains("FROM rust:"));
        assert!(dockerfile.contains("cargo build --release"));
        assert!(dockerfile.contains("distroless"));
    }

    #[test]
    fn test_generate_python_dockerfile() {
        let dockerfile = generate_python_dockerfile(
            "apps/api",
            &BTreeMap::new(),
        );

        assert!(dockerfile.contains("FROM python:"));
        assert!(dockerfile.contains("pip install"));
        assert!(dockerfile.contains("uvicorn"));
    }

    #[test]
    fn test_generate_deno_dockerfile() {
        let dockerfile = generate_deno_dockerfile(
            "apps/api",
            "denoland/deno:alpine",
            &BTreeMap::new(),
        );

        assert!(dockerfile.contains("FROM denoland/deno:alpine"));
        assert!(dockerfile.contains("deno"));
    }

    #[test]
    fn test_dockerfile_with_build_args() {
        let mut build_args = BTreeMap::new();
        build_args.insert("API_KEY".to_string(), "secret".to_string());

        let dockerfile = generate_nextjs_dockerfile(
            "apps/web",
            "22",
            &build_args,
        );

        assert!(dockerfile.contains("ARG API_KEY"));
    }

    #[test]
    fn test_detect_nextjs_with_next_dep() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "dependencies": {"next": "14.0.0", "react": "18.0.0"}}"#;
        std::fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert!(detect_nextjs(&dir.path().to_string_lossy()));
    }

    #[test]
    fn test_detect_nextjs_with_next_devdep() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "devDependencies": {"next": "14.0.0"}}"#;
        std::fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert!(detect_nextjs(&dir.path().to_string_lossy()));
    }

    #[test]
    fn test_detect_nextjs_without_next() {
        let dir = tempdir().unwrap();
        let pkg_json = r#"{"name": "test", "dependencies": {"express": "4.0.0"}}"#;
        std::fs::write(dir.path().join("package.json"), pkg_json).unwrap();

        assert!(!detect_nextjs(&dir.path().to_string_lossy()));
    }

    #[test]
    fn test_detect_nextjs_no_package_json() {
        let dir = tempdir().unwrap();
        // No package.json file
        assert!(!detect_nextjs(&dir.path().to_string_lossy()));
    }
}
