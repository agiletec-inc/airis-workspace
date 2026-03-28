# airis-monorepo-mcp Design

**Status**: Design Proposal
**Author**: Claude + Kazuki
**Date**: 2025-11-20

---

## Vision

**現在の問題:**
- プラグインに実装ロジックが散在
- 人間が CLI で叩く処理と LLM が実行する処理が分離
- airis-agent が直接ファイル操作をしている（責務の混在）

**解決策:**
- **airis-monorepo-mcp** を作る
- CLI の本体ロジックを MCP ツールとして公開
- LLM が `uvx` 経由で CLI を"仮想的に"叩けるようにする

---

## Architecture

```
┌─────────────────────────────────────────────┐
│  Human                    LLM (Claude)      │
├─────────────────────────────────────────────┤
│  $ airis init             call tool         │
│  $ airis validate         "workspace_init"  │
│  $ airis sync             "workspace_sync"  │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│      airis-monorepo-mcp (薄いラッパー)      │
├─────────────────────────────────────────────┤
│  • MCP Server (Python or Rust)              │
│  • subprocess.run(["airis", "init"])        │
│  • または Rust ライブラリを直呼び出し        │
└─────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────┐
│     airis-monorepo (Rust CLI/Library)      │
├─────────────────────────────────────────────┤
│  • 本体ロジック                              │
│  • manifest.toml 解析                        │
│  • テンプレート生成                          │
│  • バリデーション                            │
│  • guards チェック                           │
└─────────────────────────────────────────────┘
```

---

## Responsibilities

### airis-monorepo (Rust CLI)
**責務:** モノレポ管理ロジックの本体

- `airis init` - プロジェクト初期化
- `airis validate` - 検証
- `airis workspace sync` - 依存関係同期
- `airis generate types` - 型定義生成
- `airis bump-version` - バージョン管理

### airis-monorepo-mcp
**責務:** CLI を MCP ツールとして公開する薄いラッパー

**ツール一覧:**
- `workspace_init` → `airis init --no-snapshot`
- `workspace_validate_all` → `airis validate all`
- `workspace_validate_manifest` → `airis validate manifest`
- `workspace_validate_deps` → `airis validate deps`
- `workspace_validate_arch` → `airis validate arch`
- `workspace_sync` → `airis workspace sync`
- `workspace_generate_types` → `airis generate types`
- `workspace_bump_version` → `airis bump-version --auto`
- `workspace_status` → `airis status`
- `workspace_doctor` → `airis doctor`

### airis-agent (MCP)
**責務:** 思考・プランニング・オーケストレーション

- リポジトリの状態解析
- 「こういう変更が必要」と判断
- **実行は airis-monorepo-mcp のツールを呼ぶ**

### プラグイン (airis-agent-plugin)
**責務:** UX層、ボタンのラベルだけ

- `/airis:init` - 初期化コマンド
- `/airis:analyze` - 解析コマンド
- `/airis:fix-workspace` - 修正コマンド

**中身:**
```markdown
# /airis:init の実装例

1. airis-agent MCP の `analyze_workspace` を呼ぶ
2. 必要な変更を計画
3. airis-monorepo-mcp の `workspace_init` を呼ぶ
4. 結果を報告
```

---

## MCP Definition

```json
{
  "mcpServers": {
    "airis-monorepo": {
      "command": "uvx",
      "args": [
        "--from",
        "git+https://github.com/agiletec-inc/airis-monorepo-mcp",
        "airis_workspace_mcp"
      ],
      "env": {
        "AIRIS_WORKSPACE_ROOT": "${HOME}/github/airis-monorepo"
      }
    }
  }
}
```

---

## Implementation Options

### Option A: Python MCP Server (推奨)

**メリット:**
- MCP SDK が充実
- 開発速度が速い
- subprocess で CLI を叩くだけなので実装が簡単

**実装例:**
```python
from mcp.server import Server
from mcp.types import Tool
import subprocess
import json

server = Server("airis-monorepo")

@server.list_tools()
async def list_tools():
    return [
        Tool(
            name="workspace_init",
            description="Initialize airis workspace from manifest.toml",
            inputSchema={
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Workspace path"},
                    "no_snapshot": {"type": "boolean", "default": True}
                }
            }
        )
    ]

@server.call_tool()
async def call_tool(name: str, arguments: dict):
    if name == "workspace_init":
        path = arguments.get("path", ".")
        cmd = ["airis", "init"]
        if arguments.get("no_snapshot"):
            cmd.append("--no-snapshot")

        result = subprocess.run(
            cmd,
            cwd=path,
            capture_output=True,
            text=True
        )

        return {
            "content": [
                {
                    "type": "text",
                    "text": result.stdout or result.stderr
                }
            ]
        }
```

