# Deployment Guide

リリースは **git tag 駆動の [cargo-dist](https://opensource.axo.dev/cargo-dist/)** で行う。
`.github/workflows/release.yml` は cargo-dist が自動生成したものなので手編集しない
(変更は `dist-workspace.toml` を編集して `dist generate` で再生成する)。

## リリースフロー

```
バージョン更新 (Cargo.toml / VERSION)
  ↓
PR → main にマージ
  ↓
git tag vX.Y.Z を push
  ↓
GitHub Actions (release.yml) が起動
  1. dist plan — タグとパッケージバージョンの一致を検証
  2. 各プラットフォームのバイナリ・アーカイブ・インストーラをビルド
  3. SHA256 などのハッシュを生成
  4. GitHub Release を作成しアーティファクトを添付
  5. Homebrew formula を agiletec-inc/homebrew-tap に push
  ↓
完了
```

## 手順

```bash
# 1. バージョンを上げる(PR 経由で main へ)
airis bump-version --minor    # Cargo.toml / Cargo.lock を更新

# 2. main にマージ後、タグを push
git checkout main && git pull --ff-only
git tag v1.57.0
git push origin v1.57.0
```

タグとパッケージバージョンが一致しない場合、`dist plan` の段階でワークフローが fail する。

## Homebrew への公開

cargo-dist の publish ジョブが `agiletec-inc/homebrew-tap` に formula を push する。
認証は GitHub App トークン(bot identity はトークン出力から導出)で行う —
リポジトリの Actions secrets を参照。

## ユーザーのインストール方法

```bash
# Homebrew
brew install agiletec-inc/tap/airis-workspace

# シェルインストーラ
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/agiletec-inc/airis-workspace/releases/latest/download/airis-workspace-installer.sh | sh

# cargo
cargo binstall airis-workspace   # プリビルトバイナリ
cargo install airis-workspace    # ソースからビルド

# バージョン確認
airis --version
```

## crates.io への公開

`publish-crates.yml` が同じ `vX.Y.Z` タグ push を契機に crates.io へ publish する。

## トラブルシューティング

**タグ push 後にワークフローが fail する**
- `Cargo.toml` の `version` とタグ (`vX.Y.Z`) が一致しているか確認
- Actions タブで `plan` ジョブのログを確認

**Homebrew formula の push が fail する**
- GitHub App トークンの secrets 設定と homebrew-tap への権限を確認

## ローカルでのビルド確認

```bash
cargo build --release
ls -lh target/release/airis
```

---

## 参考リンク

- [cargo-dist](https://opensource.axo.dev/cargo-dist/)
- [GitHub Actions ドキュメント](https://docs.github.com/en/actions)
- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
