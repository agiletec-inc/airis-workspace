# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**AIris Workspace** is a Docker-first monorepo workspace manager built in Rust. It enforces Docker-first development by auto-generating `justfile`, `package.json`, and `pnpm-workspace.yaml` from a single `MANIFEST.toml`. `workspace.yaml` is generated metadata, not the user-editable manifest.

**Core Philosophy**: Prevent host pollution by blocking direct `pnpm`/`npm`/`yarn` execution and forcing Docker-first workflow. Special exception for Rust projects (local builds for GPU support).

## Build & Development Commands

### Building the CLI
```bash
# Build debug binary
cargo build

# Build release binary
cargo build --release

# Install locally (for testing)
cargo install --path .

# Run tests
cargo test
```

### Testing the CLI
```bash
# Test init command (creates MANIFEST.toml + workspace metadata)
cargo run -- init

# Test with force flag
cargo run -- init --force

# Validate MANIFEST + workspace metadata
cargo run -- validate
```

## Architecture & Code Structure

### Configuration Flow (MANIFEST.toml → Generated Files)

1. **MANIFEST.toml** (user-editable)
   - Parsed via `toml`
   - Describes dev apps, infra services, lint/test rules, package config (`src/manifest.rs`)

2. **workspace.yaml** (auto-generated metadata)
   - Derived from MANIFEST.toml for IDE/tooling compatibility (`src/config/mod.rs`)

3. **Template Engine** (`src/templates/mod.rs`)
   - Uses Handlebars for templating with MANIFEST-driven data
   - Generates `justfile`, `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`

4. **Generation Pipeline**
   - `init` command → creates or loads MANIFEST.toml, then triggers template sync (src/commands/init.rs)
   - `commands::generate` module → helper invoked by `init` that syncs workspace.yaml + templates (src/commands/generate.rs)

### Key Design Patterns

**Docker-First Guards**: Generated justfile contains guard recipes that block host-level `pnpm`/`npm`/`yarn` with helpful error messages (templates/mod.rs:227-247). This is the project's core enforcement mechanism.

**Runtime Exceptions**: `apps[].runtime` field allows "local" builds (e.g., Rust with GPU support). Default is "docker".

**Catalog System (NEW DESIGN)**:
- `catalog` セクションでバージョンポリシー（`policy = "latest"` | `"lts"` | `"^X.Y.Z"`）を定義
- `airis workspace sync-deps` コマンドで npm registry から実際のバージョンを解決
- package.json の `pnpm.catalog` に数字を書き込む
- Dependencies は `"dep": "catalog:"` で catalog を参照
- **人間が編集するのは MANIFEST.toml だけ、package.json は生成物**

**Design Philosophy**:
- **Avoid hardcoded version numbers** in MANIFEST.toml
- Use version policies (`latest`, `lts`) instead
- Auto-resolve to actual versions at `sync-deps` time
- Lock files maintain reproducibility

**Auto-Generation Markers**: All generated files include `DO NOT EDIT` warnings and `_generated` metadata to prevent manual edits.

### Module Responsibilities

- **src/main.rs**: CLI entry point using `clap` derive macros
- **src/config/mod.rs**: Workspace YAML schema + helpers (generated metadata)
- **src/manifest.rs**: MANIFEST.toml schema/helpers
- **src/commands/init.rs**: Creates or reloads MANIFEST.toml, then re-syncs derived files
- **src/commands/generate.rs**: Helper that syncs workspace.yaml + templates from an in-memory Manifest
- **src/commands/manifest_cmd.rs**: Implements `airis manifest ...` plumbing for justfile
- **src/templates/mod.rs**: Handlebars engine driven by MANIFEST data

## Important Constraints

### DO NOT violate these rules when making changes:

1. **Generated files must remain read-only**: Never encourage users to edit `justfile`, `package.json`, or `pnpm-workspace.yaml` directly. All changes go through `MANIFEST.toml`.

2. **Docker-first is non-negotiable**: Do not weaken guard recipes or suggest host-level package manager usage (except for Rust projects with `runtime: local`).

