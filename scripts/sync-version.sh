#!/usr/bin/env bash
set -euo pipefail

# Iris MonoLep (Airis Monorepo) - manifest-driven versioning
# Syncs all package versions from the root VERSION file.

VERSION=$(cat VERSION | tr -d '[:space:]' | sed 's/v//')
echo "🔄 Syncing version: $VERSION"

# 1. Rust (Cargo.toml)
if [ -f "Cargo.toml" ]; then
    sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml && rm Cargo.toml.bak
    echo "✅ Updated Rust: Cargo.toml"
fi

# 2. Node.js (Root package.json)
if [ -f "package.json" ]; then
    sed -i.bak "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" package.json && rm package.json.bak
    echo "✅ Updated Node: package.json"
fi

# 3. Workspace packages (apps/*, libs/*, packages/*)
for f in {apps,libs,packages}/*/package.json; do
    if [ -e "$f" ]; then
        sed -i.bak "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$f" && rm "$f.bak"
        echo "✅ Updated Workspace: $f"
    fi
done
