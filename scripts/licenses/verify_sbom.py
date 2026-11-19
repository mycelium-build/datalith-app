#!/usr/bin/env python3
"""Validate a Syft-generated SPDX JSON SBOM for Datalith.

Usage: verify_sbom.py <sbom.json> <version> <Cargo.lock>

Checks that the SBOM is SPDX JSON, identifies Datalith, and lists every GPUI crate
(gpui-kit, gpui-component, gpui-pre*) that the released Cargo.lock resolves. Reading the
list from the lockfile keeps the check in step with gpui-kit upgrades, which add and drop
gpui-pre crates. Bundled assets are covered by assets/licenses.toml.

Exit codes follow the LICENSE-E* prefixes.
"""

from __future__ import annotations

import json
import sys
import tomllib

GPUI_PREFIXES = ("gpui-kit", "gpui-component", "gpui-pre")
ALWAYS_REQUIRED = {"gpui-kit", "gpui-component", "gpui-pre"}


def fail(message: str) -> None:
    print(f"LICENSE-E601 invalid SPDX SBOM: reason={message}", file=sys.stderr)
    raise SystemExit(1)


def gpui_crates(lockfile: str) -> set[str]:
    try:
        with open(lockfile, "rb") as handle:
            lock = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        fail(f"cannot read Cargo.lock: {error}")

    names = {
        package.get("name")
        for package in lock.get("package", [])
        if isinstance(package, dict) and isinstance(package.get("name"), str)
    }
    return {name for name in names if name.startswith(GPUI_PREFIXES)}


def main() -> int:
    if len(sys.argv) != 4:
        fail("usage: verify_sbom.py <sbom.json> <version> <Cargo.lock>")

    path = sys.argv[1]
    version = sys.argv[2]
    lockfile = sys.argv[3]

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
    if name != "datalith":
        fail(f"SBOM does not identify Datalith: name={name}")

    packages = data.get("packages", [])
    if not isinstance(packages, list) or not packages:
        fail("SBOM has no packages")

    package_names = {
        package.get("name")
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("name"), str)
    }
    required = ALWAYS_REQUIRED | gpui_crates(lockfile)
    missing = sorted(required - package_names)
    if missing:
        fail(f"SBOM missing expected components: components={','.join(missing)}")

    print(f"SBOM OK: path={path} version={version} packages={len(packages)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
