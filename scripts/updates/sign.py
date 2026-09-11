#!/usr/bin/env python3
"""Prepare and sign the updater bundle produced on this release runner."""

import argparse
from pathlib import Path
import subprocess
import tarfile
import tomllib

from generate_manifest import bundles

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target", required=True)
parser.add_argument("--platform", required=True)
args = parser.parse_args()
version = tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"]
name, _ = bundles(version)[args.platform]
directory = Path("target") / args.target / "release"
bundle = directory / name
if args.platform.startswith("macos-"):
    with tarfile.open(bundle, "w:gz") as archive:
        archive.add(directory / "Datalith.app", arcname="Datalith.app")
if not bundle.is_file():
    raise FileNotFoundError(bundle)
subprocess.run(["cargo", "packager", "signer", "sign", str(bundle)], check=True)
