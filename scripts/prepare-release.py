#!/usr/bin/env python3
"""Stamp a release channel, then verify and prepare its built binary for packaging."""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tomllib


def channel_for_tag(tag):
    if re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        return "stable"
    if re.fullmatch(r"v\d+\.\d+\.\d+-rc(?:\.[0-9A-Za-z-]+)*", tag):
        return "preview"
    raise ValueError(f"Unsupported release tag: {tag}")


def prepare(tag, target):
    channel = channel_for_tag(tag)
    version = tag.removeprefix("v")
    config = tomllib.loads(Path(f"packager.{channel}.toml").read_text())
    directory = Path("target") / target / "release"
    binary = directory / ("datalith.exe" if os.name == "nt" else "datalith")
    identity = json.loads(subprocess.check_output([str(binary), "--print-build-identity"], text=True))
    expected = {
        "channel": channel,
        "product_name": config["product-name"],
        "identifier": config["identifier"],
        "stem": config["binaries"][0]["path"],
        "data_directory": config["name"],
        "update_endpoint": f"https://mycelium-build.github.io/datalith-app/updates/{channel}.json",
        "version": version,
    }
    if identity != expected:
        raise ValueError(f"Build identity mismatch: {identity!r}; expected {expected!r}")
    packaged_binary = directory / (identity["stem"] + binary.suffix)
    if packaged_binary != binary:
        shutil.copy2(binary, packaged_binary)
    config["version"] = version
    config["out-dir"] = str(directory.resolve())
    config["icons"] = [str(Path(icon).resolve()) for icon in config["icons"]]
    config["license-file"] = str(Path(config["license-file"]).resolve())
    config["nsis"]["installer-icon"] = str(Path(config["nsis"]["installer-icon"]).resolve())
    for resource in config["resources"]:
        resource["src"] = str(Path(resource["src"]).resolve())
    output = directory / "packager.json"
    output.write_text(json.dumps(config, indent=2) + "\n")
    print(f"Verified {identity['product_name']} {version}; wrote {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--target", help="Verify built identity and prepare packager.json; omit to stamp CHANNEL")
    args = parser.parse_args()
    if args.target:
        prepare(args.tag, args.target)
    else:
        Path("CHANNEL").write_text(channel_for_tag(args.tag) + "\n")


if __name__ == "__main__":
    main()
