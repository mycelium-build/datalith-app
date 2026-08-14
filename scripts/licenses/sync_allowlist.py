#!/usr/bin/env python3
"""Mirror deny.toml's accepted-license allowlist into scripts/licenses/about.toml.

`deny.toml` `[licenses].allow` is the single source of truth for the accepted license allowlist;
cargo-deny, cargo-about, and the asset validator all derive from it.
This script rewrites the `accepted` array in `scripts/licenses/about.toml`
so it reflects `deny.toml` exactly, preserving the priority order declared there
(which cargo-about treats as significant).

Idempotent: exits 0 without touching the file when nothing would change.

Exit codes follow the LICENSE-E* prefixes.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

import tomllib

REPO_ROOT = Path(__file__).resolve().parents[2]

DENY_TOML = REPO_ROOT / "deny.toml"
ABOUT_TOML = REPO_ROOT / "scripts" / "licenses" / "about.toml"

ACCEPTED_BLOCK = re.compile(r"^accepted = \[.*?\]$", re.MULTILINE | re.DOTALL)


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def load_allowlist() -> list[str]:
    try:
        with DENY_TOML.open("rb") as handle:
            data = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        fail(f"LICENSE-E120 cannot load allowlist: path={DENY_TOML} ({exc})")
    allow = data.get("licenses", {}).get("allow")
    if not allow:
        fail(f"LICENSE-E120 no [licenses].allow in: path={DENY_TOML}")
    return [str(license_id) for license_id in allow]


def render_accepted(allow: list[str]) -> str:
    lines = ["accepted = ["]
    lines.extend(f'    "{license_id}",' for license_id in allow)
    lines.append("]")
    return "\n".join(lines)


def main() -> int:
    allow = load_allowlist()

    text = ABOUT_TOML.read_text(encoding="utf-8")
    if not ACCEPTED_BLOCK.search(text):
        fail(f"LICENSE-E120 cannot locate accepted = [...] block: path={ABOUT_TOML}")

    updated = ACCEPTED_BLOCK.sub(render_accepted(allow), text, count=1)
    if updated != text:
        ABOUT_TOML.write_text(updated, encoding="utf-8")
        print(f"updated {ABOUT_TOML.relative_to(REPO_ROOT)}")
    else:
        print(f"{ABOUT_TOML.relative_to(REPO_ROOT)} already in sync")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
