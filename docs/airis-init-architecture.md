# airis init Architecture

## 概要

`airis init` は manifest.toml をソースオブトゥルースとして、ワークスペースファイルを生成する **READ-ONLY** コマンドです。

## UX（ユーザー視点）

```bash
# 新規プロジェクト or 既存プロジェクトの移行
airis init                    # 自動検出 + プレビュー (dry-run)
airis init --write            # 実行して manifest.toml 作成
airis init --skip-discovery   # 空テンプレートから作成

# manifest.toml 更新後
airis generate files          # ワークスペースファイル再生成
```

**ユーザーは `airis init` で自動検出 → manifest.toml 生成 → `airis generate files` で反映。**

## 実装アーキテクチャ（READ-ONLY モード）

### READ-ONLY の意味

- **manifest.toml → 生成ファイル**: ✅ 実装済み
- **ファイルシステム → manifest.toml**: ❌ 実装しない（LLM に任せる）

### Why READ-ONLY?

**問題**: TOML の re-serialization でコメント・フォーマットが消失

```toml
# === Apps Configuration ===  ← このコメントが消える
[apps.dashboard]
path = "apps/dashboard"         ← インデントが変わる
type = "nextjs"
```

**解決**: 責務分離

- **Rust CLI (`airis init`)**: manifest.toml → files 生成（高速・確実）
- **LLM (`/airis:init`)**: filesystem → manifest.toml 更新（柔軟・賢い）

---

## Phase 0: Backup（準備）

既存ファイルを退避：

```
.airis/backups/
├── package.json.latest
├── pnpm-workspace.yaml.latest
├── .github_workflows_ci.yml.latest
└── apps.dashboard.package.json.latest
```

**Ownership システム**:
- **Tool-owned**: pnpm-workspace.yaml, workflows → 完全再生成
- **Hybrid**: package.json → マージ
- **User-owned**: manifest.toml, workspace/ → 絶対に触らない

---

## Phase 1: Discover（既存構成スキャン）✅ 実装済み

manifest.toml が存在しない場合、既存リポジトリを自動スキャンして manifest.toml を生成：

**スキャン対象**:
- `apps/*/package.json` → アプリ検出 + フレームワーク判定
- `libs/*/package.json` → ライブラリ検出
- `apps/*/Cargo.toml` → Rust 判定
- `apps/*/pyproject.toml` → Python 判定
- `docker-compose.yml` → 場所検出 (root, workspace/, supabase/, traefik/)
- `package.json` devDependencies → カタログ抽出

**フレームワーク検出ロジック**:
- `next` dependency → Next.js
- `vite` dependency → Vite
- `hono` dependency → Hono
- `Cargo.toml` 存在 → Rust
- `pyproject.toml` 存在 → Python
- それ以外 → Node

**実装**: `src/commands/discover.rs`

```rust
pub struct DiscoveryResult {
    pub apps: Vec<DetectedApp>,
    pub libs: Vec<DetectedLib>,
    pub compose_files: Vec<DetectedCompose>,
    pub catalog: IndexMap<String, String>,
}

pub enum Framework {
    NextJs, Vite, Hono, Node, Rust, Python, Unknown
}
```

---

## Phase 2: manifest.toml 読み込み

**READ-ONLY**: 既存の manifest.toml をそのまま読み込む（変更しない）

```rust
let manifest = if manifest_path.exists() {
    Manifest::load(manifest_path)?  // 読み込むだけ
} else {
    // 初回のみ: discover から生成
    create_manifest_from_discovery(&project_name, discovered, &current_dir)
};
```

**実装**: `src/manifest.rs`

---

## Phase 3: Generate（ファイル生成）

manifest.toml から各種ファイルを生成：

**生成ファイル**:

1. **package.json** (Hybrid)
   - catalog 参照を展開
   - workspaces 定義
   - scripts, engines を追加

2. **pnpm-workspace.yaml** (Tool-owned)
   - workspaces パターン
   - catalog 定義

3. **.github/workflows/ci.yml** (Tool-owned)
   - CI ステップ自動生成

