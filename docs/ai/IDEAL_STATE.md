# Ideal State — airis-workspace「こうなったら最高」

このドキュメントは airis-workspace の **理想状態の宣言** である。実装現状ではなく「制約なくこうあるべき」を全部書き出している。各セクション末の **現状ズレ** は、この理想と現状のコントラストを記述したもので、後続の Gap 分析と移行ロードマップ (別ドキュメント) の起点となる。

## Background

Mac 開発環境では「Bash スクリプトで host の誤操作を block する」弥縫策が増殖していた。具体的には:

- `docker-first-guard.sh` の本体が **どの git 管理 repo にも存在せず** `~/.claude/hooks/` に手書きで置かれている (再現性ゼロ)
- 配置責任を巡って `HOOKS_POLICY.md` / `ARCHITECTURE.md` / `airis-mcp-gateway/skills/.claude-plugin/plugin.json` が **三つ巴で矛盾**
- ARCHITECTURE.md が hook の owner と指す `airis-monorepo` は **実在しない**

「Bash hook をどこに配置するか」という議論は、そもそも前提が崩れている。本来のビジョンは **workspace コンテナ + named volume + host 無汚染** で、それが達成されれば Bash hook で host を後追い block する必要が消える。本ドキュメントはその理想状態を全部言語化することで、弥縫策に頼らずに本筋へ戻る出発点を作る。

---

## 1. Host 無汚染原則

Mac host (開発者の物理マシン) に install するのは:

- **必須 (薄い shell 層のみ)**: Docker Desktop, git, gh CLI, ssh, doppler CLI, Claude Code (host で動く)、IDE/エディタ
- **原則 NG (計測前提)**: 言語ランタイム (node/python/ruby/go/rust/java) を直接、または Homebrew で入れない。pnpm/npm/yarn/uv/pip/cargo を host で直接叩かない。kubectl/helm/kustomize/argocd CLI も host に置かない

ただし以下は **Phase 1 では許容** (Gap 計測 → 移行ロードマップで段階的に削減):

- gh CLI が依存する node ランタイム (Homebrew の依存関係)
- husky / lefthook 等 git pre-commit hook が依存する node
- IDE 拡張 (VSCode ESLint/Prettier/TypeScript LSP、Cursor AI 補完) が要求する host runtime — 詳細 §4
- 上記以外で Gap 計測 (Follow-up plan 1) により host 必須と判明したもの。**許容理由をリストに明記** し、Phase 2 で削減できないか個別検討

**理想 = 「`brew uninstall node pnpm python kubectl ...` を実施しても日常開発が壊れない」状態**。Phase 1 はそこに至る経路、Phase 2 で例外リストを縮小する。「原則 NG」を骨抜きにしないため、例外は **必ず許容理由付きで明記** することを規約として固定。

**現状ズレ:**
- Homebrew で `node` `pnpm` `python` `kubectl` `helm` 等が入っている可能性高 (未確認、要計測)
- `~/.zshrc` に `nvm` `pyenv` `rbenv` 系の PATH 操作がある可能性
- `~/.kube/config` `~/.aws/credentials` 等の認証情報が host に置かれている
- 上記の例外候補 (gh CLI 依存 node 等) が実際に host で必要かは Phase 1 計測で確定

---

## 2. Workspace コンテナの責務

`manifest.toml` を SSOT に、`airis up` で立ち上がる長寿命コンテナ。役割:

- 言語ランタイム (Node 22, Python 3.13, Rust toolchain, Go 等、project が要るもの) を内蔵
- 開発 CLI (pnpm, uv, cargo, kubectl, helm, kustomize, doppler, sqlc, migrator, etc) を内蔵
- 開発 server (Vite, Next.js, Storybook, FastAPI dev server, etc) を内側で起動、Traefik 経由で host のブラウザに expose
- ホットリロードのため source code は bind mount (read-write)。それ以外は全部 named volume

`airis exec <cmd>` は実体としては `docker compose exec workspace <cmd>`。tab 補完も含めて host shell で違和感なく使える。

**現状ズレ:**
- `airis-workspace/compose.yaml` は service 定義済みだが、実際に「全開発コマンドを exec で叩く」運用が成立してるか未確認
- `airis exec` 相当のラッパが存在するかも未確認 (manifest.toml の commands で `up/down/test/lint` のみ列挙、汎用 exec が未整備な可能性)

