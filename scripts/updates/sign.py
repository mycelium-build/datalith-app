#!/usr/bin/env python3
"""Prepare and sign the updater bundle produced on this release runner."""

import argparse
from pathlib import Path
import subprocess
import tarfile
import os

from generate_manifest import bundles

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target", required=True)
parser.add_argument("--platform", required=True)
args = parser.parse_args()
version = os.environ["DATALITH_RELEASE_TAG"].removeprefix("v")
name, _ = bundles(version)[args.platform]
directory = Path("target") / args.target / "release"
bundle = directory / name
if args.platform.startswith("macos-"):
    product = "Datalith Preview" if Path("CHANNEL").read_text().strip() == "preview" else "Datalith"
    with tarfile.open(bundle, "w:gz") as archive:
        archive.add(directory / f"{product}.app", arcname=f"{product}.app")
if not bundle.is_file():
    raise FileNotFoundError(bundle)
subprocess.run(["cargo", "packager", "signer", "sign", str(bundle)], check=True)
