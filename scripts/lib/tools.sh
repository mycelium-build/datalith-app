#!/usr/bin/env bash
# Sourced by scripts that need a pinned cargo tool. ensure_cargo_tool makes the version pinned in
# scripts/tool-versions.env available, installing it once into target/tools (git-ignored,
# per clone) when the cargo-<name> on PATH is missing or another version.
# In CI the workflows install the pinned versions first, so this is a no-op there.

# shellcheck source=../tool-versions.env
source "$(dirname "${BASH_SOURCE[0]}")/../tool-versions.env"

TOOLS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/target/tools"
export PATH="$TOOLS_ROOT/bin:$PATH"

# ensure_cargo_tool <crate> <version> [extra `cargo install` arguments...]
ensure_cargo_tool() {
    local crate="$1" version="$2"
    shift 2
    local installed
    installed="$(cargo "${crate#cargo-}" --version 2>/dev/null | awk '{print $NF}' || true)"
    if [[ "$installed" == "$version" ]]; then
        return 0
    fi
    echo "Installing ${crate} ${version} into target/tools (found '${installed:-none}'); this happens once per version." >&2
    cargo install --locked --quiet --root "$TOOLS_ROOT" "$@" "${crate}@${version}"
}