---

## 3. Named Volume の境界

| 領域 | 種類 | 命名 |
|------|------|------|
| Source code | bind mount (host RW) | `./:/workspace` |
| Node modules (root + 各 app) | named volume | `airis_node_modules`, `airis_node_modules_<app>` |
| pnpm store | named volume | `airis_pnpm_store` (`/pnpm/store`) |
| Cargo registry / target | named volume | `airis_cargo_registry`, `airis_cargo_target` |
| Python uv / pip cache | named volume | `airis_uv_cache`, `airis_pip_cache` |
| HuggingFace / model cache | named volume | `airis_hf_cache` |
| Build artifacts (Next.js .next, Vite dist 等) | named volume | `airis_build_<app>` |

**bind mount は source code のみ**。`./node_modules` `./.pnpm-store` のようにホストへ漏れる構成は禁止。Mac の Spotlight インデックス / Time Machine バックアップが node_modules を舐める惨事を構造的に防ぐ。

**現状ズレ:**
- 個々の repo (agiletec, airis-studio 等) が上記の named volume 規約に従っているか未確認
- 過去の手作業で `./node_modules` が bind mount として残ってる可能性

---

## 4. 開発者の操作モデル

Claude Code が host で動く。Bash tool で叩くコマンドは原則 `airis exec ...` 経由 (= workspace コンテナ内で実行)。例外:

- `git`, `gh`, `ssh`, `doppler` (host で動かす方が自然なもの)
- `docker compose` (workspace コンテナ自体を制御するので host で必要)
- `airis up/down/exec/...` 自体

`pnpm install` も `cargo build` も `kubectl get` も host に install されていないので、誤って host で打つと "command not found"。**これが「Bash hook で block する」必要が消える理由**。

### IDE / git hook が host runtime を要求する問題

IDE 拡張 (VSCode ESLint/Prettier/TypeScript LSP、Cursor AI 補完) と git hook (husky/lefthook) は、現状 **host の node を呼ぶ前提で動く**。これが §1 の「host 無汚染」と直接衝突する:

- VSCode ESLint 拡張は workspace の `node_modules` から `eslint` binary を **host node で実行** する。host node が無いと拡張が起動できない
- husky の pre-commit hook は `node .husky/_/husky.sh` 経由で呼ばれる。host node が無いと commit が物理的に失敗する
- Cursor の AI 補完が裏で呼ぶプロセスが host runtime に依存している場合がある

**Phase 1 (現実解)**: §1 の例外リストとして「IDE / git hook 依存の node」を許容。ただし「IDE が要求するから何でも入れる」ではなく「**最小限の node ランタイムだけ、明示的に**」を目指す (例: 1 バージョンに pin、グローバル npm install を禁止 等は別途検討)

**Phase 2 (理想解)**: **devcontainer.json (or Cursor の Remote Container 機能) で IDE 自体をコンテナ内で動かす**。これにより IDE 拡張・git hook が workspace コンテナ内で完結し、host node が完全に不要になる。当初 §12 Out of scope としていたが、§1 の理想形を完全達成するには **Phase 2 で取り組む必須項目** に格上げする

**現状ズレ:**
- Claude Code の global rules (`~/.claude/CLAUDE.md`, `~/.claude/rules/docker-first.md`) は「host で実行するな」と文章で書いているが、host に実行可能な binary が残っている限り Claude が叩いてしまう余地が残る
- `airis exec <cmd>` でなく素の `pnpm <cmd>` を提案してくる癖が Claude / 人間双方にある
- VSCode/Cursor 設定が host node 依存の状態 (Phase 2 で devcontainer.json 化が必要)

---

## 5. AI ツール横断の設定一元管理

`manifest.toml` の `[ai]` セクションを単一 source として、`airis gen` が以下を派生展開する:

```toml
[ai]
shared_rules = ["docs/ai/PROJECT_RULES.md", "docs/ai/WORKFLOW.md", "docs/ai/REVIEW.md"]

[ai.claude]
target = "~/.claude/CLAUDE.md"
rules_dir = "~/.claude/rules/generated/"
hooks = ["server-mutation-guard", "docker-first-guard"]   # 廃止予定 (本文 §7 参照)

[ai.codex]
target = "~/.codex/AGENTS.md"

[ai.gemini]
target = "~/.gemini/GEMINI.md"

[ai.cursor]
rules_dir = ".cursor/rules/"
```

