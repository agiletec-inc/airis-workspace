#!/usr/bin/env bash
# Sync Cargo.toml version to all npm/*/package.json files
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION=$(grep -m1 '^version' "$REPO_ROOT/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')

if [ -z "$VERSION" ]; then
  echo "Error: Could not read version from Cargo.toml" >&2
  exit 1
fi

echo "Syncing version: $VERSION"

# Update all npm package versions
for pkg in "$REPO_ROOT"/npm/*/package.json; do
  # Update the package's own version
  tmp=$(mktemp)
  jq --arg v "$VERSION" '.version = $v' "$pkg" > "$tmp" && mv "$tmp" "$pkg"
  echo "  Updated $(basename "$(dirname "$pkg")")/package.json"
done

# Update optionalDependencies in the main package
MAIN_PKG="$REPO_ROOT/npm/airis/package.json"
tmp=$(mktemp)
jq --arg v "$VERSION" '
  .optionalDependencies |= with_entries(.value = $v)
' "$MAIN_PKG" > "$tmp" && mv "$tmp" "$MAIN_PKG"
echo "  Updated airis/package.json optionalDependencies"

echo "Done. All npm packages set to v$VERSION"
