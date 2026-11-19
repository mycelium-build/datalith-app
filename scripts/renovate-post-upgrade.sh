#!/usr/bin/env bash
# Renovate runs this after each Cargo update (postUpgradeTasks in renovate.json), in the
# Renovate workflow, to keep THIRD-PARTY-NOTICES.md in the same PR as the update.
# generate.sh installs the pinned cargo-about first if a tools update changed it.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# The NativeActivity bridge has a separate lockfile for its patched GPUI backend.
apps/mobile/android/prepare-backend.sh
cargo metadata --manifest-path apps/mobile/android/Cargo.toml --format-version=1 > /dev/null
scripts/licenses/generate.sh