### Option B: Rust MCP Server

**メリット:**
- airis-monorepo のライブラリを直接呼び出せる
- 型安全
- パフォーマンス

**デメリット:**
- MCP SDK がまだ experimental
- 開発コストが高い

**実装例:**
```rust
use mcp_server::{Server, Tool};
use airis_workspace::commands;

#[tokio::main]
async fn main() {
    let server = Server::new("airis-monorepo");

    server.add_tool(Tool {
        name: "workspace_init".into(),
        description: "Initialize workspace".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            }
        }),
        handler: |args| {
            let path = args.get("path").unwrap();
            commands::init::run(path)?;
            Ok(json!({"status": "ok"}))
        }
    });

    server.run().await;
}
```

---

## Implementation Roadmap

### Phase 1: MVP (Python MCP Server)

**Goal:** 最小限の動作確認

1. **airis-monorepo-mcp リポジトリ作成**
   ```bash
   mkdir airis-monorepo-mcp
   cd airis-monorepo-mcp
   uv init
   ```

2. **最小限のツール実装**
   - `workspace_init` だけ実装
   - subprocess で `airis init` を叩く

3. **ローカルでテスト**
   ```bash
   uvx --from . airis_workspace_mcp
   ```

4. **airis-agent から呼び出しテスト**
   - airis-agent の tool call で workspace_init を呼ぶ
   - 正しく実行されることを確認

### Phase 2: 主要ツールの追加

**Goal:** よく使うコマンドを網羅

- `workspace_validate_all`
- `workspace_status`
- `workspace_doctor`

### Phase 3: プラグインのリファクタリング

**Goal:** ロジックを全部 MCP に移す

1. **airis-agent-plugin から実装を削除**
   - プロンプトだけ残す
   - 中身は MCP ツール呼び出しに変更

2. **動作確認**
   - `/airis:init` が正しく動く
   - 実装は airis-monorepo-mcp に移っている

### Phase 4: Rust 版の検討 (Optional)

**Goal:** パフォーマンス最適化

- ライブラリ呼び出しに切り替え
- subprocess オーバーヘッド削減

---

## Benefits

### 1. 責務の明確化

```
思考・判断     → airis-agent (MCP)
実際の作業     → airis-monorepo-mcp
本体ロジック   → airis-monorepo (Rust CLI)
UX            → プラグイン
```

### 2. 実装の一元化

- CLI と MCP で同じロジックを使う
- バグ修正が一箇所で済む
- 機能追加も一箇所

### 3. 人間と LLM で同じツールを使う

```bash
# 人間
$ airis init

# LLM
call tool "workspace_init"
```

**同じ結果、同じロジック**

### 4. プラグインの軽量化

- ロジックがない = メンテナンス不要
- プロンプトだけ = 理解しやすい
- MCP に寄せる = 再利用性が高い

---

## Migration Strategy

### Before (現状)

```
プラグイン (/airis:init)
  ├─ ロジック実装 (直接ファイル操作)
  ├─ エラーハンドリング
  └─ 結果の整形

airis-agent
  ├─ 思考
  └─ 直接ファイル操作 (責務混在)
```

### After (目標)

```
プラグイン (/airis:init)
  └─ プロンプトのみ: "airis-monorepo-mcp の workspace_init を呼べ"

airis-agent (MCP)
  ├─ 思考・プランニング
  └─ tool call "workspace_init"

airis-monorepo-mcp (MCP)
  └─ subprocess.run(["airis", "init"])

airis-monorepo (Rust CLI)
  └─ 本体ロジック
```

---

## Next Steps

1. **Phase 1 の実装開始**
   - `airis-monorepo-mcp` リポジトリ作成
   - `workspace_init` ツールだけ実装
   - ローカルでテスト

2. **airis-agent との統合**
   - airis-agent から workspace_init を呼ぶ
   - 動作確認

3. **グローバル設定に追加**
   ```json
   {
     "mcpServers": {
       "airis-monorepo": {
         "command": "uvx",
         "args": ["--from", "git+https://github.com/agiletec-inc/airis-monorepo-mcp", "airis_workspace_mcp"]
       }
     }
   }
   ```

4. **残りのツールを順次追加**

---

## Conclusion

**airis-monorepo-mcp** を作ることで：

✅ 責務が明確になる
✅ 実装が一元化される
✅ 人間と LLM が同じツールを使える
✅ プラグインがシンプルになる

**設計と実装が綺麗に揃う** 🚀
