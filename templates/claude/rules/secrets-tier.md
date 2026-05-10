# Secrets — Tier 階層と置き場所

機密情報の機密度を 4 段階に分類し、各 Tier ごとに置き場所と取り扱いを固定する。
Tier を間違えると「漏れたら本番落ちる値が git に commit される」「逆に共有して構わない値が 1Password にロックされて誰も使えない」という両極端の事故が起きる。

迷ったら **Tier を 1 段上げる**(より厳しく扱う)方向に倒す。

## Tier 0 — Public

**漏れても困らない**。git にそのまま commit してよい。コードや rule に直接書いてよい。

例:
- 公開 API endpoint URL(documented public service)
- Anon / publishable key(Supabase `NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY`、Stripe publishable key、Cloudflare Turnstile site key)
- Project ID / Organization ID(Supabase project ref、GCP project ID)
- npm package 名、Docker image tag(public registry)

判定基準: **その値が公開ドキュメントや公開リポジトリに既に出ているか**。出ていれば Tier 0。

## Tier 1 — Sensitive

**漏れたら不便だが致命的ではない**。git には commit しない、`.env.local` または env 経由で扱う。チーム内では共有可。

例:
- infra 固有値(server hostname、Tailscale IP、内部 service URL)
- 個人の short-lived OAuth token / GitHub PAT(scope 限定、期限付き)
- 開発用 OAuth client secret(本番ではない)
- 内部 Slack channel ID、社内 LINE 公式アカウントの user ID

判定基準: **漏れたら attack surface が広がるが、即座に金銭的・データ損失被害は出ない**。

置き場所:
- 個人 PC: `~/.env.local` または Doppler `WORKSPACE/dev` config
- CI: GitHub Secrets(repository scope)
- リポジトリ内: `.env.example` に key 名だけ列挙、値は空欄

**rule に直接書かない** — 例えば `~/.claude/rules/server-access.md` に `192.0.2.42` のような実 IP を書いたら rule 自体が Tier 1 化して private repo にしか置けなくなる(`192.0.2.0/24` は RFC 5737 の documentation 用 reserved range なので例として安全)。代わりに `$AGILE_SERVER_HOST` のように env var 名だけ書き、値は Doppler から取る。

## Tier 2 — Critical

**漏れたら本番が落ちる、課金される、データが破壊される**。git に絶対 commit しない、ローカルファイル NG、Doppler / GitHub Secrets / Vault 必須。

例:
- Service role key(Supabase service role、Stripe secret key、Anthropic API key)
- 本番 DB credentials(production Postgres password、Supabase service-role JWT secret)
- Cloudflare API token(account-wide)
- 本番 deploy 用 SSH 鍵 / GitHub App private key
- Twilio Auth Token(本番)、Retell API key

置き場所:
- Doppler の `prd` config(Doppler が SSoT、ローカルファイル禁止)
- 値の取得は実行時のみ: `doppler run -p <project> -c prd -- <command>`
- CI/CD: GitHub Secrets(organization scope、必要 repo に限定アクセス)

**コード内では絶対に echo / log しない**。`doppler secrets get ... --plain` の出力を `>` でファイルに落とさない。

## Tier 3 — Never written down

**漏れたら法的・経営的損害**。デジタルでは保存しない、人間の頭か hardware key のみ。

例:
- Root CA private key
- 本番 master credentials(Supabase project owner password、AWS root account)
- Infrastructure recovery seed phrase
- Hardware key(YubiKey 等)の PIN

置き場所:
- 1Password(個人 vault、share NG)
- Hardware key(YubiKey、TPM)
- 人間の記憶(passphrase 系)

**コード / rule / runbook に書かない**。手順書には「1Password vault `infrastructure` から取得」とだけ書く。

## Tier 判定の早見表

| 値 | Tier | 例 |
|---|---|---|
| Anon key, publishable key, public URL | 0 | git に書いて OK |
| Server hostname, IP, internal endpoint | 1 | Doppler dev config |
| Service role key, prod DB password, prod API key | 2 | Doppler prd config |
| Root credentials, master keys, recovery seeds | 3 | 1Password / hardware |

## 既存 secret を削除する手順

うっかり Tier 1 以上を git にコミットした場合:

1. **値をローテーション**(コミット履歴に残るので、値そのものを失効させる)
2. `git filter-repo` または BFG で履歴から消す(force-push 必要、影響範囲確認)
3. ローテート済み新値を Doppler に登録
4. 関係者に通知

ローテーションを先にする。順序を逆にすると、攻撃者がスクレイピングしてローテート前の値で侵入する窓ができる。

## 関連

- `~/.claude/rules/server-access.md` — server hostname 等(Tier 1)を env 経由で扱う具体例
- `~/.claude/rules/docker-first.md` — `${PORT:-3000}` のように env で値を持つ実装パターン
