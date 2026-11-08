#!/usr/bin/env bash
# Prints the tool versions pinned in [package.metadata.tools] of Cargo.toml as
# NAME_VERSION=value lines (cargo-deny -> CARGO_DENY_VERSION), for `eval "$(...)"` in scripts
# and `>> "$GITHUB_ENV"` in workflows.
set -euo pipefail

manifest="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/Cargo.toml"

versions="$(awk '
    /^\[/ { in_tools = ($0 == "[package.metadata.tools]"); next }
    in_tools && /^[A-Za-z0-9_-]+[[:space:]]*=[[:space:]]*"/ {
        name = $0; sub(/[[:space:]]*=.*/, "", name)
        value = $0; sub(/^[^"]*"/, "", value); sub(/".*/, "", value)
        gsub(/-/, "_", name)
        print toupper(name) "_VERSION=" value
    }' "$manifest")"

if [[ -z "$versions" ]]; then
    echo "no [package.metadata.tools] entries in $manifest" >&2
    exit 1
fi
printf '%s\n' "$versions"
