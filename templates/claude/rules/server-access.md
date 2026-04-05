# Server Access (kazuki@100.82.212.64)

Tailscale host, Ubuntu, RTX 5070 Ti (16GB VRAM), Cloudflare Tunnel → Traefik.

## SSH rules
- **Read-only**: freely run `cat`, `ls`, `grep`, `docker ps`, `docker compose logs`, `nvidia-smi`, `git log/status/diff`
- **Changes**: ask user before `rm`, `mv`, `docker compose up/down/build`, file edits
- **Never**: `sudo`, file transfers (`scp`/`rsync`/`>`), `git pull/checkout/reset`, `pip/npm/apt install`

## GitHub Actions Self-Hosted Runners

ランナー自体が Docker コンテナ (myoung34/github-runner)。`agiletec-inc/agile-server` リポジトリの `runners/` 配下の compose ファイルで管理。

| Runner | Scope | Label | 用途 |
|--------|-------|-------|------|
| runner-agiletec-01~08 | org (agiletec-inc) | `repo-agiletec` | agiletec monorepo |
| runner-video-restore-01 | repo (kazukinakai/video-restore) | `repo-video-restore,gpu` | video-restore (GPU) |
| runner-shared-01 | org (agiletec-inc) | `pool-shared,light` | デモ・軽量リポジトリ |

ランナーの起動・更新: `agile-server` の GA workflow `Manage Runners` (workflow_dispatch) 経由。

## Deploy Flow (GitOps)

```
push to main
  → GA self-hosted runner がジョブを受け取る
  → actions/checkout@v4 で /tmp/runner/work/{repo}/{repo}/ に checkout
  → docker compose build + up -d（checkout dir 内で実行）
  → コンテナが起動し、永続データは SSD の volume をマウント
```

- サーバー上にリポジトリの手動 clone は存在しない。全て GA 経由
- `git pull` はサーバーで実行しない
- `profiles` 付きサービス (bench, tools) は GitOps 対象外 — 手動 `docker compose build` が必要
- Always confirm with user before deploying

## Directory Layout

```
/tmp/runner/work/                    # GA runner checkout (named volume)
├── video-restore/video-restore/     # video-restore リポジトリ
├── agiletec/agiletec/               # agiletec monorepo
└── agile-server/agile-server/       # サーバーインフラ管理

/home/kazuki/ssd-2tb/                # Samba 共有 SSD — 永続データ
├── video-restore/
│   ├── data/                        # 動画データ (コンテナに /data としてマウント)
│   └── models/                      # AI モデル
└── ...

/home/kazuki/                        # ホームディレクトリ
└── (リポジトリ clone は置かない)
```
