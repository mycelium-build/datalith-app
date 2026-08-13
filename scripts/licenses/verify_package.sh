#!/usr/bin/env bash
# Verifies that a built package contains the required legal documents.
#
# Usage: verify_package.sh <format> <package-path>
#   format: appimage | dmg | nsis | deb | rpm | archlinux
set -euo pipefail

FORMAT="${1:?format required}"
PACKAGE="${2:?package path required}"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

REQUIRED_FILES=(
    "LICENSE"
    "LICENSE-GPL-3.0"
    "LICENSING.md"
    "THIRD-PARTY-NOTICES.md"
)

fail() {
    echo "LICENSE-E301 $*" >&2
    exit 1
}

[[ -f "$PACKAGE" ]] || fail "package not found: path=$PACKAGE"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

extract() {
    case "$FORMAT" in
        appimage)
            # The AppImage is a self-extracting archive.
            (cd "$TMP" && "$PACKAGE" --appimage-extract >/dev/null 2>&1)
            ;;
        deb)
            (cd "$TMP" && dpkg-deb -x "$PACKAGE" root)
            ;;
        rpm)
            (cd "$TMP" && mkdir root && rpm2cpio "$PACKAGE" | cpio -idm -D root >/dev/null 2>&1)
            ;;
        archlinux)
            (cd "$TMP" && mkdir root && tar --zstd -xf "$PACKAGE" -C root)
            ;;
        nsis)
            (cd "$TMP" && 7z x -y "$PACKAGE" >/dev/null)
            ;;
        dmg)
            # Only meaningful on macOS; mounts and inspects the .app resources.
            if [[ "$(uname -s)" != "Darwin" ]]; then
                fail "DMG inspection requires macOS: format=dmg"
            fi
            hdiutil attach "$PACKAGE" -mountpoint "$TMP/mnt" -nobrowse -quiet
            trap 'hdiutil detach "$TMP/mnt" -quiet; rm -rf "$TMP"' EXIT
            ;;
        *)
            fail "unsupported format: format=$FORMAT"
            ;;
    esac
}

extract

for file in "${REQUIRED_FILES[@]}"; do
    found="$(find "$TMP" -type f -name "$file" -print -quit)"
    if [[ -z "$found" ]]; then
        fail "package legal file missing: format=$FORMAT file=$file"
    fi
    if [[ ! -s "$found" ]]; then
        fail "package legal file empty: format=$FORMAT file=$file"
    fi
    if ! cmp -s "$REPO_ROOT/$file" "$found"; then
        fail "package legal file differs from release source: format=$FORMAT file=$file"
    fi
done

echo "package legal documents OK: format=$FORMAT"
