#!/usr/bin/env bash
# Installs the license tools at the versions pinned in [package.metadata.tools] of Cargo.toml.
# Run it again after Renovate bumps one of them.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
eval "$("$REPO_ROOT/scripts/tool-versions.sh")"

cargo install --locked "cargo-deny@${CARGO_DENY_VERSION}"
# cargo-about only builds its binary with the cli feature.
cargo install --locked --features cli "cargo-about@${CARGO_ABOUT_VERSION}"