### 二層構造 — GENERATED ブロック + 手動領域

`airis gen` が触れていい範囲を明示するため、各 target ファイルは **冒頭に固定の GENERATED ブロック** を持つ:

```markdown
<!-- BEGIN GENERATED airis gen -->
(共通 shared_rules がここに展開される)
<!-- END GENERATED -->

(以下は手動領域 — airis gen は read も write もしない)

## My personal notes
...
```

ルール:

- **GENERATED ブロックの位置は target ファイルの冒頭に固定**。末尾や中間には置かない (手動領域に挟まると編集事故を招く)
- **`airis gen` は GENERATED ブロックの外を一切 read しない**。例外は「競合検出のため diff を取る」目的のみで、書き換えは絶対しない
- **memory ファイル (`~/.claude/projects/.../memory/*.md`) は対象外**。Claude の auto-memory は per-session の独自管理で、tool 固有の領域 (airis gen が触ると会話履歴が消える)
- **rules ディレクトリは `generated/` と `manual/` に分離** (例: `~/.claude/rules/generated/*.md` だけが airis gen の対象、`~/.claude/rules/manual/*.md` は手動領域)。混在を避ける
- **競合検出**: 手動領域に GENERATED ブロックと同じ規約を書いてしまった場合、`airis gen` が warning で「重複」を報告 (block はしない、ユーザーが整理する)

新ツール追加 = manifest.toml に 1 セクション足し、target/rules_dir を指定するだけ。

**現状ズレ:**
- `[ai]` セクションは未定義 (manifest.toml は `[docs] vendors = ["claude"]` 程度)
- Codex/Gemini/Cursor 用の派生は手動配置に近い状態
- `~/.claude/rules/*.md` と `airis-workspace/templates/claude/rules/` で内容が二重管理になっている可能性
- GENERATED/手動の二層構造はまだ存在しない (現状の `~/.claude/CLAUDE.md` は全部手動領域として扱われている)
- `~/.claude/rules/` 配下が `generated/` `manual/` に分離されていない

---

## 6. Server (agile-server 等) との通信モデル

GitOps cluster である以上、cluster 状態の変更は **PR → main → ArgoCD pull** の経路だけが正規ルート。host から SSH で kubectl mutation を打つことが「**そもそも環境的に不可能**」になる構造:

- Mac host に kubectl/helm/kustomize/argocd CLI が install されていない (§1)
- `~/.kube/config` も host に存在しない (§1)
- workspace コンテナ内に kubectl を入れても、kubeconfig は **read-only な GitOps debug 用** (get/describe/logs だけ。apply/annotate/rollout は権限を持たない)
- agile-server 上の k3s API は Tailscale/Cloudflare 経由でも公開しない。SSH で server に入った先での kubectl mutation も、ArgoCD の self-heal が直後に巻き戻す = 永続変更にならない

「mutation を CLI で発行する」が技術的に可能でも、ArgoCD の sync が **直後に上書きする** ことで「やる意味がない」状態。これが GitOps の本来の防御 — Bash hook で禁止しなくても結果が同じ。

緊急時の手動 refresh は ArgoCD UI から。CLI で `kubectl annotate refresh=hard` を打つ習慣を作らない。

### この防御が成立する前提条件

- **全 Application で `syncPolicy.automated.selfHeal: true` が有効化されていること**。selfHeal が無効な app では手動 mutation が永続化してしまい、§6 の防御から外れる
- **`syncPolicy.automated.prune: true` も併用** (Git から消した resource が cluster に残らない)
- Phase 1 で全 Application の `syncPolicy` を計測し、selfHeal/prune が未有効の app を有効化する PR を出す。これが完了するまで §6 は **部分的にしか機能しない**

この前提は §7 の対応表 (環境設計が防御を担う) の信頼性を直接支える。selfHeal が無いと「ArgoCD self-heal で巻き戻る」という防御の根拠そのものが崩れる。

**現状ズレ:**
- Mac host に kubectl が入っている可能性 (要計測)
- `~/.kube/config` が生きている可能性
- ArgoCD の selfHeal/prune が全 Application で有効か未確認 (有効でない app は §6 の防御外、Phase 1 で網羅的に確認)
- agile-server CLAUDE.md `Common Commands` に書いた SSH+kubectl mutation コマンド 3 件は read-only に書き換え済み

