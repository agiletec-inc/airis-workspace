# WORKSPACE

`workspace.yaml` は airis-workspace が自動生成するメタデータであり、モノレポの物理構造 (packages / docker image 名 / volume / path) を記述するファイル。人間が直接編集したり dev ロジックを書く場ではない。

## ポリシー

1. `workspace.yaml` はジェネレーターが再生成するので手動変更しない。ロジックや設定は `MANIFEST.toml` に置く。
2. Justfile / docker-compose / エージェントは `workspace.yaml` のメタ情報を参照するだけで、 dev/CI の意思決定には使わない。
3. workspace の構成を変えたい場合は airis-workspace のテンプレートや設定を更新して再生成する。

## 利用例

- パッケージの一覧やフレームワーク判定に `workspace.yaml` を読み、補完や IDE 設定を行う。
- Docker イメージ名、ボリューム設定、path resolution など技術的な事実を引き出す。

## まとめ

- `workspace.yaml`: 自動生成された構造メタデータ。編集禁止。
- `MANIFEST.toml`: 人間が定義する開発/運用ロジックの真実。すべての dev/CI 対象はここで指定。

それぞれの責務を混ぜないことで、再生成しても壊れない安定したワークフローを維持できる。
