{
  "_generated": {
    "by": "airis init",
    "from": "manifest.toml",
    "warning": "⚠️  DO NOT EDIT - Update manifest.toml then rerun `airis init`"
  },
  "dependencies": {},
  "devDependencies": {},
  "name": "airis-workspace",
  "packageManager": "pnpm@10.22.0",
  "private": true,
  "scripts": {
    "build": "echo 'Run: just build-<app-name>'",
    "dev": "echo 'Run: just dev-<app-name>'",
    "lint": "echo 'Run: just lint'",
    "test": "echo 'Run: just test'"
  },
  "type": "module",
  "version": "0.0.0",
  "workspaces": [
    "apps/*",
    "libs/*",
    "packages/*"
  ]
}