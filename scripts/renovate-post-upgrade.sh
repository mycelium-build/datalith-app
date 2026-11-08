#!/usr/bin/env bash
# Renovate runs this after each Cargo update (postUpgradeTasks in renovate.json), in the
# Renovate workflow, to keep THIRD-PARTY-NOTICES.md in the same PR as the update.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# shellcheck source=tool-versions.env
source scripts/tool-versions.env
required="$CARGO_ABOUT_VERSION"
installed="$(cargo about --version 2>/dev/null | awk '{print $NF}' || true)"
if [[ "$installed" != "$required" ]]; then
    # cargo-about only builds its binary with the cli feature.
    cargo install --locked --features cli "cargo-about@${required}"
fi

scripts/licenses/generate.sh