---

## 7. 「ガード弥縫策が要らない世界」の対応表

| 旧弥縫策 (Bash hook) | 環境設計の代替 (こちらが本来) |
|----------------------|------------------------------|
| `docker-first-guard.sh`: host で `pnpm install` 等を block | host に node/pnpm/python/uv/cargo が入っていない (§1) → `command not found` で物理的に不可能 |
| `docker-first-edit-guard.sh`: bind mount 形式や host path を Edit/Write で書こうとした時 block | manifest.toml SSOT で compose.yaml/Dockerfile が生成され、手書きしないので破綻パターンが入り込む経路がない (§11) |
| `server-mutation-guard.sh`: ssh+kubectl mutation を block | host に kubectl/kubeconfig 無し (§1) + ArgoCD self-heal で mutation が即巻き戻る (§6 の前提条件下) |
| `~/.claude/settings.json` PreToolUse 配線で正規表現マッチ | matcher が必要な時点で「host にやれてしまう余地」が残ってる証左。環境を直す方が筋 |

ガードが完全に消える世界では、`~/.claude/hooks/` 自体が空になる。`~/.claude/CLAUDE.md` と `~/.claude/rules/*.md` (= 振る舞いの規約) は残るが、それは「Claude にこう動いてほしい」を伝える人間語のドキュメント。**強制力は環境が持つ、文書は意図を伝える**、の二層に分離する。

**現状ズレ:**
- `~/.claude/hooks/docker-first-guard.sh` `docker-first-edit-guard.sh` `server-mutation-guard.sh` が生きている
- `~/.claude/settings.json` PreToolUse に hook 配線が残っている
- これらは §1 の host 無汚染 + §6 の selfHeal 前提が達成された段階で、初めて「消しても困らない」になる。順序が大事

---

## 8. 再現性 (新マシン / 新メンバー setup)

新規 Mac で `git clone airis-workspace && cd airis-workspace && airis up` の **3 行だけ** で開発可能になる。所要時間目標 = 初回 image pull 5〜10 分、2 回目以降 30 秒。

成立条件:

- Docker Desktop だけは事前 install (これは Mac のシステム整備としてやる)
- doppler login も事前 (secret access)
- それ以外の言語ランタイム/CLI は全部 image に焼く or named volume で初回起動時に prepare

新メンバーが「わたしの環境では動かない」と言う余地を構造的に消す。

**現状ズレ:**
- 現状 README に書かれた setup 手順が `airis up` 3 行で済むか未確認
- Doppler の secret 設定が完了していない状態で `airis up` した時のフォールバックが妥当か未確認

---

## 9. 本番デプロイの一貫性

ローカル workspace コンテナ・CI ビルド image・k3s 上の本番 Pod が、**同じ multi-stage Dockerfile から `--target` で stage を切り替えて build される**。

```dockerfile
# Dockerfile (例)
FROM node:22-alpine AS base
# 共通の system deps、user setup、共通 env
RUN apk add --no-cache tini && \
    addgroup -g 1000 app && adduser -D -u 1000 -G app app

FROM base AS dev
# bind mount 前提、ホットリロード用 dev server
WORKDIR /workspace
CMD ["pnpm", "run", "dev"]

FROM base AS build
WORKDIR /app
COPY pnpm-lock.yaml package.json ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm run build

FROM base AS prod
WORKDIR /app
COPY --from=build /app/dist /app/dist
COPY --from=build /app/node_modules /app/node_modules
USER app
CMD ["node", "/app/dist/server.js"]
```

- ローカル: `docker compose up` (compose.yaml で `build.target: dev` を指定、bind mount で source 投入)
- CI: `docker build --target prod -t ghcr.io/.../<app>:<sha> .`
- k3s: 上記 image を ArgoCD が pull

**`base` stage は完全に同一** — system deps、user setup、共通 env は全 stage で共有される。**違うのは「source の入れ方」(dev は bind mount / build/prod は COPY)** と「entry command」だけ。これが現実的に達成可能な「一貫性」の正確な定義。

「ローカルでは動くが本番で動かない」が発生したら、**stage 差分 (dev vs build vs prod の Dockerfile レイヤ)** を疑う。base stage 共通部分を疑わない。

