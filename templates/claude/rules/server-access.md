# Server Access (`$AGILE_SERVER_HOST`)

リモート開発サーバーへの SSH 接続規約。
**サーバー固有値(host、IP、hardware spec、ストレージパス)は本ファイルにハードコードしない**。
全て env / Doppler から取得する設計(根拠は `~/.claude/rules/secrets-tier.md` の Tier 1)。

## 値の解決

実行時に必要な env var:

| Env var | 役割 | 取得元 |
|---|---|---|
| `$AGILE_SERVER_HOST` | SSH 接続先 host(Tailscale 経由想定) | Doppler `WORKSPACE/dev` |
| `$AGILE_SERVER_USER` | SSH ユーザー | Doppler `WORKSPACE/dev` |
| `$WORKSPACE_PERSISTENT_BASE` | 永続データ用ディレクトリ(Samba 共有等) | Doppler `WORKSPACE/dev` |
| `$RUNNER_WORK_DIR` | GitHub Actions runner の checkout dir(typically `/tmp/runner/work`) | Doppler `WORKSPACE/dev` |

agent 起動時に Doppler 経由で env を inject:

```bash
doppler run -p workspace -c dev -- claude
# または zsh/bash の rc に:
eval "$(doppler secrets download --project workspace --config dev --no-file --format env)"
```

env が解決できない場合は **rule に従わずユーザーに確認**(値を推測しない)。

## SSH rules

- **Read-only 観測のみ自動可**: `cat`, `ls`, `grep`, `tail`, `docker ps`, `docker compose logs`, `nvidia-smi`, `git log/status/diff`, `kubectl get/describe/logs/top`
- **Cluster mutation 全面禁止**: `kubectl (apply|annotate|rollout|patch|delete|create|scale|edit|replace)` を SSH 経由で実行しない。Application sync は **GitOps (PR → main → ArgoCD pull) のみ**。緊急時の手動 refresh は ArgoCD UI から
- **File transfer / server-side write 全面禁止**: `scp`, `rsync`, ssh + heredoc でのサーバー側ファイル作成、`>` / `tee` / `cat > /path` でのサーバー側書き込み、`.bak` / `.old` / `_backup` のバックアップファイル作成、すべて禁止。etckeeper 等の git 安全網を前提にしない
- **bootstrap 例外**: 該当リポジトリの `bootstrap/README.md` で明示された Secret plant 手順(`kubectl ... --from-file=KEY=/dev/stdin` で stdin 経由、ディスク不経由)のみ。手順書記載外の SSH mutation は禁止
- **Never**: `sudo`, `git pull/checkout/reset`, `pip/npm/apt install`
- 上記 mutation/転送パターンは PreToolUse hook(`airis guards install --global` で配布)で機械的にブロックされる

## サーバー上での手動操作 — 絶対禁止

- **手動 `git clone` 禁止** — サーバー上にリポジトリの手動 clone を作らない。全て GitHub Actions 経由
- **`compose.override.yml` 作成禁止** — `compose.yml` は 1 つだけ。全て profile で管理
- **`docker compose up/build` の手動実行禁止** — GA workflow 経由のみ。profile 付きサービスも含む
- **`git pull` 禁止** — サーバー上で直接 pull しない
- **ファイル作成・編集禁止** — サーバー上のソースコードや config を手で触らない
- 違反を発見したら: ユーザーに報告し、GA workflow 経由で正しく管理する方法を提案する

## GitHub Actions Self-Hosted Runners

ランナーは ephemeral コンテナで、ジョブ実行時のみ起動し完了後に自動削除される。
具体的な runner 名 / scope / label / 用途の一覧は **本 rule ではなく**、ランナーを管理する別 repo の runbook(例: `<org>/<runner-mgmt-repo>/README.md`)を参照する。

理由: runner の構成は環境ごとに変わる(数、命名、scope の組み方)ので rule に持たせると陳腐化する。

## Deploy Flow (GitOps 配下のサーバー)

```
push to main
  → GA self-hosted runner がジョブを受け取る
  → actions/checkout で $RUNNER_WORK_DIR/{org}/{repo}/{repo}/ に ephemeral checkout
  → docker compose build + up -d (checkout dir 内で実行)
  → ジョブ完了後 checkout は消える(ephemeral)
  → コンテナはイメージから動作、永続データは $WORKSPACE_PERSISTENT_BASE 配下の volume にマウント
```

- サーバー上にリポジトリの手動 clone は存在しない。全て GA 経由
- Always confirm with user before deploying

## Directory Layout

Docker は root 権限、データはユーザー権限。Samba 共有でホスト Mac から直接アクセス可能(構成済みの場合)。

```
$RUNNER_WORK_DIR/                    # GA runner checkout (ephemeral — ジョブ完了で消える)
├── <repo-A>/<repo-A>/
└── <repo-B>/<repo-B>/

$WORKSPACE_PERSISTENT_BASE/          # 永続ストレージ(Samba 共有等、ユーザー権限)
├── <project-A>/
│   ├── data/                        # アプリデータ → コンテナ内 /data/
│   └── models/                      # AI モデル等の大容量ファイル
└── ...

$HOME/                               # ホームディレクトリ
└── (リポジトリ clone は絶対に置かない)
```

具体的な値は env から解決すること。本 rule に絶対パスを書かない。
