# airis-workspace

**AI doesn't break your build system. It breaks your environment.**

[![Crates.io](https://img.shields.io/crates/v/airis.svg)](https://crates.io/crates/airis)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/agiletec-inc/airis-workspace/actions/workflows/ci.yml/badge.svg)](https://github.com/agiletec-inc/airis-workspace/actions/workflows/ci.yml)

![airis demo](assets/airis-demo.gif)

AI pair-programmers are powerful, but they often have **poor environment hygiene**. They run `pnpm install` on the host, break bind mounts, leak credentials, and destroy reproducibility. 

**airis** is the Docker-first environment orchestrator for the vibe coding era. It doesn't replace your build tools—it makes them safe to use in AI-assisted workflows by automating the "Docker hygiene" that's too tedious for humans to maintain manually.

---

## Why airis exists

Nx and Turborepo optimize builds. **airis ensures your environment is host-hygienic.**

We wanted Docker-first development for one simple reason: **reproducibility**. But when AI agents write code, the Docker boundary often collapses:

1. **Host Pollution.** Agents run commands on the host by mistake. Node modules leak between host and container.
2. **Filesystem Decay.** Bind mounts for generated files (`.next`, `dist`) slow down macOS and corrupt states.
3. **Manual Config Drift.** Manually updating `compose.yaml` is slow. One mistake and the "truth" is gone.

airis enforces environment hygiene structurally. It uses repository conventions and a thin manifest to compile a perfect Docker environment.

```
┌─────────────────────────────────────────────┐
│  Your Build Tool (Turborepo / NX / Bazel)   │  ← Task orchestration, caching
├─────────────────────────────────────────────┤
│  AIRIS Workspace                            │  ← Environment Orchestrator,
│                                             │    Command Guards, Hygiene Enforcer
├─────────────────────────────────────────────┤
│  Docker / Compose                           │  ← Containers, Volumes, Networking
└─────────────────────────────────────────────┘
```

**Works with Nx and Turborepo. Does not replace them.**

---

## Core Pillars

### 1. Thin Manifest & Strong Convention
Airis follows **Convention over Configuration**. It automatically discovers projects in `apps/*` and `libs/*`. Your `manifest.toml` stays thin—only containing exceptions and intent like custom ports or explicit dependencies.

### 2. Automatic Artifact Isolation (Hygiene)
Airis understands the "hygiene" of your stack. It automatically isolates build artifacts (`node_modules`, `.next`, `target`, `.venv`) in **named volumes**. This keeps your host filesystem clean and ensures maximum performance on macOS/Linux.

### 3. One-Shot Boot (`airis up`)
`airis up` is the universal entry point. It handles secret injection (e.g., Doppler), syncs configuration, installs dependencies inside the container, and starts your services in one go. No more "I forgot to run pnpm install inside Docker" errors.

### 4. AI-Safe Command Guards
Airis provides **Command Guards** that intercept host-polluting commands (like `npm install`) and redirect them to Docker or warn the agent. It ensures AI stays inside the container where it belongs.

---

## Quick Start

### Install

```bash
cargo install airis
```

### Bootstrap a Workspace

```bash
mkdir my-monorepo && cd my-monorepo
# Use the MCP tool to initialize or create a thin manifest.toml
airis gen              # Sync config based on conventions
airis up               # One-shot boot: install deps & start services
```

---

## The AIRIS Suite

airis-workspace is part of the **AIRIS Suite**: Infrastructure for the vibe coding era.

- **AIRIS Workspace (this repo):** The environment orchestrator.
- **[AIRIS MCP Gateway](https://github.com/agiletec-inc/airis-mcp-gateway):** Unified tool access with 90% token reduction.
- **MindBase:** Durable semantic memory substrate.
- **AIRIS Keeper:** Scoped credential control plane.

---

## Documentation

- [manifest.toml Reference](docs/CONFIG.md)
- [Commands Guide](docs/commands.md)

## License

MIT — see [LICENSE](LICENSE).

---

[@agiletec-inc](https://github.com/agiletec-inc)