3. **Rust edition is 2024**: Cargo.toml specifies `edition = "2024"` (line 4). Maintain compatibility.

4. **Template consistency**: When modifying templates, ensure:
   - Handlebars syntax is valid
   - Generated files include auto-generation warnings
   - Just recipes follow naming convention: `<action>-<type>` (e.g., `dev-next`, `build-rust`)

## Configuration Schema Notes

**Mode types** (src/config/mod.rs:30-34):
- `docker-first`: Default. Allows local builds with explicit `runtime: local`
- `hybrid`: (not yet implemented)
- `strict`: (not yet implemented)

**WorkspaceApp variants** (src/config/mod.rs:52-59):
- `Simple(String)`: Just app name (type inferred from `apps` section)
- `Detailed`: Inline type specification

**App runtime resolution**: Keep Docker-first semantics. Runtime overrides (e.g., Rust local builds) should be modeled in MANIFEST extensions rather than reintroducing host-level exceptions elsewhere.

## Testing Strategy

When adding features:
1. Add unit tests in `#[cfg(test)]` modules (see examples in config/mod.rs:242-270, commands/generate.rs:92-118)
2. Use `tempfile` crate for filesystem tests (already in dev-dependencies)
3. Test YAML parsing/serialization roundtrips
4. Verify template rendering produces valid output (justfile syntax, valid JSON)

## Future Implementation Notes

**Planned but not yet implemented** (README.md:162-171):
- Environment variable validation
- LLM context generation
- MCP server integration
- Migration from existing projects

**New Features (Catalog Version Policy)**:
- [ ] Add `catalog` section to Manifest struct (src/manifest.rs)
  - `catalog.<package>.policy = "latest" | "lts" | "^X.Y.Z"`
- [ ] Implement `airis workspace sync-deps` command
  - Query npm registry for latest/lts versions
  - Resolve policy → actual version number
  - Write to package.json `pnpm.catalog`
- [ ] Update template generation
  - Generate package.json with `pnpm.catalog` from resolved versions
  - Add `_generated.from = "MANIFEST.toml"` marker

**Implementation Priority**:
1. Schema addition (manifest.rs) - Define CatalogSection struct
2. npm registry client - Query API for version info
3. sync-deps command - Main logic for version resolution
4. Template updates - package.json generation with catalog

**New Features (Auto-Migration)**:
- [ ] Project discovery module (src/commands/discover.rs)
  - Scan apps/ directory → detect Next.js/Node/Rust apps
  - Scan libs/ directory → detect TypeScript libraries
  - Find docker-compose.yml locations (root, supabase/, traefik/, etc.)
  - Parse existing package.json → extract catalog info
- [ ] Safe migration module (src/commands/migrate.rs)
  - Move docker-compose.yml to correct locations (NEVER overwrite)
  - Create workspace/ directory if missing
  - Warn user if file already exists at target location
- [ ] Enhanced init command
  - Run discovery → migration → generation flow
  - Generate manifest.toml from detected project structure
  - Display changes and ask for confirmation (unless --force)
  - User just runs `airis init` and everything is optimized

**Auto-Migration Workflow**:
```
airis init
  ↓
1. Discovery Phase
   - Scan apps/, libs/
   - Detect docker-compose.yml locations
   - Parse package.json catalog
  ↓
2. Migration Phase (safe, no overwrites)
   - Create workspace/ if missing
   - Move root/docker-compose.yml → workspace/docker-compose.yml
   - Validate supabase/docker-compose.yml, traefik/docker-compose.yml
  ↓
3. Generation Phase
   - Generate manifest.toml with:
     - Detected apps/libs
     - Detected compose file paths in orchestration.dev
     - Extracted catalog from package.json
   - Generate workspace.yaml, justfile, etc.
  ↓
4. Verification Phase
   - Show diff/changes
   - Ask confirmation (unless --force)
   - Save files
```

**Safety Rules**:
- NEVER overwrite existing files without user confirmation
- ALWAYS create backups before migration (.bak suffix)
- ALWAYS warn user if target file exists
- Prefer moving files over copying (preserve git history)

Do not implement these features without checking the current project roadmap.
