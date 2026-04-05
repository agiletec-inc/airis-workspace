# Docker-First Development — 絶対ルール

## 原則: manifest.toml → airis gen → airis up

これだけ。手で Dockerfile や compose.yml を直すな。

1. `manifest.toml` を編集する（唯一の設定ファイル）
2. `airis gen` で Dockerfile, compose.yml, CI設定を再生成する
3. `airis up` でビルド＋起動（`pnpm install` は Docker ビルド時に自動実行）
4. 問題が出たら **manifest.toml か airis-monorepo のテンプレートを直す**

「手で直した方が早い」は禁止。それは技術的負債の先送り。

## エラーが出たときの対処フロー

**場当たり的に手で直すな。根本原因を突き止めろ。**

```
エラー発生
  ├→ 自分の設定ミス？ → manifest.toml を直して airis gen
  ├→ テンプレートのバグ？ → airis-monorepo のテンプレートを修正
  ├→ Dockerfile の構成問題？ → manifest.toml の [docker] セクションを修正して airis gen
  └→ 一時的な回避策で push → 絶対禁止。CI で落ちる。時間の無駄。
```

## 絶対禁止リスト

### ホストでの実行禁止（Bash hook でブロック済み）

```
pnpm install / npm install / yarn install / pnpm add
node / npx / tsx / ts-node
pip install / python -m pip
```

### ファイル編集での禁止パターン（Edit/Write hook でブロック済み）

| 禁止 | 理由 | 正しい方法 |
|------|------|-----------|
| Dockerfile に `/Users/...` や `/home/...` を書く | ホスト依存パス。CI で通らない | コンテナ内パスのみ使う |
| `PNPM_STORE_DIR=./.pnpm-store` | ホストに漏れる | `PNPM_STORE_DIR=/pnpm/store` (named volume) |
| compose.yml に `./node_modules:/app/node_modules` | bind mount でホスト汚染 | `node_modules:/app/node_modules` (named volume) |
| `airis gen` 生成ファイル（`_generated` / `DO NOT EDIT` ヘッダー付き）の手動編集 | airis gen で上書きされる | manifest.toml を変更して `airis gen` |

### hooks の回避禁止

```
--no-verify
git config core.hooksPath を変更
テストをスキップして push
CI が通る前に「完了」と報告
```

## パスのルール

### コンテナ内で使うべきパス

```
/pnpm/store          — pnpm グローバルストア (named volume)
/app/node_modules    — ルート依存 (named volume)
/app                 — ソースコード (COPY)
```

### 絶対に使ってはいけないパス

```
/Users/*             — macOS ホストパス
/home/*              — Linux ホストパス
~/*                  — ホームディレクトリ展開
./.pnpm-store        — ローカル pnpm store
./node_modules       — bind mount としての node_modules
```

## CI との整合性

**ローカルで通って CI で通らないのは致命的バグ。** 原因の99%は:

1. ホスト固有パスの混入（`/Users/...`）
2. bind mount 前提の構成（CI には bind mount がない）
3. `PNPM_STORE_DIR` がローカルパスを指している
4. named volume に入るべきものが bind mount 経由になっている

これらは全て **hook でブロックされる**。ブロックされたら回避策を探すのではなく、根本原因を直せ。

## パッケージの追加・更新

```bash
# manifest.toml の [packages.catalog] に追加して:
airis gen    # package.json 再生成
airis up     # Docker ビルドで install

# または直接（コンテナ内で）:
docker compose exec <service> pnpm add <package>
```

## ホストで実行してよいコマンド

```
airis up/down/ps/test/lint/typecheck/build/gen/clean
git / gh / docker compose / doppler / supabase
ファイル編集
```

それ以外は `docker compose exec <service> <command>` を使う。

## 完了の定義

1. `airis test` がエラー0
2. `airis gen` で生成されたファイルと手動変更が矛盾していない
3. ホスト固有パスが一切含まれていない
4. CI で通ることが確認できている（push 後 `gh run watch`）
