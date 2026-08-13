#!/usr/bin/env python3
"""Validate a Syft-generated SPDX JSON SBOM for Datalith.

Usage: verify_sbom.py <sbom.json> <version>

Checks that the SBOM is SPDX JSON, identifies the Datalith release, and lists the core Rust components.
Bundled assets are covered by assets/licences.toml.

Exit codes follow the LICENSE-E* prefixes.
"""

from __future__ import annotations

import json
import sys


def fail(message: str) -> None:
    print(f"LICENSE-E601 invalid SPDX SBOM: reason={message}", file=sys.stderr)
    raise SystemExit(1)


def main() -> int:
    if len(sys.argv) != 3:
        fail("usage: verify_sbom.py <sbom.json> <version>")

    path = sys.argv[1]
    version = sys.argv[2]

    try:
        with open(path, encoding="utf-8") as handle:
            data = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        fail(f"cannot read valid JSON: {error}")

    if not isinstance(data, dict):
        fail("top-level SPDX document is not an object")

    spdx_version = data.get("spdxVersion")
    if not isinstance(spdx_version, str) or not spdx_version.startswith("SPDX-"):
        fail(f"missing SPDX version: spdxVersion={spdx_version}")

    name = str(data.get("name", ""))
    if version not in name:
        fail(f"SBOM does not identify Datalith version {version}: name={name}")

    packages = data.get("packages", [])
    if not isinstance(packages, list) or not packages:
        fail("SBOM has no packages")

    package_names = {
        package.get("name")
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("name"), str)
    }
    required = {"gpui", "gpui-component", "zlog", "ztracing", "ztracing_macro"}
    missing = sorted(required - package_names)
    if missing:
        fail(f"SBOM missing expected components: components={','.join(missing)}")

    print(f"SBOM OK: path={path} version={version} packages={len(packages)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
