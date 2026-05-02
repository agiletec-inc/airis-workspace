# OrbStack — 開発環境の前提

Mac での Docker / Kubernetes / Linux VM の動作基盤は **OrbStack**(Docker Desktop の代替)。
公式: <https://docs.orbstack.dev/>

軽量 Linux VM + 共有カーネル(WSL2 と同じ思想)。Apple Silicon 向けに専用最適化。
"Starts in 2 seconds"、メニューバー常駐。

## なぜ知っておく必要があるか

- **`docker` / `kubectl` コマンドはそのまま使える**(API は Docker / kind 互換)
- **でも OrbStack 固有の便利機能を知らないと損をする**(自動 DNS、ローカル image を直接使える、`/mnt/mac` 経由のホスト共有など)
- **Docker Desktop と挙動が違う部分がある**(network stack が独自、`*.orb.local` ドメインが自動)

## Docker

```bash
orb config docker            # daemon.json を編集
orb restart docker
orb logs docker
```

- ホスト ↔ コンテナの bind mount は VirtioFS で高速。`./node_modules` の bind mount を避けて named volume を使うルール(`docker-first.md`)はそのまま
- container は `<container>.<compose-project>.orb.local` で自動 DNS 解決
- volume の中身は Mac から直接 `~/OrbStack/...` で参照できる

## Kubernetes

OrbStack 内蔵 k8s を使う場合(リモート本番 k3s/EKS とは別、ローカル開発用):

```bash
orb start k8s                # クラスタ起動
orb stop k8s
orb restart k8s
orb delete k8s               # クラスタ破棄(ユーザー確認必須)
```

特徴:
- **同じ container engine を共有**: ローカルで `docker build` した image は registry push なしで k8s pod から参照できる
- ただし `:latest` タグは挙動が紛らわしいので、ローカル開発では `:dev` 等の別タグ + `imagePullPolicy: IfNotPresent`
- LoadBalancer Service は `*.k8s.orb.local` で自動 DNS 解決(host 側のブラウザから直接アクセスできる)
- 標準で Ingress controller は入っていない。必要なら Helm で `ingress-nginx` または `traefik` を入れる
- 軽量だが kind / k3d / minikube より機能が揃っている

**用途の住み分け**(典型例):
- ステージング / 本番 → リモート k8s(k3s / EKS / GKE 等) + GitOps(ArgoCD / Flux)
- ローカルでの統合動作確認 → docker compose で十分(多くの場合)
- **k8s manifest の事前検証 / Helm chart の動作確認** → OrbStack k8s が便利。CI に乗せる前に手元で `kubectl apply` して挙動を見る

## Linux machines(VM)

```bash
orb create ubuntu my-vm      # Ubuntu VM を作成(<1 分)
orb -m my-vm <command>       # VM 内でコマンド実行(SSH キー自動転送)
ssh my-vm@orb                # 直接 SSH(multiplexed)
```

- Mac のファイルは VM 内の `/mnt/mac` で参照
- VM 内のファイルは Mac の `~/OrbStack/<machine>` で参照
- Docker container では再現しにくい OS 固有の検証(systemd、kernel module、ネットワーク機器など)に使う

## Docker Desktop / Rancher Desktop との違い

| 項目 | OrbStack | Docker Desktop |
|---|---|---|
| 起動時間 | ~2s | 数十秒 |
| バックエンド | カスタム軽量 VM(Apple Silicon 最適化) | LinuxKit / qemu |
| メモリ | 必要最小限のみ | 固定割り当て(数 GB) |
| `*.orb.local` 自動 DNS | あり | なし |
| ローカル image を k8s から参照 | 自動 | registry push 必要 |
| ライセンス | 個人無料、商用は有償 | Docker 社の有償ライセンス |

商用ライセンスの判断は利用組織の責任。コード書くときは挙動の差だけ意識すれば OK。

## トラブル時の確認

公式 FAQ / トラブルシューティングは <https://docs.orbstack.dev/> 配下。
ドキュメントが整理されているので、推測で書く前に必ず引く(error handling ルールと同じ)。
