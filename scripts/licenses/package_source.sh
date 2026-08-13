#!/usr/bin/env bash
# Assembles the Corresponding Source archive for a release.
#
# The archive contains the exact source tree for the release commit, the
# vendored dependency sources, and the offline Cargo configuration needed to
# rebuild each supported target. It is produced with `cargo vendor --locked`.
#
# Usage: package_source.sh [--tag <tag>] [--output <dir>]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

TAG="${TAG:-}"
OUTPUT_DIR="${OUTPUT_DIR:-.}"

fail() {
    echo "LICENSE-E4xx $*" >&2
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag) TAG="$2"; shift 2 ;;
        --output) OUTPUT_DIR="$2"; shift 2 ;;
        *) fail "unknown argument: $1" ;;
    esac
done

# Resolve the version from Cargo.toml.
PACKAGE_VERSION="$(python3 - <<'PY'
import tomllib
with open('Cargo.toml', 'rb') as f:
    print(tomllib.load(f)['package']['version'])
PY
)"

if [[ -z "$TAG" ]]; then
    TAG="v${PACKAGE_VERSION}"
fi

# 1. Require the checkout to be at the tag and clean.
if ! git rev-parse --verify "refs/tags/${TAG}" >/dev/null 2>&1; then
    fail "release tag not found: tag=${TAG}"
fi

TAG_SHA="$(git rev-list -n 1 "refs/tags/${TAG}")"
HEAD_SHA="$(git rev-parse HEAD)"
if [[ "$TAG_SHA" != "$HEAD_SHA" ]]; then
    fail "checkout HEAD is not the release tag: tag=${TAG} head=${HEAD_SHA} tag_sha=${TAG_SHA}"
fi

if [[ -n "$(git status --porcelain)" ]]; then
    fail "working tree is not clean; commit or stash changes before packaging source"
fi

# 2. Verify the tag is either the Cargo version or one of its release
# candidates. RC tags point at the release PR, whose Cargo version is stable.
RELEASE_VERSION="${TAG#v}"
ESCAPED_PACKAGE_VERSION="${PACKAGE_VERSION//./\\.}"
if [[ "$RELEASE_VERSION" != "$PACKAGE_VERSION" && \
      ! "$RELEASE_VERSION" =~ ^${ESCAPED_PACKAGE_VERSION}-rc\.[1-9][0-9]*$ ]]; then
    fail "release/source version mismatch: tag=${TAG} cargo=${PACKAGE_VERSION}"
fi

STAGING="$(mktemp -d)"
trap 'rm -rf "$STAGING"' EXIT

ARCHIVE_NAME="datalith-${RELEASE_VERSION}-corresponding-source.tar.zst"

# 3. Copy tracked source using git archive (excludes .git, target, etc.).
git archive --format=tar --prefix="datalith-${RELEASE_VERSION}/" "$TAG" \
    -o "$STAGING/source.tar"
mkdir -p "$STAGING/src"
tar xf "$STAGING/source.tar" -C "$STAGING/src"
SRC_DIR="$STAGING/src/datalith-${RELEASE_VERSION}"

# 4. Vendor all dependencies (crates.io and Git) with the locked versions.
mkdir -p "$SRC_DIR/.cargo"
(
    cd "$SRC_DIR"
    cargo vendor --locked vendor >.cargo/config.toml

    # Prove that the generated configuration redirects both registry and Git
    # dependencies. An empty Cargo home prevents a warm Git checkout or
    # registry cache from hiding an incomplete source replacement.
    OFFLINE_CARGO_HOME="$STAGING/offline-cargo-home"
    mkdir -p "$OFFLINE_CARGO_HOME"
    CARGO_HOME="$OFFLINE_CARGO_HOME" \
        cargo metadata --offline --locked --format-version=1 >/dev/null
)

# 5. Record a source manifest.
{
    echo "package = datalith"
    echo "package_version = ${PACKAGE_VERSION}"
    echo "release_version = ${RELEASE_VERSION}"
    echo "tag = ${TAG}"
    echo "commit = ${TAG_SHA}"
    echo "toolchain = $(cat rust-toolchain.toml)"
} >"$SRC_DIR/SOURCE-MANIFEST"

# 6. Create the deterministic archive.
mkdir -p "$OUTPUT_DIR"
tar --sort=name --mtime='UTC 2000-01-01' --owner=0 --group=0 --numeric-owner \
    --zstd -cf "$OUTPUT_DIR/$ARCHIVE_NAME" -C "$STAGING/src" "datalith-${RELEASE_VERSION}"

echo "wrote $OUTPUT_DIR/$ARCHIVE_NAME"
