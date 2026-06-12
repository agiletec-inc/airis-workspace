# Changelog

All notable changes to airis-workspace will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Breaking Changes

- **Binary renamed: `airis` → `airis-workspace` (v4.0.0).** The AIRIS suite
  moves to git-style dispatch: a thin `airis` dispatcher execs `airis-<tool>`
  binaries. Install the `airis` dispatcher and call `airis workspace <cmd>`
  (the binary also works standalone as `airis-workspace <cmd>`). Release
  archives were already named `airis-workspace-{target}`, so installer/asset
  naming is unchanged.
- **Docker wrapper subcommands removed (15):** `run`, `up`, `down`, `shell`,
  `test`, `lint`, `format`, `typecheck`, `ps`, `logs`, `exec`, `restart`,
  `network`, `status`, `init-shell`. Run `docker compose` and your native
  toolchain (pnpm/cargo/uv/...) directly. The auto-up machinery
  (`AIRIS_NO_AUTO_UP`) and pre-command hooks went with them.
- **MCP tools reduced 22 → 11.** Removed the Docker-wrapper tools
  (`workspace_up`, `workspace_down`, `workspace_restart`, `workspace_logs`,
  `workspace_install`, `workspace_run`, `workspace_exec`, `workspace_test`,
  `workspace_lint`, `workspace_typecheck`, `workspace_status`). Remaining:
  `workspace_init`, `workspace_cleanup`, `workspace_discover`,
  `manifest_validate`, `manifest_apply`, `migration_execute`,
  `workspace_gen`, `workspace_validate_all`, `workspace_doctor`,
  `workspace_verify`, `workspace_clean`.
- **Self-upgrade discontinuity.** Binaries older than this release look for
  release assets under the old `airis-{os}-{arch}` naming and cannot
  self-upgrade across the rename. Reinstall via Homebrew
  (`brew install agiletec-inc/tap/airis-workspace`), `cargo install
  airis-workspace`, or the installer script instead. From this release on,
  `upgrade` follows cargo-dist asset naming
  (`airis-workspace-{target-triple}`).

### Migration Guide (binary rename + wrapper removal)

- `airis clean` → `airis workspace clean` (all other surviving subcommands
  likewise: `airis gen` → `airis workspace gen`, `airis doctor` →
  `airis workspace doctor`, ...).
- Wrapper subcommands → run the underlying tools directly:
  `airis up`/`down`/`logs`/`ps`/`restart` → `docker compose up -d` / `down` /
  `logs` / `ps` / `restart`; `airis shell`/`exec` →
  `docker compose exec workspace <cmd>`; `airis test`/`lint`/`format`/
  `typecheck`/`run <task>` → the package-manager or toolchain command
  (e.g. `pnpm test`).
- `airis doctor --truth` (`recommended_commands`) now reports docker compose /
  package-manager commands instead of the removed wrappers.

### Breaking Changes (earlier, unreleased)

- **`airis init` CLI subcommand removed.** Initialization (repo scan → manifest.toml
  proposal) is now exclusively an LLM-assisted flow through the MCP server. Invoke
  `workspace_init` via Claude Code (or the `/airis:init` slash command), then
  `workspace_gen` / `airis gen` to materialize workspace files. Rationale: the
  discovery-to-manifest step needs format-preserving judgment (comments, ordering,
  catalog consolidation) that a TOML re-serializer cannot provide, while the
  deterministic manifest→files path belongs to the Rust CLI.

### Added

- MCP tools exposed by `airis mcp`: `workspace_gen`, `workspace_validate_all`,
  `workspace_doctor`, `workspace_verify`, `workspace_status` (in addition to the
  existing `workspace_init`, `workspace_cleanup`, `workspace_discover`,
  `manifest_validate`, `manifest_apply`, `migration_execute`).
### Removed

- Dead guard/shim remnants left behind by the May removal (472ec2e): the MCP
  server no longer advertises the broken `guards_install` / `guards_status` /
  `guards_uninstall` tools (they subprocess-called the deleted `airis guards`
  subcommand and failed at runtime), the unused `shim_commands` manifest field
  is gone (old manifests containing it still parse — unknown keys are ignored),
  `airis new` no longer emits a `[guards]` section, and stale
  `AIRIS_SKIP_GUARD` / `AIRIS_BYPASS` checks and shim docstrings were removed.
