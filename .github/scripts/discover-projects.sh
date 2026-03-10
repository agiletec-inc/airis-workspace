#!/usr/bin/env bash
# Discover deployable projects (those with a Dockerfile) and detect changes.
#
# Usage:
#   bash .github/scripts/discover-projects.sh <base_sha> <head_sha> <env_suffix>
#
# Outputs (via $GITHUB_OUTPUT or stdout):
#   matrix      - JSON array of {name, dir, parent_dir}
#   has_targets - "true" | "false"

set -euo pipefail

BASE_SHA="${1:-}"
HEAD_SHA="${2:-HEAD}"
ENV_SUFFIX="${3:-stg}"

SCAN_DIRS="apps libs products"

# ------------------------------------------------------------------
# 1. Collect all projects that have a Dockerfile
# ------------------------------------------------------------------
all_names=""
all_dirs=""
all_parents=""
count=0

for scan_dir in $SCAN_DIRS; do
  [ -d "$scan_dir" ] || continue
  for dir in "$scan_dir"/*/; do
    [ -f "${dir}Dockerfile" ] || continue
    name="$(basename "$dir")"

    # Name collision detection: apps/foo and products/foo both exist
    if echo "$all_names" | grep -qw "$name" 2>/dev/null; then
      echo "::error::Name collision detected: '$name' exists in multiple scan directories"
      exit 1
    fi

    all_names="${all_names:+$all_names }$name"
    all_dirs="${all_dirs:+$all_dirs }${dir%/}"
    all_parents="${all_parents:+$all_parents }$scan_dir"
    count=$((count + 1))
  done
done

if [ "$count" -eq 0 ]; then
  echo "No deployable projects found (no Dockerfile in $SCAN_DIRS)."
  if [ -n "${GITHUB_OUTPUT:-}" ]; then
    echo "matrix=[]" >> "$GITHUB_OUTPUT"
    echo "has_targets=false" >> "$GITHUB_OUTPUT"
  fi
  exit 0
fi

# ------------------------------------------------------------------
# 2. Detect changed files (skip if no BASE_SHA -> deploy all)
# ------------------------------------------------------------------
changed_files=""
if [ -n "$BASE_SHA" ]; then
  changed_files="$(git diff --name-only "$BASE_SHA" "$HEAD_SHA" 2>/dev/null || true)"
fi

# Check if libs/ changed (triggers full deploy for now)
libs_changed=false
if [ -n "$changed_files" ]; then
  if echo "$changed_files" | grep -q '^libs/'; then
    libs_changed=true
  fi
fi

# Check root-level shared files that affect all builds
root_changed=false
if [ -n "$changed_files" ]; then
  if echo "$changed_files" | grep -qE '^(package\.json|pnpm-lock\.yaml|pnpm-workspace\.yaml|\.npmrc)$'; then
    root_changed=true
  fi
fi

# ------------------------------------------------------------------
# 3. Build matrix of affected projects
# ------------------------------------------------------------------
# Convert space-separated lists to arrays via positional params
set -- $all_names
names_arr=("$@")
set -- $all_dirs
dirs_arr=("$@")
set -- $all_parents
parents_arr=("$@")

matrix="["
first=true

for i in $(seq 0 $((count - 1))); do
  name="${names_arr[$i]}"
  dir="${dirs_arr[$i]}"
  parent="${parents_arr[$i]}"
  include=false

  if [ -z "$BASE_SHA" ]; then
    # No base SHA -> include all (first deploy or manual trigger)
    include=true
  elif [ "$libs_changed" = "true" ] || [ "$root_changed" = "true" ]; then
    # Shared dependency changed -> include all
    # TODO: Use `pnpm ls --filter` for fine-grained dependency analysis
    include=true
  elif echo "$changed_files" | grep -q "^${dir}/"; then
    # Direct changes to this project
    include=true
  fi

  if [ "$include" = "true" ]; then
    if [ "$first" = "true" ]; then
      first=false
    else
      matrix+=","
    fi
    matrix+="{\"name\":\"${name}\",\"dir\":\"${dir}\",\"parent_dir\":\"${parent}\"}"
  fi
done

matrix+="]"

has_targets="false"
if [ "$first" = "false" ]; then
  has_targets="true"
fi

# ------------------------------------------------------------------
# 4. Output
# ------------------------------------------------------------------
echo "Discovered projects: $matrix"
echo "Has targets: $has_targets"

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "matrix=${matrix}" >> "$GITHUB_OUTPUT"
  echo "has_targets=${has_targets}" >> "$GITHUB_OUTPUT"
else
  # Local testing output
  echo "---"
  echo "MATRIX=$matrix"
  echo "HAS_TARGETS=$has_targets"
fi
