#!/usr/bin/env bash
# prek hook: runs scripts/licenses/check.sh when dependency or license files change.
# Stale notices are regenerated and staged, so the commit goes through with them.
# Advisories are skipped: they depend on the advisory database, not on this commit,
# and CI still checks them.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# The license scripts install the pinned cargo-deny and cargo-about themselves; they need uv for
# Python.
if ! command -v uv >/dev/null 2>&1; then
    echo "Dependency or license files changed: the license check needs uv (https://docs.astral.sh/uv/)." >&2
    exit 1
fi

# The license scripts need Python 3.11+: uv provides it for the whole script.
generated=(THIRD-PARTY-NOTICES.md scripts/licenses/about.toml)
if ! LICENSE_CHECK_SKIP_ADVISORIES=1 uv run --no-project --python '>=3.11' -- scripts/licenses/check.sh; then
    # check.sh runs cargo-deny first and exits before regenerating when it fails, so changed
    # notices mean cargo-deny passed and the only problem was stale generated material.
    if git diff --quiet -- "${generated[@]}"; then
        exit 1
    fi
    git add -- "${generated[@]}"
    echo "License material was stale: regenerated and staged ${generated[*]}." >&2
fi
