# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

Rust CLI tool (`airis`) that manages Docker-first monorepo workspaces. Reads `manifest.toml` (single source of truth) and generates `package.json`, `pnpm-workspace.yaml`, `compose.yml`, `Dockerfile`, CI workflows.

This repo is a **Rust project** — build with `cargo` directly on host (not Docker-first).

## Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # All tests
cargo test test_name           # Single test
cargo test -- --nocapture      # Tests with stdout
cargo install --path .         # Install to ~/.cargo/bin
```

After release build, copy to user's airis location:
```bash
cp target/release/airis ~/.local/share/cargo/bin/airis
```

## Architecture

### Data Flow

```
manifest.toml (user edits this)
    → src/manifest.rs (parse to Manifest struct)
    → src/templates/mod.rs (Handlebars rendering)
    → generated files (package.json, compose.yml, Dockerfile, CI)
```

### Key Modules

| Module | Purpose |
|--------|---------|
| `manifest.rs` | TOML schema: `Manifest`, `ProjectDefinition`, `ServiceConfig`, etc. |
| `templates/mod.rs` | Handlebars template engine for all generated files |
| `commands/run.rs` | Execute `[commands]` from manifest, env interpolation, Docker exec |
| `commands/generate.rs` | `airis gen` — orchestrates all file generation |
| `commands/init.rs` | `airis init` — creates manifest.toml template (never overwrites) |
| `commands/doctor.rs` | Workspace health check, drift detection |
| `commands/discover.rs` | Auto-scan apps/libs, detect frameworks |
| `generators/package_json.rs` | Generate per-project package.json from `[[app]]` definitions |
| `docker_build.rs` | Multi-target Docker builds with channel/version resolution |
| `dag.rs` + `executor.rs` | Parallel DAG build engine (tokio semaphore) |
| `version_resolver.rs` | Resolve `"latest"` / `"lts"` via npm registry HTTP API |
| `ownership.rs` | File ownership model: Tool (overwrite) / Hybrid (merge) / User (never touch) |
| `safe_fs.rs` | Filesystem ops with automatic `.airis/backups/` |

### manifest.toml Key Sections

- `[workspace]` — name, package_manager, image, volumes
- `[apps.*]` / `[libs.*]` — workspace member definitions (for pnpm-workspace.yaml)
- `[[app]]` — project definitions with deps/scripts (generates per-project package.json)
- `[service.*]` — Docker Compose service definitions
- `[packages.catalog]` — version policies (`"latest"`, `"lts"`, `"^X.Y.Z"`, `{ follow = "pkg" }`)
- `[commands]` — user-defined CLI commands (`airis up`, `airis down`, `airis ps`)
- `[guards]` — Docker-first enforcement (`deny`, `forbid`, `danger`, `wrap`)
- `[remap]` — auto-translate banned commands to safe alternatives
- `[versioning]` — version strategy (conventional-commits, manual, auto)
- `[ci]` — GitHub Actions workflow generation

### `[[app]]` ProjectDefinition Fields

```toml
[[app]]
name = "my-app"
kind = "app"              # "app" | "lib" | "service"
path = "products/my-app"
scope = "@myorg"          # package name scope (default: @workspace)
description = "..."
framework = "node"        # "nextjs" | "react-vite" | "node" | "rust"
main = "dist/index.js"

[app.bin]
mycli = "dist/cli.js"

[app.scripts]
build = "tsup"
dev = "tsx watch src/cli.ts"

[app.deps]
zod = "catalog"           # resolved from [packages.catalog]
commander = "^13.1.0"     # pinned version

[app.dev_deps]
typescript = "catalog"
```

## Generated Docker Design

**No workspace container.** Each service container runs `pnpm install` during Docker build.

- **Dockerfile**: `COPY . .` → `RUN pnpm install --frozen-lockfile` → `ENTRYPOINT ["tini","--"]`
- **Compose (dev)**: bind mount `.:/app` overrides COPY, named volumes isolate dependencies
- **Compose (prod)**: no bind mount, COPY layer is used as-is
- **Default CLI commands**: `up`, `down`, `ps` only — no `shell`/`build`/`test`/`lint`

**Filesystem boundary rules:**
- Dependencies (node_modules, pnpm store) → named volumes, never on host
- Source code → bind mount `.:/app` (dev only)
- Build cache (.next, dist, .turbo) → named volumes (keeps host clean)
- Workspace node_modules → auto-generated `ws_nm_*` volumes per workspace path

## Invariants

1. **`airis init` NEVER overwrites manifest.toml** — no `--force` flag exists. If manifest.toml exists, it shows guidance only.
2. **Generated files are read-only** — all include `DO NOT EDIT` markers. Changes go through manifest.toml.
3. **Rust edition 2024** — maintain compatibility.
4. **Handlebars templates must produce valid output** — JSON, TOML, YAML syntax.

## Testing

- Unit tests: `#[cfg(test)]` modules colocated in source files
- Integration tests: `tests/` directory using `assert_cmd` + `predicates`
- Filesystem tests: use `tempfile` crate

**Thread-safe directory test pattern** (required when using `set_current_dir`):
```rust
use std::sync::Mutex;
static DIR_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_with_directory_change() {
    let _guard = DIR_LOCK.lock().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let result = std::panic::catch_unwind(|| { /* test logic */ });
    std::env::set_current_dir(original_dir).unwrap();
    result.unwrap();
}
```
