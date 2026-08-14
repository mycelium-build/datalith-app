#!/usr/bin/env bash
# Local/CI verification entry point.
# Regenerates the notice material and fails if it differs from what is committed,
# or if any compliance check fails.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

CARGO_DENY_VERSION="0.18.9"

fail() {
    echo "LICENSE-E2xx $*" >&2
    exit 1
}

# 1. cargo-deny policy check (licenses, sources, bans, advisories).
command -v cargo >/dev/null 2>&1 || fail "cargo not found on PATH"
deny_version="$(cargo deny --version 2>/dev/null | awk '{print $NF}')"
if [[ "$deny_version" != "$CARGO_DENY_VERSION" ]]; then
    fail "cargo-deny $CARGO_DENY_VERSION required, found '${deny_version:-none}'"
fi

cargo deny --manifest-path "$REPO_ROOT/Cargo.toml" check licenses sources bans advisories

# 2. Regenerate notices and fail if they are stale.
"$REPO_ROOT/scripts/licenses/generate.sh"

if ! git diff --exit-code --quiet -- THIRD-PARTY-NOTICES.md; then
    echo "LICENSE-E201 generated notices are stale: run scripts/licenses/generate.sh" >&2
    git diff -- THIRD-PARTY-NOTICES.md >&2
    exit 1
fi

echo "license-compliance: OK"
