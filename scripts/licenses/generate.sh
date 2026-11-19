#!/usr/bin/env bash
# Deterministic regeneration entry point for license notice material.
# Produces THIRD-PARTY-NOTICES.md.
# Do not edit the generated files by hand; run this script instead.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

SCRIPTS_DIR="$REPO_ROOT/scripts/licenses"

# Installs the pinned tool versions (scripts/tool-versions.env) when missing.
# shellcheck source=../lib/tools.sh
source "$REPO_ROOT/scripts/lib/tools.sh"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

fail() {
    echo "LICENSE-E201 $*" >&2
    exit 1
}

command -v cargo >/dev/null 2>&1 || fail "cargo not found on PATH"

# 1. Use the pinned tool version. cargo-about only builds its binary with the cli feature.
ensure_cargo_tool cargo-about "$CARGO_ABOUT_VERSION" --features cli \
    || fail "cannot install cargo-about $CARGO_ABOUT_VERSION"

# 2. Validate the asset manifest.
python3 "$SCRIPTS_DIR/check_assets.py"

# 3. Sync the cargo-about accepted-license list from deny.toml (single source).
python3 "$SCRIPTS_DIR/sync_allowlist.py"

# 4. Render the non-Cargo asset sections.
python3 "$SCRIPTS_DIR/check_assets.py" --emit "$TMP_DIR/assets.md"

# 5. Render the Rust dependency sections.
cargo about generate --manifest-path "$REPO_ROOT/apps/desktop/Cargo.toml" \
    --config "$SCRIPTS_DIR/about.toml" \
    --format handlebars \
    --fail \
    --locked \
    --output-file "$TMP_DIR/rust.md" \
    "$SCRIPTS_DIR/about.hbs"

# Compare by exact license text as well as package identity. Platform builds
# can embed different copyright notices under the same SPDX identifier.
apps/mobile/android/prepare-backend.sh
license_graph() {
    local name="$1" manifest="$2"
    shift 2
    cargo about generate --locked --fail --manifest-path "$manifest" \
        --config "$SCRIPTS_DIR/about.toml" --format json "$@" \
        --output-file "$TMP_DIR/$name.json"
}
license_graph desktop apps/desktop/Cargo.toml
license_graph ios apps/mobile/Cargo.toml --target aarch64-apple-ios
license_graph android apps/mobile/android/Cargo.toml --target aarch64-linux-android
python3 "$SCRIPTS_DIR/mobile_notices.py" "$TMP_DIR" > "$TMP_DIR/mobile.md"

# 6. Combine into the final documents.
{
    cat "$SCRIPTS_DIR/notices-intro.md"
    printf '\n'
    cat "$TMP_DIR/assets.md"
    printf '\n'
    cat "$TMP_DIR/rust.md"
    cat "$TMP_DIR/mobile.md"
} > "$TMP_DIR/notices.md"

# Some upstream license texts contain CRLF line endings. Normalize the final
# assembled artifact so every generator path produces the same bytes.
sed 's/\r$//' "$TMP_DIR/notices.md" > "$TMP_DIR/notices.clean.md"
mv "$TMP_DIR/notices.clean.md" "$TMP_DIR/notices.md"

# 7. Write the committed artifact byte-for-byte.
# The application embeds this same file directly, so there is no second generated copy to keep in sync.
cp "$TMP_DIR/notices.md" "$REPO_ROOT/THIRD-PARTY-NOTICES.md"

echo "wrote THIRD-PARTY-NOTICES.md"