4. **.github/workflows/release.yml** (Tool-owned)
   - リリースワークフロー

**実装**: `src/commands/generate.rs`

```rust
pub fn sync_from_manifest(manifest: &Manifest) -> Result<()> {
    let resolved_catalog = resolve_catalog_versions(&manifest.packages.catalog)?;
    generate_package_json(&manifest, &engine, &resolved_catalog)?;
    generate_pnpm_workspace(&manifest, &engine)?;
    if manifest.ci.enabled {
        generate_github_workflows(&manifest, &engine)?;
    }
    Ok(())
}
```

---

## Phase 4: Verify（整合性チェック）

生成されたファイルの検証：

**実装**: `src/commands/validate_cmd.rs`

```bash
airis validate all          # 全チェック
airis validate ports        # ports: mapping チェック
airis validate networks     # Traefik ネットワークチェック
airis validate env          # 環境変数チェック
airis validate deps         # 依存関係アーキテクチャチェック
```

**チェック項目**:
- ports: mapping が apps/ に存在しないか
- Traefik ネットワーク設定が正しいか
- 公開環境変数が許可リストに含まれるか
- アプリ間の依存が正しいか（apps → libs のみ）

---

## Bidirectional Sync（双方向同期）

### `/airis:init` (Claude Code Command)

manifest.toml の更新は LLM に任せる：

```typescript
// airis-agent MCP ツール sync_manifest を使用
const result = await use_mcp_tool("airis-agent", "sync_manifest", {
  repo_path: process.cwd(),
  dry_run: false,
  apply_guidelines: true  // Catalog migration など
});
```

**機能**:
1. ファイルシステムスキャン
2. manifest.toml 更新（テキストベース編集、コメント保持）
3. Airis ベストプラクティス適用：
   - Catalog migration（重複依存を統合）
   - Dependency consolidation
   - Version unification
4. `airis init` 自動実行

**実装**: `airis-agent/src/airis_agent/api/sync_manifest.py`

---

## CI vs ローカルの分岐

```rust
pub fn run(force_snapshot: bool, no_snapshot: bool) -> Result<()> {
    let should_snapshot = !no_snapshot && (!snapshots_exist || force_snapshot);

    if should_snapshot {
        snapshot::capture_snapshots()?;
    }

    let manifest = if manifest_path.exists() {
        Manifest::load(manifest_path)?
    } else {
        // 初回のみ discover
        let discovered = discover::discover_project(&current_dir)?;
        create_manifest_from_discovery(&project_name, discovered, &current_dir)
    };

    generate::sync_from_manifest(&manifest)?;
    Ok(())
}
```

**CI モード** (`--no-snapshot`):
- スナップショット不要（CI は manifest を信じる）
- discover スキップ（manifest のみ使用）

---

## Bug Fixes（既に修正済み）

### ✅ 1. HTMLエンティティエスケープ

**修正**: serde_json で直接シリアライズ（エスケープなし）

### ✅ 2. カタログ定義欠落

**修正**: pnpm-workspace.yaml に catalog セクション追加

### ✅ 3. Docker Volume権限

**修正**: Dockerfile.dev で事前作成

```dockerfile
RUN mkdir -p /app/{node_modules,.pnpm-store,.next,dist,build,out} && \
    chown -R app:app /app
```

---

## Airis Suite 連携

**airis-workspace (Rust CLI)**:
- manifest.toml → files 生成（READ-ONLY）
- 高速・確実・予測可能

**airis-agent (Python MCP)**:
- filesystem → manifest.toml 更新
- 賢い判断・catalog 移行・コメント保持

**airis-mcp-gateway (Python)**:
- MCP サーバーバンドラー
- airis-agent を含む複数 MCP を統合

---

## 参考

- [PNPM Catalog](https://pnpm.io/catalogs)
- [Docker Compose Spec](https://docs.docker.com/compose/compose-file/)
- [TOML Spec](https://toml.io/)
- [MCP Protocol](https://modelcontextprotocol.io/)
