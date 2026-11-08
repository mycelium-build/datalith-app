#!/usr/bin/env bash
# prek hook: formats the workspace and stages the formatting of the Rust files being
# committed (prek passes them as arguments), so the commit goes through formatted.
# prek sets unstaged changes aside while hooks run, so only staged content is formatted.
# If formatting changes a file that is not part of the commit, prek stops the commit.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

cargo fmt --all
git add -- "$@"
