# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**AIris Workspace** is a Docker-first monorepo workspace manager built in Rust. It enforces Docker-first development by auto-generating `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`, and `Cargo.toml` from a single `manifest.toml`.

**Core Philosophy**: Prevent host pollution by blocking direct `pnpm`/`npm`/`yarn` execution and forcing Docker-first workflow via `airis` CLI commands. Special exception for Rust projects (local builds for GPU support).

**Current Version**: v1.41.0
- v1.0.2: Command unification (`[commands]`, `[guards]`, `[remap]`) - justfile now optional
- v1.1.0: Version automation (`[versioning]`, `airis bump-version`, Git hooks)
- v1.2.0: Removed workspace.yaml (unused), simplified init command

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
# Test init command (creates manifest.toml + workspace metadata)
cargo run -- init

# Validate MANIFEST + workspace metadata
cargo run -- validate
```

## ⚠️ airis init Specification (Strictly Enforced)

**manifest.toml is sacred and inviolable.**

The `airis init` command must NEVER overwrite manifest.toml.

### Behavior

1. **If manifest.toml exists**
   - Display guidance message (directing user to `airis generate files`)
   - Never modify manifest.toml itself

2. **If manifest.toml does not exist**
   - Default is dry-run (preview display)
   - Use `--write` option to create template

3. **No mechanism exists to overwrite manifest.toml**
   - No `--force` flag exists
   - No `--reset` command exists
   - CLI provides no means to delete or overwrite manifest.toml

### Command Examples

```bash
# No manifest.toml: preview display
airis init

# No manifest.toml: create template
airis init --write

# manifest.toml exists: regenerate files
airis generate files
```

**Any implementation violating this specification is treated as a bug.**

## Architecture & Code Structure

### Configuration Flow (manifest.toml → Generated Files)

1. **manifest.toml** (user-editable, Single Source of Truth)
   - Parsed via `toml`
   - Describes dev apps, infra services, lint/test rules, package config (`src/manifest.rs`)

2. **Template Engine** (`src/templates/mod.rs`)
   - Uses Handlebars for templating with MANIFEST-driven data
   - Generates `package.json`, `pnpm-workspace.yaml`, `docker-compose.yml`, `Cargo.toml`

3. **Generation Pipeline**
   - `init` command → creates manifest.toml template if not exists (src/commands/init.rs)
   - `generate files` command → regenerates workspace files from manifest.toml (src/commands/generate.rs)

### Key Design Patterns

**Docker-First Guards (v1.0.2+)**: `[guards]` section in manifest.toml defines:
- `deny`: Block commands for all users (e.g., `["npm", "yarn", "pnpm"]`)
- `forbid`: LLM-specific blocking (via MCP integration)
- `danger`: Prevent catastrophic commands (e.g., `["rm -rf /"]`)

**Command Unification (v1.0.2+)**: All operations via `airis` CLI:
- `[commands]` section defines user commands (install, up, down, dev, test, build, clean)
- `airis run <task>` executes commands from manifest.toml
- Built-in shorthands: `airis up`, `airis dev`, `airis shell`, etc.
- `[remap]` auto-translates banned commands to safe alternatives

**Version Automation (v1.1.0+)**: Automatic version bumping:
- `[versioning]` section with `strategy` (conventional-commits, auto, manual)
- `airis bump-version` command (--major, --minor, --patch, --auto)
- Git pre-commit hook for auto-bump on commit
- Syncs manifest.toml ↔ Cargo.toml

**Runtime Exceptions**: `apps[].runtime` field allows "local" builds (e.g., Rust with GPU support). Default is "docker".

**Catalog System** (integrated into `airis init`):
- Define version policies in `[packages.catalog]`: `"latest"`, `"lts"`, `"^X.Y.Z"`, or `{ follow = "package" }`
- `airis init` resolves policies to actual versions via npm registry
- Writes resolved versions to `pnpm-workspace.yaml` catalog section
- Dependencies use `"catalog:"` reference in package.json
- **Only manifest.toml is human-edited; package.json is generated**

**Auto-Generation Markers**: All generated files include `DO NOT EDIT` warnings and `_generated` metadata to prevent manual edits.

### Module Responsibilities

- **src/main.rs**: CLI entry point using `clap` derive macros
- **src/manifest.rs**: manifest.toml schema/helpers + Mode enum
- **src/commands/init.rs**: Creates manifest.toml template (if not exists)
- **src/commands/generate.rs**: Regenerates workspace files from manifest.toml
- **src/commands/manifest_cmd.rs**: Implements `airis manifest ...` plumbing
- **src/commands/run.rs**: Executes commands from `[commands]` section (v1.0.2+)
- **src/commands/bump_version.rs**: Version bumping with Conventional Commits (v1.1.0+)
- **src/commands/hooks.rs**: Git hooks installation (v1.1.0+)
- **src/commands/sync_deps.rs**: Catalog version resolution (deprecated, now in init)
- **src/templates/mod.rs**: Handlebars engine driven by MANIFEST data

## Important Constraints

### DO NOT violate these rules when making changes:

1. **Generated files must remain read-only**: Never encourage users to edit `package.json`, `pnpm-workspace.yaml`, or `Cargo.toml` directly. All changes go through `manifest.toml`. (justfile is optional in v1.0.2+)

2. **Docker-first is non-negotiable**: Do not weaken guard recipes or suggest host-level package manager usage (except for Rust projects with `runtime: local`).

3. **Rust edition is 2024**: Cargo.toml specifies `edition = "2024"`. Maintain compatibility.

4. **Template consistency**: When modifying templates, ensure:
   - Handlebars syntax is valid
   - Generated files include auto-generation warnings
   - Commands follow naming convention: `<action>` (e.g., `dev`, `build`, `test`)

## Configuration Schema Notes

**Mode types** (src/manifest.rs):
- `docker-first`: Default. Allows local builds with explicit `runtime: local`
- `hybrid`: (not yet implemented)
- `strict`: (not yet implemented)

**App runtime resolution**: Keep Docker-first semantics. Runtime overrides (e.g., Rust local builds) should be modeled in MANIFEST extensions rather than reintroducing host-level exceptions elsewhere.

## Testing Strategy

When adding features:
1. Add unit tests in `#[cfg(test)]` modules (see examples in commands/generate.rs, commands/bump_version.rs)
2. Use `tempfile` crate for filesystem tests (already in dev-dependencies)
3. Test TOML parsing/serialization roundtrips
4. Verify template rendering produces valid output (valid JSON, TOML, YAML)

## Future Implementation Notes

**Planned but not yet implemented** (see README.md Phases 4-6):
- Environment variable validation
- LLM context generation
- MCP server integration
- Migration from existing projects

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
  - User runs `airis init` and everything is optimized

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
   - Generate package.json, pnpm-workspace.yaml, docker-compose.yml
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

These features are planned but not yet implemented. Confirm with the user before starting implementation.
