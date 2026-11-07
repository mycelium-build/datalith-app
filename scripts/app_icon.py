#!/usr/bin/env python3
"""Generate blue Stable, yellow Preview, and green Dev icons in assets/logo/.

This writes the flat pixel-art icons (SVG, PNG, hicolor, ICO, Windows resource).
The layered macOS icons (Icon Composer document, Assets.car, .icns) come from scripts/macos_icon.py,
which reuses the colors defined here.

Usage: python3 scripts/app_icon.py
"""

import pathlib
import struct
import sys
import zlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOGO = ROOT / "assets" / "logo" / "datalith.txt"
HICOLOR_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024]

SIZE = 32

PRIMARY = (0x1E, 0x8D, 0xFF)

# Whitening applied to PRIMARY for the monolith's two sides.
# Mirrored by src/ui/monolith.rs (RIGHT_SIDE_WHITEN / LEFT_SIDE_WHITEN); keep in sync.
RIGHT_SIDE_WHITEN = 0.35
LEFT_SIDE_WHITEN = 0.7

# Background of the Stable macOS icon (see scripts/macos_icon.py).
MACOS_BG = (0x23, 0x8C, 0xFF)


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


# Monolith color per channel; scripts/macos_icon.py adds each one's macOS background.
CHANNELS = (
    ("stable", PRIMARY),
    ("preview", (0xE8, 0xB9, 0x20)),
    ("dev", (0x30, 0xBA, 0x78)),
)

WHITE = (0xFF, 0xFF, 0xFF)
TRANSPARENT = (0, 0, 0, 0)
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def load_grid(primary, border=None) -> list[list[tuple[int, int, int, int]]]:
    border = primary if border is None else border
    colors = {"M": primary, "1": whiten(primary, RIGHT_SIDE_WHITEN),
              "2": whiten(primary, LEFT_SIDE_WHITEN), "I": WHITE, "0": WHITE}
    rows = LOGO.read_text(encoding="utf-8").splitlines()
    if len(rows) > SIZE:
        raise ValueError(f"logo has {len(rows)} rows, expected at most {SIZE}")
    grid: list[list[tuple[int, int, int, int]]] = []
    for row in rows:
        cells = []
        for ch in row[:SIZE]:
            if ch == "M":
                cells.append((*border, 255))
            elif ch in colors:
                cells.append((*colors[ch], 255))
            else:
                cells.append(TRANSPARENT)
        while len(cells) < SIZE:
            cells.append(TRANSPARENT)
        grid.append(cells)
    while len(grid) < SIZE:
        grid.append([TRANSPARENT] * SIZE)
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


def write_svg(grid, directory) -> None:
    rects = []
    for y, row in enumerate(grid):
        for x, (r, g, b, a) in enumerate(row):
            if a == 0:
                continue
            rects.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="#{r:02x}{g:02x}{b:02x}"/>')
    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" '
        'viewBox="0 0 32 32" shape-rendering="crispEdges">'
        f'{''.join(rects)}</svg>'
    )
    (directory / "datalith.svg").write_text(svg, encoding="utf-8")
    print(f"wrote {(directory / "datalith.svg").relative_to(ROOT)}")


def write_png(grid, path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png_bytes(grid))
    print(f"wrote {path.relative_to(ROOT)}")


def write_hicolor(grid, directory) -> None:
    for size in HICOLOR_SIZES:
        path = (directory / "hicolor") / f"{size}x{size}" / "apps" / "datalith.png"
        write_png(scale_to(grid, size), path)


def write_ico(grid, directory) -> None:
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
    (directory / "datalith.ico").parent.mkdir(parents=True, exist_ok=True)
    (directory / "datalith.ico").write_bytes(header + bytes(entries) + bytes(payload))
    print(f"wrote {(directory / "datalith.ico").relative_to(ROOT)}")


def write_rc(directory) -> None:
    ico_path = (directory / "datalith.ico").relative_to(ROOT).as_posix()
    (directory / "datalith.rc").write_text(f'1 ICON "{ico_path}"\n', encoding="utf-8")
    print(f"wrote {(directory / "datalith.rc").relative_to(ROOT)}")


def main() -> int:
    for channel, primary in CHANNELS:
        directory = ROOT / "assets/logo" / channel
        directory.mkdir(parents=True, exist_ok=True)
        grid = load_grid(primary)
        write_svg(grid, directory)
        write_png(scale_to(grid, 1024), directory / "datalith.png")
        write_hicolor(grid, directory)
        write_ico(grid, directory)
        write_rc(directory)
    return 0


if __name__ == "__main__":
    sys.exit(main())
