# Deployment Guide

## 自動デプロイの設定方法

### 前提条件

- GitHub リポジトリ: `agiletec-inc/airis-workspace`
- Homebrew Tap リポジトリ: `agiletec-inc/homebrew-tap`
- GitHub Personal Access Token（`repo` スコープ付き）

### 1. GitHub Personal Access Token の作成

1. GitHub にログイン
2. Settings → Developer settings → Personal access tokens → Tokens (classic)
3. "Generate new token (classic)" をクリック
4. Note: `HOMEBREW_TAP_TOKEN` (識別用の名前)
5. Expiration: 推奨は "No expiration"（長期運用の場合）
6. Select scopes: **`repo`** にチェック（フルアクセス）
7. "Generate token" をクリック
8. **トークンをコピー**（この画面を閉じると二度と表示されない）

### 2. GitHub Secrets に追加

1. `agiletec-inc/airis-workspace` リポジトリにアクセス
2. Settings → Secrets and variables → Actions
3. "New repository secret" をクリック
4. Name: `HOMEBREW_TAP_TOKEN`
5. Secret: コピーしたトークンを貼り付け
6. "Add secret" をクリック

### 3. デプロイの動作確認

#### 自動デプロイ（推奨）

Conventional Commits でコミットすると自動的にバージョンが更新されます：

```bash
# 機能追加 → minor bump
git commit -m "feat: add new feature"

# バグ修正 → patch bump
git commit -m "fix: fix critical bug"

# 破壊的変更 → major bump
git commit -m "feat!: BREAKING CHANGE: new API"

# main にプッシュ
git push origin main

# → 自動的に:
#   1. pre-commit hook がバージョンを自動更新
#   2. GitHub Actions が起動
#   3. リリースバイナリをビルド
#   4. GitHub Release を作成
#   5. Homebrew formula を更新
```

#### 手動トリガー

GitHub の Actions タブから手動で実行できます：

1. `agiletec-inc/airis-workspace` の Actions タブにアクセス
2. "Release to Homebrew" ワークフローを選択
3. "Run workflow" → "Run workflow" をクリック

### 4. デプロイの流れ

```
コミット & プッシュ
  ↓
GitHub Actions 起動
  ↓
1. バージョン抽出（Cargo.toml から）
  ↓
2. タグ重複チェック（既存の場合はスキップ）
  ↓
3. アーキテクチャ検出（arm64 or x86_64）
  ↓
4. リリースバイナリビルド
  ↓
5. SHA256 計算
  ↓
6. GitHub Release 作成
  ↓
7. homebrew-tap の Formula 更新
  ↓
8. Formula をコミット & プッシュ
  ↓
完了！
```

### 5. ユーザーのインストール方法

リリース後、ユーザーは以下のコマンドでインストールできます：

```bash
# 初回インストール
brew tap agiletec-inc/tap
brew install airis

# 更新
brew upgrade airis

# バージョン確認
airis --version
```

### 6. トラブルシューティング

#### ワークフローが失敗する場合

**エラー: "HOMEBREW_TAP_TOKEN not found"**
- GitHub Secrets に `HOMEBREW_TAP_TOKEN` が設定されているか確認
- トークンの有効期限が切れていないか確認

**エラー: "Push failed"**
- Personal Access Token の `repo` スコープが有効か確認
- homebrew-tap リポジトリへのアクセス権限があるか確認

**エラー: "Tag already exists"**
- 同じバージョンのタグが既に存在する場合は自動的にスキップされます
- バージョンを上げてから再度コミット

#### ローカルでのビルド確認

```bash
# リリースビルド
cargo build --release

# バイナリサイズ確認
ls -lh target/release/airis

# tar.gz 作成テスト
tar -czf airis-test.tar.gz -C target/release airis

# SHA256 確認
shasum -a 256 airis-test.tar.gz
```

### 7. ローカル開発での自動インストール

開発中は以下のコマンドで自動的に `~/.cargo/bin` にインストールできます：

```bash
# ビルド + インストール
cargo build-install

# ファイル監視 + 自動インストール
cargo watch-install
```

### 8. ワークフローのカスタマイズ

`.github/workflows/release.yml` を編集することで、デプロイの挙動を変更できます：

- **トリガー条件の変更**: `on.push.paths` セクション
- **対象ブランチの変更**: `on.push.branches` セクション
- **ビルドオプションの追加**: `cargo build --release` コマンド
- **Formula の内容変更**: `cat > Formula/airis.rb` セクション

---

## 参考リンク

- [GitHub Actions ドキュメント](https://docs.github.com/en/actions)
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Conventional Commits](https://www.conventionalcommits.org/)
# Test