**現状ズレ:**
- 各 app の Dockerfile が `manifest.toml` ベースで自動生成される設計か、手書きか未確認
- multi-stage で dev/build/prod が分離された Dockerfile が存在するか未確認 (現状は dev のみ・prod のみ separate Dockerfile の可能性)
- `airis gen` が Dockerfile を上書きするときに 3 stage 全部を吐くか未確認

---

## 10. Repository 責務の境界

| Repo | 責務 | 持たないもの |
|------|------|------------|
| **airis-workspace** | manifest.toml SSOT、`airis gen/up/exec/...` CLI、Claude/Codex/Gemini/Cursor 用ルール source、(将来) workspace コンテナ image。**hook/rule/guard の owner はここ**。 | MCP proxy、個別 app の business logic |
| **airis-mcp-gateway** | MCP server 多重化 + proxy + dynamic routing。Docker compose で立ち上がる単機能サービス。 | hook/rule/guard、host 開発者の workflow |
| **agiletec** | corporate / dashboard / media の web app monorepo。manifest.toml で airis-workspace に従う。 | infra、cluster、shared rules |
| **agile-server** | k3s + ArgoCD GitOps cluster の宣言的定義のみ。bootstrap/* + apps/* + manifests/* + ApplicationSet。 | workload (各 app の repo が自分で Application を登録)、host setup の bash 手順 (将来 Ansible 化) |
| **airis-studio** | 動画 / 画像処理の app。GPU を使う Pod を持つ。 | infra、cluster、shared rules |

→ **HOOKS_POLICY.md / ARCHITECTURE.md / plugin.json の三つ巴矛盾は、「hook/rule/guard owner = airis-workspace」に統一して解消する**。`airis-mcp-gateway` は MCP proxy 単機能、自分の plugin description から "Docker-first guards" を抜く。`airis-monorepo` という存在しない repo への参照を ARCHITECTURE.md から消す。

**現状ズレ:**
- HOOKS_POLICY.md / ARCHITECTURE.md / plugin.json が三つ巴で違うことを言っている
- 統一作業はこの IDEAL_STATE 確定後の別 PR

---

## 11. manifest.toml の SSOT 範囲

`manifest.toml` が扱う:

- workspace のランタイム / package manager / app 定義
- Docker compose 生成 (workspace + Traefik + 必要に応じて DB/Redis 等)
- Dockerfile 生成 (multi-stage、named volume mount points)
- AI ツール rule / hook 派生 (§5)
- CI workflow 雛形 (`airis gen` で `.github/workflows/` を吐く)
- 言語別 verify コマンド (`airis verify` のチェーン)

扱わない:

- Application runtime config (DB-backed settings, tenant boundaries, feature flags) — `docs/ai/architecture-invariants.md` 系の別 SSOT
- Secret values — Doppler が SSOT
- Cluster manifest (Application/AppProject/Helm values) — `agile-server/apps/*.yaml` が SSOT

**現状ズレ:**
- 上記範囲が `docs/ai/PROJECT_RULES.md` に明記されているが、実際の `manifest.toml` 機能で全部カバー出来ているかは未計測

---

## 12. Out of scope (このビジョンが扱わないこと)

- IDE 拡張 (VSCode / Cursor) のチーム共通設定 (※ チーム単位で個別管理、個人 dotfiles の領域)
- モバイルネイティブ開発 (iOS / Android 開発は別 stack)
- Windows / Linux ネイティブ host (Mac 開発前提)

(※ devcontainer.json / Remote Container は §4 Phase 2 で取り組む対象に格上げしたため Out of scope ではなくなった)

---

## Next

このドキュメントが確定したら、次の 3 つを別ドキュメント / 別 PR で進める:

1. **Gap 計測** (`docs/ai/GAP.md`): §1 〜§11 の「現状ズレ」を実測 (`brew list`、`which kubectl`、`~/.kube/config` 存在、各 repo の compose.yaml と named volume 実態、ArgoCD selfHeal/prune 状態、manifest.toml カバレッジ)。表にまとめる
2. **移行ロードマップ** (`docs/ai/MIGRATION.md`): Gap を埋める順序。Phase 1 (selfHeal 有効化、§1 例外リスト確定、`airis exec` 拡張、§5 二層構造実装) → Phase 2 (devcontainer.json、§7 hook 撤去) の段階分け
3. **責務文書の整合 PR**: §10 の責務境界に揃えて HOOKS_POLICY.md / ARCHITECTURE.md / plugin.json description を改訂
