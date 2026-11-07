#!/usr/bin/env bash
# prek hook: runs scripts/licenses/check.sh when dependency or license files change.
# Advisories are skipped: they depend on the advisory database, not on this commit,
# and CI still checks them.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

missing=()
command -v cargo-deny >/dev/null 2>&1 || missing+=("cargo install --locked cargo-deny@0.20.2")
command -v cargo-about >/dev/null 2>&1 || missing+=("cargo install --locked --features cli cargo-about@0.9.1")
# scripts/licenses/*.py use tomllib (Python 3.11+); macOS ships an older python3.
python3 -c 'import tomllib' >/dev/null 2>&1 \
    || missing+=("python3 >= 3.11 first on PATH (e.g. brew install python, or uv python install 3.12)")
if ((${#missing[@]})); then
    echo "Dependency or license files changed, but the license tools are missing. Install with:" >&2
    printf '  %s\n' "${missing[@]}" >&2
    exit 1
fi

if ! LICENSE_CHECK_SKIP_ADVISORIES=1 scripts/licenses/check.sh; then
    if ! git diff --quiet -- THIRD-PARTY-NOTICES.md scripts/licenses/about.toml; then
        echo "License material was stale and has been regenerated." >&2
        echo "Review it, then: git add THIRD-PARTY-NOTICES.md scripts/licenses/about.toml" >&2
    fi
    exit 1
fi
