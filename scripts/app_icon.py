#!/usr/bin/env python3
"""Generate every Datalith app-icon format from the pixel-art logo.

Reads `assets/datalith.txt` (a 32×32 grid: `M`/`1`/`2` on white) and writes
the app icons for every platform into `assets/`:

  assets/datalith.svg      32×32 vector source (Linux/SVG, icon source)
  assets/datalith.png      1024×1024 (32× per logo pixel) — Linux / bundles
  assets/datalith.ico      16/24/32/48/64/128/256 embedded PNGs — Windows
  assets/datalith.icns     icp4/icp5/icp6/ic07/ic08/ic09/ic10 — macOS

Everything is produced with the standard library (zlib/struct) — the logo is
pixel art, so no SVG rasterizer is needed.

Usage:
  uv run scripts/app_icon.py
"""

import pathlib
import struct
import sys
import zlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOGO = ROOT / "assets" / "datalith.txt"
SVG_OUT = ROOT / "assets" / "datalith.svg"
PNG_OUT = ROOT / "assets" / "datalith.png"
ICO_OUT = ROOT / "assets" / "datalith.ico"
ICNS_OUT = ROOT / "assets" / "datalith.icns"

SIZE = 32

PRIMARY = (0x1E, 0x8D, 0xFF)
RIGHT_SIDE_WHITEN = 0.35
LEFT_SIDE_WHITEN = 0.7


def rgb_to_hsl(rgb: tuple[int, int, int]) -> tuple[float, float, float]:
    r, g, b = (c / 255.0 for c in rgb)
    mx, mn = max(r, g, b), min(r, g, b)
    delta = mx - mn
    l = (mx + mn) / 2.0
    if l in (0.0, 1.0):
        s = 0.0
    elif l < 0.5:
        s = delta / (2.0 * l)
    else:
        s = delta / (2.0 - 2.0 * l)
    if delta == 0.0:
        h = 0.0
    elif mx == r:
        h = ((g - b) / delta) % 6.0 / 6.0
    elif mx == g:
        h = ((b - r) / delta + 2.0) / 6.0
    else:
        h = ((r - g) / delta + 4.0) / 6.0
    return (h, s, l)


def hsl_to_rgb(h: float, s: float, l: float) -> tuple[int, int, int]:
    # Mirrors gpui's `Hsla -> Rgba` conversion.
    c = (1.0 - abs(2.0 * l - 1.0)) * s
    x = c * (1.0 - abs((h * 6.0) % 2.0 - 1.0))
    m = l - c / 2.0
    arm = int(h * 6.0) % 6
    if arm == 0:
        r, g, b = c + m, x + m, m
    elif arm == 1:
        r, g, b = x + m, c + m, m
    elif arm == 2:
        r, g, b = m, c + m, x + m
    elif arm == 3:
        r, g, b = m, x + m, c + m
    elif arm == 4:
        r, g, b = x + m, m, c + m
    else:
        r, g, b = c + m, m, x + m
    return tuple(min(255, max(0, round(v * 255))) for v in (r, g, b))


def whiten(rgb: tuple[int, int, int], amount: float) -> tuple[int, int, int]:
    h, s, l = rgb_to_hsl(rgb)
    return hsl_to_rgb(h, s * (1.0 - amount), l + (1.0 - l) * amount)


TIER_COLORS = {
    "M": PRIMARY,
    "1": whiten(PRIMARY, RIGHT_SIDE_WHITEN),
    "2": whiten(PRIMARY, LEFT_SIDE_WHITEN),
    "I": (0xFF, 0xFF, 0xFF),
}
WHITE = (0xFF, 0xFF, 0xFF)
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]
ICNS_IMAGES = [
    ("icp4", 16),
    ("icp5", 32),
    ("icp6", 64),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic10", 1024),
]


def load_grid() -> list[list[tuple[int, int, int, int]]]:
    rows = LOGO.read_text(encoding="utf-8").splitlines()
    if len(rows) > SIZE:
        raise ValueError(f"logo has {len(rows)} rows, expected at most {SIZE}")
    grid: list[list[tuple[int, int, int, int]]] = []
    for row in rows:
        cells = []
        for ch in row[:SIZE]:
            color = TIER_COLORS.get(ch, WHITE)
            cells.append((*color, 255))
        while len(cells) < SIZE:
            cells.append((*WHITE, 255))
        grid.append(cells)
    while len(grid) < SIZE:
        grid.append([(*WHITE, 255)] * SIZE)
    return grid


def scale_up(grid, factor: int) -> list[list[tuple[int, int, int, int]]]:
    out: list[list[tuple[int, int, int, int]]] = []
    for row in grid:
        for _ in range(factor):
            out.append([cell for cell in row for _ in range(factor)])
    return out


def scale_down(grid, target: int) -> list[list[tuple[int, int, int, int]]]:
    out: list[list[tuple[int, int, int, int]]] = []
    for y in range(target):
        out.append([grid[y * SIZE // target][x * SIZE // target] for x in range(target)])
    return out


def scale_to(grid, target: int) -> list[list[tuple[int, int, int, int]]]:
    if target % SIZE == 0:
        return scale_up(grid, target // SIZE)
    return scale_down(grid, target)


def png_bytes(grid) -> bytes:
    height = len(grid)
    width = len(grid[0])
    raw = bytearray()
    for row in grid:
        raw.append(0)  # filter: none
        for r, g, b, a in row:
            raw += bytes((r, g, b, a))
    ihdr = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + _chunk(b"IHDR", ihdr)
        + _chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + _chunk(b"IEND", b"")
    )


def _chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def write_svg(grid) -> None:
    rects = []
    for y, row in enumerate(grid):
        for x, (r, g, b, a) in enumerate(row):
            if (r, g, b, a) == (*WHITE, 255):
                continue
            rects.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="#{r:02x}{g:02x}{b:02x}"/>')
    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">'
        f'<rect x="0" y="0" width="32" height="32" fill="#ffffff"/>{''.join(rects)}</svg>'
    )
    SVG_OUT.write_text(svg, encoding="utf-8")
    print(f"wrote {SVG_OUT.relative_to(ROOT)}")


def write_png(grid, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png_bytes(grid))
    print(f"wrote {path.relative_to(ROOT)}")


def write_ico(grid) -> None:
    blobs = []
    for size in ICO_SIZES:
        blobs.append((size, png_bytes(scale_to(grid, size))))
    header = struct.pack("<HHH", 0, 1, len(blobs))
    offset = 6 + 16 * len(blobs)
    entries = bytearray()
    payload = bytearray()
    for size, data in blobs:
        byte = 0 if size == 256 else size
        entries += struct.pack("<BBBBHHII", byte, byte, 0, 0, 1, 32, len(data), offset)
        payload += data
        offset += len(data)
    ICO_OUT.parent.mkdir(parents=True, exist_ok=True)
    ICO_OUT.write_bytes(header + bytes(entries) + bytes(payload))
    print(f"wrote {ICO_OUT.relative_to(ROOT)}")


def write_icns(grid) -> None:
    chunks = []
    for kind, size in ICNS_IMAGES:
        data = png_bytes(scale_to(grid, size))
        chunks.append(kind.encode() + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    ICNS_OUT.parent.mkdir(parents=True, exist_ok=True)
    ICNS_OUT.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)
    print(f"wrote {ICNS_OUT.relative_to(ROOT)}")


def main() -> int:
    grid = load_grid()
    write_svg(grid)
    write_png(scale_to(grid, 1024), PNG_OUT)
    write_ico(grid)
    write_icns(grid)
    return 0


if __name__ == "__main__":
    sys.exit(main())
