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

# The basenames and single-file families that make up one release.
cd "$ARTIFACT_DIR"

REQUIRED_EXACT=(
    "THIRD-PARTY-NOTICES.md"
    "LICENSE"
    "LICENSE-GPL-3.0"
    "LICENSING.md"
)

REQUIRED_PATTERNS=(
    "*.AppImage"
    "*.deb"
    "*.rpm"
    "*.pkg.tar.zst"
    "*.dmg"
    "*setup.exe"
    "datalith-*-corresponding-source.tar.zst"
    "datalith-*.spdx.json"
)

for name in "${REQUIRED_EXACT[@]}"; do
    [[ -f "$name" ]] || fail "missing release artifact: name=$name"
done

for pattern in "${REQUIRED_PATTERNS[@]}"; do
    count="$(find . -maxdepth 1 -type f -name "$pattern" | wc -l)"
    [[ "$count" -eq 1 ]] || fail "expected one release artifact: name=$pattern count=$count"
done

# Reject anything outside the contract before checksumming it.
# A retry may download a checksum file from an earlier draft run. It is output,
# not input, and is replaced after the artifact set is accepted.
mapfile -t files < <(
    find . -maxdepth 1 -type f ! -name SHA256SUMS -printf '%f\n' | sort
)
for file in "${files[@]}"; do
    allowed=false
    for name in "${REQUIRED_EXACT[@]}"; do
        [[ "$file" == "$name" ]] && allowed=true
    done
    for pattern in "${REQUIRED_PATTERNS[@]}"; do
        [[ "$file" == $pattern ]] && allowed=true
    done
    [[ "$allowed" == true ]] || fail "unexpected release artifact: name=$file"
done

sha256sum -- "${files[@]}" >SHA256SUMS
echo "wrote $ARTIFACT_DIR/SHA256SUMS"
