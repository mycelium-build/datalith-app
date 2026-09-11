#!/usr/bin/env python3
"""Validate signed release bundles and generate the stable updater manifest."""

import argparse
import base64
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import tempfile

PUBLIC_KEY = Path(__file__).with_name("public-key.txt")
STABLE_TAG = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")


def bundles(version):
    stem = "datalith-preview" if "-rc" in version else "datalith"
    return {
        f"{system}-{arch}": (name, format_name)
        for arch in ("x86_64", "aarch64")
        for system, name, format_name in (
            ("linux", f"{stem}_{version}_{arch}.AppImage", "appimage"),
            ("macos", f"{stem}_{version}_{arch}.app.tar.gz", "app"),
            ("windows", f"{stem}_{version}_{'x64' if arch == 'x86_64' else 'arm64'}-setup.exe", "nsis"),
        )
    }


def gh_json(*args):
    return json.loads(subprocess.check_output(["gh", *args], text=True))


def latest_stable(releases):
    candidates = []
    for release in releases:
        match = STABLE_TAG.fullmatch(release["tag_name"])
        if (match and tuple(map(int, match.groups())) > (0, 1, 0)
                and not release["draft"] and not release["prerelease"]):
            candidates.append((tuple(map(int, match.groups())), release))
    return max(candidates, key=lambda item: item[0])[1] if candidates else None


def verify_signature(bundle, encoded_signature, public_key=PUBLIC_KEY):
    with tempfile.TemporaryDirectory() as temporary:
        key = Path(temporary) / "key.pub"
        signature = Path(temporary) / "bundle.sig"
        key.write_bytes(base64.b64decode(public_key.read_text().strip(), validate=True))
        decoded_signature = base64.b64decode(encoded_signature, validate=True)
        if not decoded_signature.decode().splitlines()[2].endswith(f"\tfile:{bundle.name}"):
            raise ValueError(f"Signature filename mismatch: {bundle.name}")
        signature.write_bytes(decoded_signature)
        subprocess.run(
            ["minisign", "-V", "-q", "-p", str(key), "-m", str(bundle), "-x", str(signature)],
            check=True,
        )


def generate_manifest(release, directory, public_key=PUBLIC_KEY):
    version = release["tag_name"].removeprefix("v")
    expected = bundles(version)
    assets = release["assets"]
    counts = Counter(asset["name"] for asset in assets)
    required = {name + suffix for name, _ in expected.values() for suffix in ("", ".sig")}
    for name in required:
        if counts[name] != 1:
            raise ValueError(f"Expected exactly one {name}, found {counts[name]}")
    for name in counts:
        if name.endswith((".AppImage", ".app.tar.gz", "-setup.exe", ".sig")) and name not in required:
            raise ValueError(f"Unexpected updater asset: {name}")
    by_name = {asset["name"]: asset for asset in assets}
    platforms = {}
    for platform, (name, format_name) in expected.items():
        bundle = directory / name
        if bundle.stat().st_size != by_name[name]["size"] or not bundle.stat().st_size:
            raise ValueError(f"Asset size mismatch: {name}")
        signature = (directory / f"{name}.sig").read_text().strip()
        verify_signature(bundle, signature, public_key)
        platforms[platform] = {
            "url": by_name[name]["browser_download_url"],
            "signature": signature,
            "format": format_name,
        }
    return {"version": version, "pub_date": release["published_at"], "platforms": platforms}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", help="Validate this draft or published release instead of selecting latest stable")
    parser.add_argument("--assets", type=Path, help="Use already downloaded assets")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.tag:
        release = gh_json("api", f"repos/{args.repository}/releases/tags/{args.tag}")
    else:
        pages = gh_json("api", "--paginate", "--slurp", f"repos/{args.repository}/releases?per_page=100")
        release = latest_stable([release for page in pages for release in page])
    if release is None:
        manifest = {"version": "0.0.0", "platforms": {}}
    else:
        with tempfile.TemporaryDirectory() as temporary:
            directory = args.assets or Path(temporary)
            if args.assets is None:
                subprocess.run([
                    "gh", "release", "download", release["tag_name"], "--repo", args.repository,
                    "--dir", str(directory), "--pattern", "*.AppImage", "--pattern", "*.app.tar.gz",
                    "--pattern", "*-setup.exe", "--pattern", "*.sig",
                ], check=True)
            manifest = generate_manifest(release, directory)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
