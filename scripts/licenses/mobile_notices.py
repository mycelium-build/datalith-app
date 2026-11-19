"""Append license records shipped on mobile but absent from the desktop notices."""
import json
import sys
from collections import defaultdict
from pathlib import Path


def graph(path):
    records = defaultdict(set)
    for license in json.loads(path.read_text())["licenses"]:
        key = (license["name"], license["id"], license["text"].replace("\r\n", "\n"))
        for use in license["used_by"]:
            crate = use["crate"]
            records[key].add((crate["name"], crate["version"], crate["license"] or "", crate["repository"] or ""))
    return records


def render(directory):
    desktop = graph(directory / "desktop.json")
    mobile = graph(directory / "ios.json")
    for key, packages in graph(directory / "android.json").items():
        mobile[key].update(packages)

    print("\n# Additional Rust notices for iOS and Android\n")
    for key, packages in sorted(mobile.items()):
        additional = sorted(packages - desktop[key])
        if not additional:
            continue
        name, identifier, text = key
        print(f"### {name}\n\nSPDX identifier: `{identifier}`\n\nUsed by:\n")
        for crate, version, expression, repository in additional:
            line = f"- `{crate}` {version}"
            if expression:
                line += f" — `{expression}`"
            if repository:
                line += f" — <{repository}>"
            if repository == "https://github.com/mycelium-build/datalith-app":
                line += " <!-- x-release-please-version -->"
            print(line)
        print(f"\n```\n{text}\n```\n")


if __name__ == "__main__":
    render(Path(sys.argv[1]))
