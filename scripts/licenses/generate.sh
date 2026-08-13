#!/usr/bin/env bash
# Deterministic regeneration entry point for license notice material.
# Produces THIRD-PARTY-NOTICES.md.
# Do not edit the generated files by hand; run this script instead.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

SCRIPTS_DIR="$REPO_ROOT/scripts/licenses"

# Pinned tool versions. Bump in a dedicated dependency-update change that also
# regenerates the notices.
CARGO_ABOUT_VERSION="0.9.1"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
    echo "LICENSE-E201 $*" >&2
    exit 1
}

command -v cargo >/dev/null 2>&1 || fail "cargo not found on PATH"

# 1. Verify the pinned tool version.
about_version="$(cargo about --version 2>/dev/null | awk '{print $NF}')"
if [[ "$about_version" != "$CARGO_ABOUT_VERSION" ]]; then
    fail "cargo-about $CARGO_ABOUT_VERSION required, found '${about_version:-none}'"
fi

# 2. Validate the asset manifest.
python3 "$SCRIPTS_DIR/check_assets.py"

# 3. Render the non-Cargo asset sections.
python3 "$SCRIPTS_DIR/check_assets.py" --emit "$TMP_DIR/assets.md"

# 4. Render the Rust dependency sections.
cargo about generate \
    --config "$SCRIPTS_DIR/about.toml" \
    --format handlebars \
    --fail \
    --locked \
    --output-file "$TMP_DIR/rust.md" \
    "$SCRIPTS_DIR/about.hbs"

# 5. Combine into the final documents.
{
    cat "$SCRIPTS_DIR/notices-intro.md"
    printf '\n'
    cat "$TMP_DIR/assets.md"
    printf '\n'
    cat "$TMP_DIR/rust.md"
} > "$TMP_DIR/notices.md"

# 6. Write the committed artifact byte-for-byte. The application embeds this
# same file directly, so there is no second generated copy to keep in sync.
cp "$TMP_DIR/notices.md" "$REPO_ROOT/THIRD-PARTY-NOTICES.md"

echo "wrote THIRD-PARTY-NOTICES.md"
