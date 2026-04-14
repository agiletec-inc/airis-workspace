# Architecture Invariants（設計不変条件）

<!-- Scope: runtime apps and services in an airis-managed workspace. Complements manifest.toml SoT for workspace/tooling. -->

ワークスペース構成の機械可読な真実は引き続き `manifest.toml` が担う。本書は **アプリ実行時の状態・設定の扱い** に関する不変条件を定める。人間向けというより **エージェントとレビュー双方のガードレール** として使う。

---

## 1. 適用範囲

- **対象**: DB や永続ストアで管理する設定・テナント境界・機能フラグなど、アプリの振る舞いを変える「運用設定」。
- **非対象**: CLI のコマンドガード、`manifest.toml` 由来の生成物、純粋なアルゴリズムや UI のみの都合。
- **製品固有のキー名やスキーマ** は各リポジトリ側で増やす。ここでは **原則だけ** を共通化する。

---

## 2. 不変条件一覧（要約）

| ID | 原則 |
|----|------|
| INV-1 | DB 等で SoT と宣言した領域では **silent fallback を禁止**（未設定は明示的に失敗）。 |
| INV-2 | 設定解決は **単一の公開 API** に集約し、散在参照を禁止。 |
| INV-3 | **`DEFAULT_*` 定数によるドメイン設定の暗黙デフォルト** を新規に増やさない（既存は削減計画へ）。 |
| INV-4 | **設定層は fail-fast**、**ランタイム処理層** でだけ可用性のフォールバックを検討する（層を逆にしない）。 |
| INV-5 | 仕様は **結合テスト（実 DB / エミュレータ）** で「欠損時に落ちる」「DB 値が反映される」を固定する。 |

---

## 3. ルール（DO / DON'T / WHY）

### INV-1: Single Source of Truth と silent fallback 禁止

**Rule（原則）**  
DB（またはチームが SoT と合意した永続ストア）で管理する設定は、**そのストアを唯一の真実** とみなす。

**Do（やる）**

- 値がない・取れない場合は **明示的なドメインエラー**（ログに理由が残る）で止める。
- 一時的な移行が必要なら **フラグ・マイグレーション期** を明示し、期限と削除タスクを紐付ける。

**Don't（やらない）**

- `or default` / `get(..., default=...)` などで **設定欠損を隠す**。
- `try/except` で握りつぶし、**別ソース（古い env・ハードコード）に黙って逃がす**。
- 「とりあえず動く」ための **silent continue**。

**Why（理由）**  
silent fallback は不整合を隠し、UI や運用の期待と実行時挙動をずらす。原因特定コストが跳ね上がる。

---

### INV-2: 設定解決の単一入口

**Rule（原則）**  
ビジネスロジックから **直接** `os.getenv` / 生の ORM 散発クエリ / 複数のヘルパに設定を読ませない。

**Do（やる）**

- チームで名前を決めた **1 つの解決モジュール**（例: `resolve_config(org_id, key)` 相当）だけを通す。
- キャッシュ·観測可能性·監査ログが必要ならその層に閉じ込める。

**Don't（やらない）**  
同一キーを複数パスで解決する（優先順位の民間合意が増える）。

**Why（理由）**  
散在が「どの経路が真実か」を壊し、エージェントが既存の悪いパターンをコピーしやすくなる。

---

### INV-3: `DEFAULT_*` とハードコードの温床

**Rule（原則）**  
**新規** の `DEFAULT_ORG_ID` 型の暗黙デフォルトを置かない。既存は削除または DB SoT へ寄せる。

**Why（理由）**  
組織境界・テナント分離と相性が最悪で、横展開すると一気に感染する。

---

### INV-4: fail-fast する層と許容する層

**Rule（原則）**

- **設定解決・入力検証** → **fail-fast**（可用性より正しさ）。
- **リクエスト処理・ジョブ実行** など → チーム方針に従い、限定的な resilience を許容し得る。

層を逆（設定で手握りつぶし、下流でエラーハンドリング地獄）は **禁止**。

---

### INV-5: テストで物理的に殺す

**Rule（原則）**  
SoT が DB の機能は、少なくとも次を **実装レベル** で固定する。

**Do（やる）**

- 必要キーが無い状態で **明示エラー** になること。
- DB に入れた値が **実際に出力・副作用に反映** されること。

**プロジェクト方針**: 外部 DB 等のモックで挙動をごまかさない（倉庫のテスト方針に従う）。

---

## 4. 三層モデル（どこに何を書くか）

1. **モノレポ共通（本書）**  
   原則・禁止事項・PR 観点・テストの骨格。
2. **各プロダクト配下**  
   SoT となるテーブル・キー・ドメイン例外·廃止予定のレガシー。
3. **CI / pre-commit**  
   単純パターンの機械検知（grep / 静的解析）と結合テストの必須化。

CI での単純 grep は **偽陽性·偽陰性** がある。完璧な規則検出ではなく **網の目** として補完し、最終判断は設計レビューとテストに任せる。

---

## 5. PR テンプレ用チェックリスト（コピペ可）

```markdown
## 設計チェック（Architecture invariants）

- [ ] DB（合意した SoT）以外に「本番の真実」を置いていない
- [ ] silent fallback / silent continue を追加していない
- [ ] 未設定・不正値のとき明示エラーになる（握りつぶしていない）
- [ ] 設定参照は単一入口（新たなバラ読みを増やしていない）
- [ ] `DEFAULT_*` 系のハードコードを新規に増やしていない
- [ ] SoT 変更があれば結合テストで欠損時・上書き時をカバーした
```

---

## 6. コードの形の例（英語コメント）

意図の共有用。製品の言語·フレームワークに合わせて置き換える。

```python
# resolve_config is the ONLY public entry for DB-backed settings.


def resolve_config(org_id: str, key: str) -> str:
    row = db.fetch_config(org_id, key)
    if row is None:
        # Explicit failure — no silent fallback to env or constants.
        raise ConfigError(f"missing config: org={org_id} key={key}")
    return row.value
```

```python
# Integration: missing config must abort the use case.


def test_missing_config_fails(real_db, org_id):
    with pytest.raises(ConfigError):
        generate_email(org_id=org_id)  # no seed row


def test_db_value_applies(real_db, org_id):
    real_db.upsert_config(org_id, "sender_name", "A")
    body = generate_email(org_id=org_id)
    assert "A" in body
```

---

## 7. 監査・インサイト用の一文（社内共有）

現在の問題は特定エージェントの誤実装ではなく、「DB を Source of Truth としない設計」と「silent fallback による不整合隠蔽」がガードなしで混入·増殖した構造的問題である。対応は個別修正ではなく、設計レベルでの強制と CI によるブロックが必要である。

---

## 8. 関連ドキュメント

- `docs/ai/PROJECT_RULES.md` — ワークスペースと `manifest.toml` の SoT
- `docs/ai/REVIEW.md` — レビュー観点
- `docs/ai/playbooks/TESTING.md` — テストレベルとモック方針