- `src/commands/init.rs` and `Commands::Init` CLI variant.
- `scripts/gif-recording/01-init-demo.tape` (corresponding demo).
- **`airis ui` subcommand, the `airis claude tab-title` shim, and the
  `[claude.terminal_title]` config section.** Managing a few entries in
  `~/.claude/settings.json` and two static shell scripts through a Rust
  install/uninstall command was over-engineered for a personal-config
  feature, and it produced three follow-up bugs (PR #258 → #264 → #265)
  tied to the generator round-trip. Maintain
  `~/.claude/hooks/airis-tab-title.sh`, `~/.claude/statusline-command.sh`,
  and the `hooks` / `statusLine` entries in `~/.claude/settings.json`
  directly (or via a dotfiles manager). Bake the emoji into the hook
  command's `args` instead of `[claude.terminal_title]`.

### Migration Guide

Replace any scripted `airis init` or `airis init --write` invocation with one of:

- Interactive: open Claude Code in the repo and run `/airis:init` (or directly call
  the `workspace_init` MCP tool through any MCP-aware agent).
- Scripted: author `manifest.toml` by hand (see `docs/manifest.md`) and run
  `airis gen`.


## [4.0.1] - 2026-04-13

### Breaking Changes

- **`airis gen` no longer generates `compose.yml`.** Docker Compose files are now project-owned and must be hand-edited. This completes the direction started in 31a0599 (which stopped generating `Dockerfile` and `.github/workflows/*`) and unblocks language runtimes (Python, Rust, Go) and compose features (custom healthchecks, `env_file`, `entrypoint`, `depends_on` conditions) that could not fit a uniform schema.
- **`airis gen` no longer generates `.env.example`.** The `[env]` section of `manifest.toml` still documents required/optional variables, but the companion example file is user-owned.
- **File ownership change:** `compose.yml`, `Dockerfile`, and `.env.example` are now `Ownership::User` (previously `Ownership::Tool`). `airis doctor` and `airis diff` no longer check drift against a generated compose.

### Removed

- `src/templates/compose.rs` and `src/commands/generate/docker_gen.rs`
- `src/templates/env.rs` and `src/commands/generate/env_gen.rs`
- `TemplateEngine::render_docker_compose` / `build_compose_file` / `prepare_docker_compose_data` / `render_env_example`
- `detect_legacy_compose_files()` and the `--migrate` legacy compose migration path in `airis gen`

### Migration Guide

Existing projects with a generated `compose.yml`:
1. Run `git add compose.yml` to pin the current generated content as your starting point.
2. Remove the `# Generated by airis gen - DO NOT EDIT MANUALLY` header and treat the file as yours.
3. If `airis gen` shows orphan cleanup output for `compose.yml` or `.env.example`, your `.airis/generated.toml` still lists them — delete those entries (or let the next orphan cleanup pass drop them; the files themselves stay on disk).

## [2.0.0] - 2026-04-06

### Breaking Changes
- `airis guards install --hooks` is deprecated; use `airis claude setup` instead
- `airis sync-deps` removed (use `airis init`)

### Added
- `airis claude setup/status/uninstall` — Claude Code integration as top-level command
- `airis policy check/apply` — Policy gates for testing, security, deployment
- `airis test --scan` — Test quality governance (forbidden patterns, type enforcement)
- `airis build --docker --affected` — Multi-target Docker builds with change detection
- `airis bundle` — Deployment bundle generation
- `airis deps tree/check` — Dependency graph visualization and architecture validation
- `airis diff` — Preview changes between manifest.toml and generated files
- `airis new` — Project scaffolding from templates
- Global guards (`~/.airis/bin/`) for host-wide command blocking
- Registry-based file sync for Claude Code configuration

### Changed
- Guards command now exclusively manages PATH-based command blocking
- Ownership model: CLI owns specs (CLAUDE.md, rules/), plugin owns hooks

## [1.70.0] - 2026-03-02

### Removed

#### Breaking Changes
- **`airis sync-deps` command removed**: This command has been deprecated since v1.43.0.
  - Use `airis init` instead, which resolves catalog versions automatically
  - Version resolution functions moved to `src/version_resolver.rs` module
  - The `--migrate` flag functionality is no longer available

### Changed
- Refactored version resolution logic into standalone `version_resolver` module
- Internal: Improved code organization by separating npm version resolution from CLI command handling

### Migration Guide
If you were using `airis sync-deps`:
```bash
# Before (deprecated)
airis sync-deps

# After
airis init        # resolves catalog versions during initialization
airis gen  # regenerates workspace files with resolved versions
```

## [1.43.0] - 2025-01-09

### Added

#### Auto-Migration Feature
- **`airis init` Auto-Discovery**: Automatically scans existing projects
  - Detects apps in `apps/` directory (Next.js, Vite, Hono, Node, Rust, Python)
  - Detects libraries in `libs/` directory
  - Finds docker-compose.yml files (root, workspace/, supabase/, traefik/)
  - Extracts catalog from root package.json devDependencies

- **Safe Migration**: Moves files with automatic backups
  - Creates `workspace/` directory if needed
  - Moves root docker-compose.yml to workspace/
  - Creates `.bak` backups before any file moves
  - Never overwrites existing files

#### New CLI Options
```bash
airis init                    # Auto-discover & show migration plan (dry-run)
airis init --write            # Execute migration
airis init --skip-discovery   # Use empty template (legacy mode)
```

#### New Modules
- `src/commands/discover.rs` - Project discovery with framework detection
- `src/commands/migrate.rs` - Safe migration with backup creation

### Changed
- `airis init` now runs discovery by default (use `--skip-discovery` for template mode)
- Updated README.md with Auto-Discovery documentation
- Updated CLAUDE.md to reflect implemented features

## [1.0.2] - 2025-01-17

### Added

#### Core Features
- **`airis` command統一**: `just` 依存を内部化し、`airis` コマンドだけで完結する UX を実現
- **`[commands]` セクション**: manifest.toml でユーザー定義コマンドを管理
  - `airis run <task>` で任意のコマンドを実行
  - 頻出コマンドは `airis up`, `airis dev`, `airis shell` などのショートハンドで提供
- **`[guards]` セクション**: LLM 向けコマンド制御
  - `forbid`: LLM に対して完全禁止するコマンドリスト（`npm`, `pnpm`, `docker` など）
  - `danger`: 危険コマンドのブロック（`rm -rf /`, `chmod -R 777` など）
- **`[remap]` セクション**: LLM コマンドの自動リマップ
  - `"npm install"` → `"airis install"` のような自動変換
  - LLM が禁止コマンドを叩こうとしても manifest.toml で強制的に安全なコマンドに変換

#### CLI Commands
```bash
airis run <task>      # manifest.toml [commands] から実行
airis up              # Docker services 起動
airis down            # Docker services 停止
airis shell           # コンテナシェルに入る
airis dev             # 開発サーバー起動
airis test            # テスト実行
airis install         # 依存インストール
airis build           # ビルド
airis clean           # ビルド成果物削除
```

#### Manifest Schema Extensions
- `Manifest.commands: IndexMap<String, String>` - ユーザー定義コマンド
- `Manifest.remap: IndexMap<String, String>` - LLM コマンドリマップ
- `GuardsSection.forbid: Vec<String>` - LLM 禁止コマンド
- `GuardsSection.danger: Vec<String>` - 危険コマンド
- `OrchestrationSection` - マルチ compose ファイル対応の準備

### Changed
- **依存の内部化**: `just` を直接呼び出さず、`airis` が manifest.toml の `[commands]` を解釈して実行
- **UX の統一**: すべてのワークフロー操作を `airis` コマンド配下に集約

### Philosophy
この変更により、airis-workspace は単なる「モノレポツール」から「**開発環境ポリシーエンジン**」へと進化：

- **人間向け**: `[commands]` に定義すれば誰でも従う（便利だから）
- **LLM向け**: `[guards]` + `[remap]` で強制的に安全なコマンドに変換
- **manifest.toml = 唯一の真実**: コマンド、ポリシー、依存、構造すべてが一元管理

## [0.4.0] - 2025-01-16

### Added
- Full package.json generation from manifest.toml
- Project-level scripts and dependencies management
- Multi-compose orchestration support (`[orchestration.dev]`)

## [0.3.0] - 2025-01-15

### Added
- Catalog version policy resolution (`airis sync-deps`)
- Support for "latest" and "lts" policies
- Auto-update pnpm-workspace.yaml catalog

## [0.2.1] - 2025-01-14

### Added
- Auto-discovery of apps/libs directories
- Docker-first command guards
- Project structure detection

## [0.1.0] - 2025-01-13

### Added
- Initial release
- Basic manifest.toml support
- Template generation (justfile, package.json, pnpm-workspace.yaml)
- `airis init` command

---

[1.0.2]: https://github.com/agiletec-inc/airis-workspace/releases/tag/v1.0.2
[0.4.0]: https://github.com/agiletec-inc/airis-workspace/releases/tag/v0.4.0
[0.3.0]: https://github.com/agiletec-inc/airis-workspace/releases/tag/v0.3.0
[0.2.1]: https://github.com/agiletec-inc/airis-workspace/releases/tag/v0.2.1
[0.1.0]: https://github.com/agiletec-inc/airis-workspace/releases/tag/v0.1.0
