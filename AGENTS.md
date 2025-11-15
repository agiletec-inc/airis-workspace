# Workspace / Dev Rules

1. `MANIFEST.toml` is the single source of truth for dev targets, infra dependencies, startup order, and build/lint/test scopes.
2. `workspace.yaml` is auto-generated metadata—do not edit it or rely on it for dev/CI logic.
3. `justfile`, `docker-compose.yml`, and any tooling are thin wrappers that only read values from `MANIFEST.toml`.
4. Dev orchestration logic may not rely on Node scripts, ad-hoc JSON parsing, or other standalone scripts—those are banned.
5. To change workspace behavior you must edit `MANIFEST.toml`; no other file may redefine those lists.
