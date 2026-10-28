#!/usr/bin/env bash
# Local/CI verification entry point.
# Regenerates the notice material and fails if generation changes the current files,
# or if any compliance check fails.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

CARGO_DENY_VERSION="0.20.2"

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

# 2. Snapshot and regenerate notices, then fail if generation changed them.
check_tmp_dir="$(mktemp -d)"
trap 'rm -rf "$check_tmp_dir"' EXIT
cp "$REPO_ROOT/THIRD-PARTY-NOTICES.md" "$check_tmp_dir/THIRD-PARTY-NOTICES.md"
cp "$REPO_ROOT/scripts/licenses/about.toml" "$check_tmp_dir/about.toml"

"$REPO_ROOT/scripts/licenses/generate.sh"

generated_material_changed=false
cmp --silent "$check_tmp_dir/THIRD-PARTY-NOTICES.md" "$REPO_ROOT/THIRD-PARTY-NOTICES.md" \
    || generated_material_changed=true
cmp --silent "$check_tmp_dir/about.toml" "$REPO_ROOT/scripts/licenses/about.toml" \
    || generated_material_changed=true

if [[ "$generated_material_changed" == true ]]; then
    echo "LICENSE-E201 generated license material is stale: run scripts/licenses/generate.sh" >&2
    diff --unified \
        --label THIRD-PARTY-NOTICES.md.before \
        --label THIRD-PARTY-NOTICES.md.after \
        "$check_tmp_dir/THIRD-PARTY-NOTICES.md" \
        "$REPO_ROOT/THIRD-PARTY-NOTICES.md" >&2 || true
    diff --unified \
        --label scripts/licenses/about.toml.before \
        --label scripts/licenses/about.toml.after \
        "$check_tmp_dir/about.toml" \
        "$REPO_ROOT/scripts/licenses/about.toml" >&2 || true
    exit 1
fi

echo "license-compliance: OK"
