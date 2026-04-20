# airis-workspace

**AI doesn't break your build system. It breaks your environment.**

[![Crates.io](https://img.shields.io/crates/v/airis.svg)](https://crates.io/crates/airis)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/agiletec-inc/airis-workspace/actions/workflows/ci.yml/badge.svg)](https://github.com/agiletec-inc/airis-workspace/actions/workflows/ci.yml)

![airis demo](assets/airis-demo.gif)

AI pair-programmers are powerful, but they have **terrible environment hygiene**. They run `npm install` on the host, break bind mounts, leak credentials, and destroy reproducibility. 

**airis** is the deterministic environment compiler for Docker-first monorepos. It doesn't replace your build tools—it makes them safe to use in AI-assisted workflows.

---

## Why airis exists

Nx and Turborepo optimize builds. **airis makes the environment deterministic.**

We wanted Docker-first development for one simple reason: **reproducibility**. But when your AI pair-programmer writes the code, Docker-first breaks in ways it never did with human developers:

1. **The AI forgets the rules.** It runs commands on the host. Dependencies leak. Reproducibility is gone.
2. **It destroys the filesystem.** Bind mounts and `node_modules` drift apart.
3. **Docker-first is hard to keep correct.** One manual change to `compose.yml` and the "truth" is gone.

airis enforces Docker-first hygiene structurally. Adopting `manifest.toml` as your Source of Truth guarantees that AI agents operate in a locked-down, reproducible environment.

```
┌─────────────────────────────────────────────┐
│  Your Build Tool (Turborepo / NX / Bazel)   │  ← Task orchestration, caching
├─────────────────────────────────────────────┤
│  AIRIS Workspace                            │  ← Environment SoT, Config Compiler,
│                                             │    Command Guards, Hygiene Enforcer
├─────────────────────────────────────────────┤
│  Docker / Compose                           │  ← Containers, Volumes, Networking
└─────────────────────────────────────────────┘
```

**Works with Nx and Turborepo. Does not replace them.**

---

## Core Pillars

### 1. Deterministic Compilation
`manifest.toml` is your **Single Source of Truth**. Every workspace configuration is derived from it:
- `package.json` (Root & Apps)
- `pnpm-workspace.yaml`
- `compose.yml` (With automatic artifact isolation)
- `tsconfig.json`
- CI/CD workflows

### 2. True Docker-First Isolation
airis understands the "hygiene" of your stack. It automatically isolates build artifacts (`node_modules`, `.next`, `target`, `.venv`) in **named volumes**, preventing host-filesystem pollution and ensuring maximum performance on macOS by leveraging Docker-native filesystems.

### 3. AI-Safe Execution & Quality Gates
Airis implements **Command Guards** that intercept dangerous or host-polluting commands and routes them through Docker. It now includes a mandatory **Verification Gate** (`airis verify`) that enforces quality checks inside Docker before any task is considered complete by AI agents.

### 4. Modern Developer Experience
- **Stack-Driven Development**: Declare `stack = ["nextjs"]` and airis handles the rest—volumes, verify commands, and environments.
- **Indicatif Progress**: Beautiful parallel task execution with spinners and progress bars.
- **Miette Diagnostics**: Rich, colorful error reporting that points to the exact line in your manifest.

---

## Quick Start

### Install

```bash
cargo install airis
# Then install shell completion (optional)
airis completion zsh > ~/.zsh/completion/_airis
```

### Initialize a Workspace

```bash
mkdir my-monorepo && cd my-monorepo
airis init --write     # Auto-discovers frameworks and creates manifest.toml
airis up               # Sync config, install deps inside Docker, and start services
```

---

## CLI Highlights

- `airis up`: The "One Command" to sync everything and start development.
- `airis doctor`: Diagnose and auto-repair drift between manifest and environment.
- `airis hooks install`: Install non-destructive Git hooks that auto-bump versions and prevent `.env` leaks.
- `airis completion <shell>`: Generate autocompletion scripts.
- `airis mcp`: Start the MCP server for integration with Claude Code, Cursor, etc.

---

## The AIRIS Suite

airis-workspace is part of the **AIRIS Suite**: Infrastructure for the vibe coding era.

- **AIRIS Workspace (this repo):** The deterministic environment compiler.
- **[AIRIS MCP Gateway](https://github.com/agiletec-inc/airis-mcp-gateway):** Unified tool access with 90% token reduction.
- **[MindBase](https://github.com/agiletec-inc/mindbase):** Durable semantic memory substrate.
- **[AIRIS Keeper](https://github.com/agiletec-inc/airis-keeper):** Scoped credential control plane.

---

## Documentation

- [manifest.toml Reference](docs/CONFIG.md)
- [Commands Guide](docs/commands.md)
- [Changelog](CHANGELOG.md)

## License

MIT — see [LICENSE](LICENSE).

---

[@agiletec-inc](https://github.com/agiletec-inc)
