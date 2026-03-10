/// Default AI workflow rules for ~/.claude/rules/
///
/// Each rule is embedded as a static string and written to disk
/// by `airis rules init` / `airis rules update`.

pub struct DefaultRule {
    pub name: &'static str,
    pub filename: &'static str,
    pub description: &'static str,
    pub content: &'static str,
}

pub const DEFAULT_RULES: &[DefaultRule] = &[
    DefaultRule {
        name: "docker-first",
        filename: "airis-docker-first.md",
        description: "Docker-first development philosophy",
        content: r#"# Docker-First Development

Docker-first 開発。ホストで package manager (npm/yarn/pnpm) を直接実行禁止。
airis コマンド経由で Docker 内で実行すること。

## 禁止コマンド (ホスト実行禁止)

| 禁止 | 代替 |
|------|------|
| `npm install` | `airis install` |
| `pnpm install` | `airis install` |
| `yarn install` | `airis install` |
| `pnpm add <pkg>` | `airis shell` → `pnpm add <pkg>` |
| `pnpm dev` | `airis dev` |
| `pnpm build` | `airis build` |
| `pnpm test` | `airis test` |
| `pnpm lint` | `airis lint` |
| `pnpm typecheck` | `airis typecheck` |
| `docker compose up` | `airis up` |
| `docker compose exec workspace <cmd>` | `airis shell` → `<cmd>` |

## 許可コマンド (ホストOK)

`airis *`, `git`, `doppler`, `gh`, `supabase`

## 原則

- `manifest.toml` の `[commands]` が全コマンドの定義元
- `node_modules` がホストに存在する場合: `airis clean && airis install`
"#,
    },
    DefaultRule {
        name: "commit-format",
        filename: "airis-commit-format.md",
        description: "Conventional Commits format",
        content: r#"# Commit Message Format

```
<type>(<scope>): <subject>

<body>

Co-Authored-By: Claude <user>@anthropic.com
```

## Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no code change |
| `refactor` | Code restructure, no feature change |
| `test` | Adding or fixing tests |
| `chore` | Maintenance, build, etc. |
"#,
    },
    DefaultRule {
        name: "forbidden-patterns",
        filename: "airis-forbidden-patterns.md",
        description: "Forbidden code patterns",
        content: r#"# Forbidden Patterns

| Pattern | Alternative |
|---------|-------------|
| `console.log` | Use a proper logger |
| Hardcoded secrets | Use environment variables |
| Excessive `any` type | Explicitly type everything |
| TODO comments left behind | Create issues instead |
| `.env.local`, `.env.development` 等の作成 | 絶対禁止。`.env` / `.env.example` のみ許可 |
"#,
    },
    DefaultRule {
        name: "on-failure",
        filename: "airis-on-failure.md",
        description: "Error recovery protocol",
        content: r#"# On Failure Protocol

1. **STOP** — ミスを重ねない
2. **Root Cause Analysis** — なぜ失敗したか分析する
3. **Retry** — 修正したアプローチで再試行する
"#,
    },
    DefaultRule {
        name: "test-first-bugfix",
        filename: "airis-test-first-bugfix.md",
        description: "Test-first bug fix workflow",
        content: r#"# Test-First Bug Fix

バグ修正時は必ず以下の手順を守ること：

1. **再現テストを先に書く** — 修正前に FAIL することを確認
2. **修正を実施**
3. **テストが PASS することを確認**
4. **既存テストも全て PASS すること**

テストなしの修正は未完了。「直しました」だけでは完了ではない。
再発防止のテストがセットで初めて完了。
"#,
    },
    DefaultRule {
        name: "pre-push-checklist",
        filename: "airis-pre-push-checklist.md",
        description: "Pre-push verification checklist",
        content: r#"# Pre-Push Checklist

push する前に必ず検証を実行すること：

```bash
airis check  # lint + typecheck + test を一括実行
```

## 禁止事項

- テストを実行せずに push すること
- lint エラーを放置すること
- 型エラーを放置すること
- CI 失敗を放置すること
"#,
    },
];
