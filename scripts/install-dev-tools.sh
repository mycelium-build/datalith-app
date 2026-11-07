#!/usr/bin/env bash
# Installs the license tools at the versions pinned in scripts/tool-versions.env.
# Run it again after a tool-versions bump.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=tool-versions.env
source "$REPO_ROOT/scripts/tool-versions.env"

cargo install --locked "cargo-deny@${CARGO_DENY_VERSION}"
# cargo-about only builds its binary with the cli feature.
cargo install --locked --features cli "cargo-about@${CARGO_ABOUT_VERSION}"
