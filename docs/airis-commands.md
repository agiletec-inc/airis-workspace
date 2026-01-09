# Airis Commands Usage

**目的**: Agiletec モノレポ全体で `airis` CLI を使い、Docker ワークスペース上で安全に作業するためのガイド。

- **CLI ツール**: [airis-workspace](https://github.com/agiletec-inc/airis-workspace)
- **設定ファイル**: `manifest.toml`（唯一の設定ファイル）
- **読込方式**: `airis` が `manifest.toml` を直接読み込む（Rust TOML パーサー使用）

---

## 基本方針

1. **Docker-First**
   `airis` は常に `docker compose` / `workspace` コンテナを呼び出す。ホストで `pnpm install` や `docker compose up` を直接実行しない。

2. **単一の入口**
   `airis up` → `airis install` → `airis shell` が全作業の標準ルート。CLI から `pnpm` を叩きたい場合も必ず `airis shell` 内で行う。

3. **ガード内蔵**
   `pnpm` / `npm` / `yarn` などをホストで直接呼ぶと `airis guards` がエラーで落とす。常に `airis` コマンド経由で実行する。

4. **Manifest = manifest.toml**
   `packages.workspaces` / `apps.*` / `libs.*` / `dev.autostart` がすべてのアプリ情報・起動順・インフラ構成を定義する。

5. **自動生成**
   `pnpm-workspace.yaml`, `package.json`, GitHub workflows などは `manifest.toml` から自動生成される。手動編集は不要（`.airis/backups/` にバックアップあり）。

---

## 主要コマンド

### セットアップ & 起動

```bash
airis init                    # 既存プロジェクトを自動検出 + manifest.toml 生成 (dry-run)
airis init --write            # 検出結果を実行し manifest.toml を作成
airis init --skip-discovery   # 空テンプレートから作成 (従来モード)
airis generate files          # manifest.toml から workspace ファイル再生成
airis up                      # Traefik → Supabase → workspace を順に起動
airis down                    # dev サーバ → Supabase → Traefik の順に停止
airis install                 # workspace コンテナ内で pnpm install
airis shell                   # workspace シェルに入る（/app）
```

#### airis init の動作 (v1.43+)

1. **Discovery Phase**: apps/, libs/ をスキャンし、Next.js/Vite/Hono/Rust/Python を検出
2. **Compose Detection**: docker-compose.yml の場所を検出 (root, workspace/, supabase/, traefik/)
3. **Catalog Extraction**: package.json の devDependencies からカタログを抽出
4. **Migration Plan**: 変更内容をプレビュー表示
5. **Execute (`--write`)**: manifest.toml を生成、必要に応じてファイルを移動

### 開発

```bash
airis dev          # dev.autostart のアプリを自動起動
airis build        # 全アプリをビルド
airis test         # テスト実行
airis lint         # リント実行
```

### モニタリング

```bash
airis ps           # コンテナ一覧
airis logs         # 全サービスのログ tail
airis logs <app>   # 特定アプリのログ
```

### ユーティリティ

```bash
airis clean        # ビルドアーティファクト削除
airis validate     # 設定の検証
airis doctor       # 問題診断 & 修復
airis verify       # システムヘルスチェック
```

---

## 使い方の例

```bash
# 1. 初回セットアップ
airis init

# 2. Docker スタック起動
airis up

# 3. 依存同期
airis install

# 4. ワークスペースで作業
airis shell
pnpm lint
pnpm test

# 5. 片付け
airis down
```

> Traefik を外部プロキシ（Coolify など）に任せる場合は `SKIP_TRAEFIK=1 airis up` を指定するとローカル Traefik の起動をスキップできます。

### dev.autostart の更新

`manifest.toml` に以下のようなブロックを持たせると `airis dev` の起動対象が自動更新される。

```toml
[dev]
autostart = [
  "corporate-site",
  "airis-dashboard",
  "airis-auto-call",
]
```

---

## ベストプラクティス

### DO

- ✅ ルートで `airis` を実行し、`airis shell` 内で `pnpm` を叩く
- ✅ `airis up` 後に `airis verify` を実行し、Traefik/Kong/Supabase の疎通を確認
- ✅ 新しいアプリや設定は `manifest.toml` に追加 → `airis init` で再生成
- ✅ `airis clean` でビルドアーティファクトを定期的に掃除

### DON'T

- ❌ `pnpm install` をホストで直叩き（ガードで失敗する）
- ❌ `docker compose up` を直接叩き、Traefik や Kong をバラバラに起動する
- ❌ `.env` / `node_modules` をリポジトリに残す
- ❌ `package.json` や `pnpm-workspace.yaml` を手動編集（自動生成される）

---

## トラブルシューティング

### 問題: コンテナが起動しない

```bash
airis doctor --fix  # 自動修復
airis network setup # ネットワーク再構築
```

### 問題: 依存関係がおかしい

```bash
airis clean
airis install
```

### 問題: manifest.toml を更新したのに反映されない

```bash
airis init  # 再生成
```

---

## メンテナンス

1. `manifest.toml` を更新し、新しいアプリや設定を追加
2. `airis init` でワークスペースファイルを再生成
3. `git diff` で変更内容を確認
4. バックアップは `.airis/backups/` に自動保存

---

## 高度な機能

### Catalog による依存管理

```toml
[packages.catalog]
next = "latest"
react = "latest"
typescript = "latest"
```

複数アプリで共通の依存を catalog に集約することで、バージョン統一が簡単になります。

### カスタムコマンド

```toml
[commands]
up = "docker compose up -d"
dev = "docker compose exec workspace pnpm dev"
build = "docker compose exec workspace pnpm build"
```

`airis run <command>` で任意のコマンドを実行できます。

### Guards によるセキュリティ

```toml
[guards]
deny = ["npm", "yarn", "pnpm", "bun"]
deny_with_message = { "docker" = "Use 'airis' instead" }
```

危険なコマンドを自動でブロックし、正しい方法を案内します。

---

## 参考

- [airis-workspace GitHub](https://github.com/agiletec-inc/airis-workspace)
- [PNPM Catalog](https://pnpm.io/catalogs)
- [Docker Compose Spec](https://docs.docker.com/compose/compose-file/)
- [TOML Spec](https://toml.io/)
