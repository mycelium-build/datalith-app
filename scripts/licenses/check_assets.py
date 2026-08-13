#!/usr/bin/env python3
"""Validate assets/licences.toml and emit deterministic asset-license Markdown.

The manifest is the single source of truth for non-Cargo assets (fonts, icons,
themes, artwork). This script:

  1. loads the manifest with the stdlib TOML parser;
  2. enumerates tracked asset files (git ls-files, falling back to os.walk);
  3. expands every entry's `paths` globs;
  4. fails if a tracked asset file matches no entry (uncovered);
  5. fails if an entry path matches no file;
  6. fails on overlapping entries;
  7. verifies every referenced license file exists and is non-empty;
  8. validates SPDX expressions against an accepted allowlist;
  9. requires author/source/revision/copyright for third-party assets;
 10. rejects third-party themes without a verifiable license;
 11. optionally emits deterministic Markdown (--emit) in stable id order.

Exit codes follow the LICENSE-E* prefixes documented in
docs/license-compliance-plan.md.
"""

from __future__ import annotations

import argparse
import glob
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]

ACCEPTED_LICENSES = {
    "0BSD",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "CC0-1.0",
    "GPL-3.0-or-later",
    "ISC",
    "MIT",
    "MIT-0",
    "MPL-2.0",
    "NCSA",
    "OFL-1.1",
    "Unicode-3.0",
    "Zlib",
    "bzip2-1.0.6",
}

ACCEPTED_EXPRESSIONS = ACCEPTED_LICENSES | {
    "Apache-2.0 WITH LLVM-exception",
    "LicenseRef-TextMateThemesBundle",
}

REQUIRED_FIELDS = {"id", "kind", "name", "license"}

THIRD_PARTY_REQUIRED = {
    "author",
    "copyright",
    "source",
    "revision",
    "license",
    "license_file",
}

EMBEDDED_ASSET_ROOTS = ("assets/icons/", "assets/fonts/", "src/ui/themes/")


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def tracked_files() -> list[str]:
    try:
        output = subprocess.run(
            ["git", "ls-files"],
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    except (subprocess.CalledProcessError, FileNotFoundError):
        output = ""
    if output:
        tracked: list[str] = []
        for line in output.splitlines():
            line = line.strip()
            if line and (REPO_ROOT / line).is_file():
                tracked.append(line)
        return tracked

    result: list[str] = []
    for root in EMBEDDED_ASSET_ROOTS:
        base = REPO_ROOT / root
        if not base.exists():
            continue
        for path in base.rglob("*"):
            if path.is_file():
                result.append(str(path.relative_to(REPO_ROOT)))
    return result


def is_asset_file(path: str) -> bool:
    if path.startswith(("assets/icons/", "assets/fonts/")):
        return True
    if path.startswith("src/ui/themes/"):
        return path.endswith(".json")
    return False


def expand_paths(patterns: list[str]) -> set[str]:
    expanded: set[str] = set()
    for pattern in patterns:
        matches = glob.glob(pattern, root_dir=REPO_ROOT, recursive=True)
        if not matches:
            fail(f"LICENSE-E104 manifest path matches no file: pattern={pattern}")
        for match in matches:
            full = (REPO_ROOT / match)
            if full.is_file():
                expanded.add(match)
    return expanded


def validate_spdx(asset_id: str, expression: str) -> None:
    if expression not in ACCEPTED_EXPRESSIONS:
        fail(
            f"LICENSE-E001 unapproved asset license: id={asset_id} "
            f"expression={expression}"
        )


def load_manifest(path: Path) -> list[dict[str, Any]]:
    with path.open("rb") as handle:
        data = tomllib.load(handle)
    assets = data.get("asset", [])
    if not assets:
        fail(f"LICENSE-E106 manifest has no [[asset]] entries: path={path}")
    return assets


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--emit",
        metavar="PATH",
        help="write deterministic asset-license Markdown to PATH",
    )
    args = parser.parse_args()

    manifest_path = REPO_ROOT / "assets" / "licences.toml"
    assets = load_manifest(manifest_path)

    seen_ids: set[str] = set()
    covered_files: dict[str, str] = {}
    for asset in assets:
        raw_id = asset.get("id")
        if not isinstance(raw_id, str) or not raw_id:
            fail("LICENSE-E107 asset entry missing string id")
        asset_id = str(raw_id)

        if asset_id in seen_ids:
            fail(f"LICENSE-E108 duplicate asset id: id={asset_id}")
        seen_ids.add(asset_id)

        missing = REQUIRED_FIELDS - asset.keys()
        if missing:
            fail(
                f"LICENSE-E109 asset missing required field: id={asset_id} "
                f"missing={sorted(missing)}"
            )

        license_expr = asset["license"]
        if not isinstance(license_expr, str) or not license_expr:
            fail(f"LICENSE-E105 asset license must be a string: id={asset_id}")
        validate_spdx(asset_id, license_expr)

        is_first_party = asset.get("source") == "first-party"
        if not is_first_party:
            missing = THIRD_PARTY_REQUIRED - asset.keys()
            if missing:
                fail(
                    f"LICENSE-E110 third-party asset missing field: id={asset_id} "
                    f"missing={sorted(missing)}"
                )
            if asset.get("kind") == "theme" and not asset.get("license_evidence"):
                fail(
                    f"LICENSE-E102 unverified theme license: id={asset_id} "
                    f"source={asset.get('source')}"
                )
            if asset.get("kind") == "theme":
                evidence = str(asset["license_evidence"])
                if re.search(r"/(?:main|master|develop)/", evidence, re.IGNORECASE):
                    fail(
                        f"LICENSE-E112 mutable theme license evidence: id={asset_id} "
                        f"evidence={evidence}"
                    )

        license_file = asset.get("license_file")
        if license_file:
            path = REPO_ROOT / license_file
            if not path.is_file():
                fail(
                    f"LICENSE-E103 missing asset license file: id={asset_id} "
                    f"path={license_file}"
                )
            if path.stat().st_size == 0:
                fail(f"LICENSE-E103 empty asset license file: id={asset_id} path={license_file}")

        paths = asset.get("paths", [])
        if paths:
            files = expand_paths(paths)
            for file in files:
                if file in covered_files:
                    fail(
                        f"LICENSE-E111 overlapping asset entry: file={file} "
                        f"id={asset_id} already={covered_files[file]}"
                    )
                covered_files[file] = asset_id

    # Coverage: every tracked asset file must be covered exactly once.
    for file in tracked_files():
        if not is_asset_file(file):
            continue
        if file not in covered_files:
            fail(f"LICENSE-E101 uncovered embedded asset: path={file}")

    if args.emit:
        write_markdown(assets, Path(args.emit))

    return 0


