# Global Rules

日本語で回答する。コードコメントは英語。

## Docker-First

全てDockerコンテナ内で実行する。ホストでパッケージマネージャやランタイムを直接実行しない。設計・構成・コマンドの詳細は `~/.claude/rules/docker-first.md` を参照。

OrbStack 固有の挙動(自動 `*.orb.local` DNS、ローカル image を k8s から registry なしで参照、`/mnt/mac` 共有など)は `~/.claude/rules/orbstack.md` を参照。

## Safety

- hookの無効化・削除・迂回をしない（`--no-verify`, `git config core.hooksPath` 変更を含む）
- テスト/lintの失敗はチェックの無効化ではなく原因を修正する
- push/deploy前にユーザーの確認を取る
- 設定ファイル（CLAUDE.md, .claude/rules/, settings*.json）はユーザーの明示的な指示なしに変更しない

## Docker Container Safety

- **自分の compose project 以外のコンテナを絶対に stop/rm/kill/down しない**
- **自分の compose project 内のコンテナは `docker stop`/`start`/`restart` OK**。GPU 競合回避や一時的な pause など、運用上必要なら止めていい。ただし `down`/`rm` はデータ損失リスクがあるのでユーザー確認
- ポート競合が発生した場合、既存コンテナを止めるのではなくユーザーに報告して判断を仰ぐ
- `docker compose down` は実行前に必ずユーザーの確認を取る（自分の project であっても）
- Traefik 経由のサービスにホストポートバインド（`ports:`）は不要。`expose:` を使う
- ポート番号をハードコードしない。環境変数（`${PORT:-3000}`）を使う
- `docker system prune`, `docker volume prune` 等の破壊的クリーンアップはユーザーの明示的な指示がある場合のみ
- 他プロジェクトのコンテナ・ボリューム・ネットワーク・イメージを削除しない

## Server-Side Mutation

GitOps 配下のサーバー(env で指定された host、詳細は `~/.claude/rules/server-access.md`)では:

- **SSH は read-only 観測のみ**: `kubectl get/describe/logs`, `cat`, `ls`, `git log`, `docker ps/logs`, `nvidia-smi` は OK
- **Cluster mutation は GitOps 経由のみ**: `kubectl (apply|annotate|rollout|patch|delete|create|scale|edit|replace)` を SSH で実行しない。Application sync は PR → main → ArgoCD pull、緊急時は ArgoCD UI から
- **server-side ファイル作成・編集・バックアップ禁止**: `scp` / `rsync` / heredoc / `>` / `tee` / `.bak` / `.old` / `_backup` 全面禁止
- **例外は bootstrap 手順のみ**: 該当リポジトリの `bootstrap/` で明示された Secret plant など、手順書記載のものだけ
- 上記は PreToolUse hook で機械的にブロックされる(`airis guards install --global` で配布)
- 詳細は `~/.claude/rules/server-access.md`

## Secrets

機密度の階層分けと置き場所(commit OK / .env / Doppler / 1Password)は `~/.claude/rules/secrets-tier.md` を参照。

infra 固有値(サーバー名、IP、Tailscale、ホストパスなど)は **rule にハードコードせず env / Doppler から取得する**。詳細は同 rule 内の Tier 1 項。

## Verify — 完了報告の前に自分で確認する

- 「実装しました」で終わりにせず、Playwrightまたはブラウザで自分の目で動作確認する
- ユーザーの元の問題が実際に解決していることを確認してから報告する
- push前に `airis test` を実行してエラー0を確認する（.husky/pre-push hook でも lint + typecheck は強制される）
- push後は `gh run watch` でCI完了を待ち、失敗したら `gh run view --log-failed` で原因を確認して自分で修正・re-pushする。CIが通るまでタスク完了を報告しない
- deployワークフローがトリガーされたら `gh run watch` でデプロイ完了を待ち、失敗したら原因を確認して修正する。特にDockerfileの依存不足に注意
- デプロイ後のヘルスチェックで問題を発見した場合、原因が今回の変更かどうかに関わらず修復すること。「今回の変更とは無関係」を理由にスキップしてはならない。ユーザーにとってはサービスが動いてるか動いてないかだけが重要
- ユーザーをデバッガー代わりにしない

## Error Handling

- エラーに遭遇したらまず公式ドキュメントとリファレンスを読む
- 公式のベストプラクティスやサンプルコードがあればそのまま使ってよい
- 自分の前提知識で推測するより公式を参照した方が速く正確に解決できる
- サイレントフォールバックや握りつぶしではなく、適切なエラーを明示的に返す
- `A || B` 式のフォールバックでエラーを握りつぶすな。失敗は失敗として throw/log する
- 環境変数の旧名→新名フォールバック(`NEW_KEY || OLD_KEY`)禁止。リネームしたら一括置換しろ

## Naming & Structure

- 名前は責務や状態を表す具体的なものにする（ディレクトリ、パッケージ、セクション、ラベル全て）
- `core`, `utils`, `common`, `shared`, `helpers` は使わない — 何をしているかわからない
- フラットに始めてファイルが増えたときだけネストする

## Planning / Bug Fix

`~/.claude/rules/planning.md` と `~/.claude/rules/bug-fix.md` を参照。

## MCP

airis-mcp-gateway経由で接続する。サーバー追加は `airis-mcp-gateway/mcp-config.json` で行う。

## Tool Routing

共通タスクには `airis-route` を使う。探索的な `airis-find` をスキップして最適なツールチェーンを直接取得できる。

## Compaction

会話圧縮時に保持する:

- 現在のタスク目標
- 変更済みファイル一覧
- 未完了ステップ
- ユーザーからのフィードバック
