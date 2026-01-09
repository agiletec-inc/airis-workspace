# Changelog

All notable changes to airis-workspace will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