def write_markdown(assets: list[dict[str, Any]], destination: Path) -> None:
    ordered = sorted(assets, key=lambda asset: asset["id"])
    lines: list[str] = [
        "## Bundled assets",
        "",
        "This section lists non-Cargo assets embedded in or distributed with",
        "Datalith. It is generated from `assets/licences.toml`; do not edit it",
        "by hand.",
        "",
    ]
    for asset in ordered:
        asset_id = asset["id"]
        name = asset["name"]
        license_expr = asset["license"]
        source = asset.get("source", "")
        author = asset.get("author", "")
        copyright_line = asset.get("copyright", "")
        license_file = asset.get("license_file")

        lines.append(f"### {name}")
        lines.append("")
        lines.append(f"- Identifier: `{asset_id}`")
        lines.append(f"- Kind: {asset['kind']}")
        if author:
            lines.append(f"- Author: {author}")
        if copyright_line:
            lines.append(f"- Copyright: {copyright_line}")
        lines.append(f"- License: {license_expr}")
        if source and source != "first-party":
            lines.append(f"- Source: {source}")
            revision = asset.get("revision")
            if revision:
                lines.append(f"- Revision: {revision}")
        else:
            lines.append("- Source: first-party (Datalith)")
        if license_file:
            lines.append(f"- License text: {license_file}")
        license_evidence = asset.get("license_evidence")
        if license_evidence:
            lines.append(f"- License evidence: {license_evidence}")
        notes = asset.get("notes")
        if notes:
            lines.append(f"- Notes: {notes}")
        lines.append("")

    # Inline the full text of every retained license once, grouped by file, so
    # the notices are self-contained (cargo-about covers Rust dependencies; this
    # covers bundled assets, including non-standard grants).
    texts: dict[str, list[str]] = {}
    for asset in ordered:
        license_file = asset.get("license_file")
        if license_file:
            texts.setdefault(license_file, []).append(asset["name"])

    if texts:
        lines.append("## Bundled asset license texts")
        lines.append("")
        lines.append(
            "The following license texts are reproduced in full for the bundled "
            "assets listed above."
        )
        lines.append("")
        for license_file, names in texts.items():
            path = REPO_ROOT / license_file
            content = path.read_text(encoding="utf-8").rstrip("\n")
            lines.append(f"### {license_file}")
            lines.append("")
            lines.append("Used by: " + ", ".join(names))
            lines.append("")
            lines.append("```")
            lines.append(content)
            lines.append("```")
            lines.append("")

    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
