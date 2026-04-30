# airis-workspace

**The Transparent Docker-First Proxy for the Vibe Coding Era.**

Airis is an environment orchestrator that acts as a transparent proxy between your host shell and Docker. It ensures a host-hygienic development environment by automatically redirecting commands to Docker whenever a Compose file is detected.

## 🚀 The Core Idea: Smart Shims

With Airis, you don't need to change your muscle memory. If you are in a directory with a `compose.yml`, Airis intercepts your commands and runs them inside the container.

- **Zero Config**: If you have a `compose.yml`, Airis works. No `manifest.toml` required.
- **Transparent Proxy**: Use `pnpm`, `npm`, `uv`, or `python` as usual. If a Docker environment is present, it runs there. If not, it runs natively.
- **Hygiene Enforcement**: Keeps `node_modules`, `target/`, and other build artifacts inside Docker volumes, keeping your host clean.

## 📦 Supported Triggers

Airis automatically detects environment intent by looking for:
- `compose.yaml` / `compose.yml`
- `docker-compose.yaml` / `docker-compose.yml`
- `manifest.toml` (Optional: for monorepo orchestration and advanced policies)

## 🛠️ Getting Started

1. **Install Airis CLI** — pick whichever fits your toolchain:
   ```bash
   # macOS / Linux: Homebrew
   brew install agiletec-inc/tap/airis-workspace

   # Any platform: shell installer
   curl --proto '=https' --tlsv1.2 -LsSf \
     https://github.com/agiletec-inc/airis-workspace/releases/latest/download/airis-workspace-installer.sh | sh

   # Rust users: prebuilt binary via cargo-binstall
   cargo binstall airis-workspace

   # Rust users: build from source
   cargo install airis-workspace
   ```

2. **Install Global Smart-Shims**:
   ```bash
   airis guards install --global
   ```
   *This adds `~/.airis/bin` to your `PATH`. These shims are the "magic" that enables transparent redirection.*

3. **Just Work**:
   Go to any project with a `compose.yml` and run your usual commands:
   ```bash
   pnpm install  # Automatically runs: docker compose exec workspace pnpm install
   python main.py # Automatically runs: docker compose exec workspace python main.py
   ```

## 🧠 Advanced Features (manifest.toml)

While optional, a `manifest.toml` allows you to:
- **Orchestrate Monorepos**: Start multiple apps and infrastructure (Supabase, Traefik) with a single `airis up`.
- **Config Compilation**: Automatically generate `compose.yaml`, `tsconfig.json`, and `package.json` from a single source of truth.
- **Policy Gates**: Forbid LLMs or humans from running dangerous commands on the host.

## 📖 Commands

- `airis up`: Start the Docker environment (via Manifest or Compose).
- `airis run <task>`: Execute a task (delegates to Docker if needed).
- `airis shell`: Enter the primary workspace container.
- `airis guards status --global`: Check the status of your smart-shims.
- `airis workspace uninstall`: Safely remove Airis hooks and generated files from a repo.

---

License: MIT
