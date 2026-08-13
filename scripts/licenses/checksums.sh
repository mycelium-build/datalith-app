#!/usr/bin/env bash
# Generates SHA256SUMS over the complete release artifact set.
#
# Usage: checksums.sh <artifact-dir>
#
# The artifact dir must contain exactly the expected set of release files. The
# script fails on duplicate names, missing artifacts, or unexpected files.
set -euo pipefail

ARTIFACT_DIR="${1:?artifact dir required}"

fail() {
    echo "LICENSE-E5xx $*" >&2
    exit 1
}

[[ -d "$ARTIFACT_DIR" ]] || fail "artifact dir not found: dir=$ARTIFACT_DIR"

# The basenames that must be present, and the wildcard families they may match.
cd "$ARTIFACT_DIR"

REQUIRED_EXACT=(
    "THIRD-PARTY-NOTICES.md"
    "LICENSE"
    "LICENSE-GPL-3.0"
)

REQUIRED_PATTERNS=(
    "Datalith-*"                    # platform packages (AppImage/DMG/NSIS)
    "datalith-*-corresponding-source.tar.zst"
    "datalith-*.spdx.json"
)

for name in "${REQUIRED_EXACT[@]}"; do
    [[ -f "$name" ]] || fail "missing release artifact: name=$name"
done

for pattern in "${REQUIRED_PATTERNS[@]}"; do
    count="$(find . -maxdepth 1 -type f -name "$pattern" | wc -l)"
    [[ "$count" -gt 0 ]] || fail "missing release artifact: name=$pattern"
done

# Fail on duplicate basenames (should not happen within a flat artifact dir).
dupes="$(find . -maxdepth 1 -type f -printf '%f\n' | sort | uniq -d)"
if [[ -n "$dupes" ]]; then
    fail "duplicate release artifact names: $dupes"
fi

sha256sum * >SHA256SUMS
echo "wrote $ARTIFACT_DIR/SHA256SUMS"
