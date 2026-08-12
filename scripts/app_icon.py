#!/usr/bin/env python3
"""Generate every Datalith app-icon format from the pixel-art logo.

Reads `assets/datalith.txt` (a 32×32 grid: `M` border, `1`/`2` sides, `0` top
side and `I` inscriptions) and writes the app icons for every platform into
`assets/`:

  assets/datalith.svg      32×32 vector source (Linux/SVG, icon source)
  assets/datalith.png      1024×1024 (32× per logo pixel) — Linux / bundles
  assets/datalith.ico      16/24/32/48/64/128/256 embedded PNGs — Windows
  assets/datalith.icns     icp4/icp5/icp6/ic07/ic08/ic09/ic10 — macOS
  assets/datalith-macos.png  1024×1024 viewable preview of the macOS icon
  assets/datalith.rc       Windows resource file embedding the .ico

Linux and Windows get the bare monolith mark on a transparent background, so
it floats with no chrome. macOS gets the classic Big-Sur layout: the monolith
centred on a solid-blue squircle with continuous corners, with the `M` border
drawn white so it reads against the blue. Everything is
produced with the standard library (zlib/struct) — the logo is pixel art, so
no SVG rasterizer is needed.

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
MACOS_PNG_OUT = ROOT / "assets" / "datalith-macos.png"
RC_OUT = ROOT / "assets" / "datalith.rc"

SIZE = 32

PRIMARY = (0x1E, 0x8D, 0xFF)

# Whitening applied to PRIMARY for the monolith's two sides.
# Mirrored by src/ui/monolith.rs (RIGHT_SIDE_WHITEN / LEFT_SIDE_WHITEN); keep in sync.
RIGHT_SIDE_WHITEN = 0.35
LEFT_SIDE_WHITEN = 0.7

# macOS (Big Sur) icon layout.
# The background is Apple's true app-icon shape: a continuous-corner rounded
# rectangle (UIBezierPath(roundedRect:cornerRadius:)), not a superellipse. The
# corner is three cubic Bézier segments reverse-engineered from that call, with
# corner radius CORNER_RADIUS_FRACTION of the icon and the cut point CORNER_CUT
# along each edge (Liam Rosenfeld, "My Quest for the Apple Icon Shape").
# The mark is scaled to MARK_FRACTION of the icon and centred.
MACOS_BG = (0x23, 0x8C, 0xFF)
MACOS_MARK_FRACTION = 0.80
CORNER_RADIUS_FRACTION = 0.225
CORNER_CUT = 1.52866498
CORNER = (
    # (p0, c1, c2, p3) per corner quadrant, normalized by the corner radius.
    ((1.528665, 0.0), (1.08849296, 0.0), (0.86840694, 0.0), (0.63149379, 0.07491139)),
    ((0.63149379, 0.07491139), (0.37282383, 0.16905956), (0.16905956, 0.37282383), (0.07491139, 0.63149379)),
    ((0.07491139, 0.63149379), (0.0, 0.86840694), (0.0, 1.08849296), (0.0, 1.52866498)),
)


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
    "0": (0xFF, 0xFF, 0xFF),
}
WHITE = (0xFF, 0xFF, 0xFF)
TRANSPARENT = (0, 0, 0, 0)
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


def load_grid(border: tuple[int, int, int] = PRIMARY) -> list[list[tuple[int, int, int, int]]]:
    rows = LOGO.read_text(encoding="utf-8").splitlines()
    if len(rows) > SIZE:
        raise ValueError(f"logo has {len(rows)} rows, expected at most {SIZE}")
    grid: list[list[tuple[int, int, int, int]]] = []
    for row in rows:
        cells = []
        for ch in row[:SIZE]:
            if ch == "M":
                cells.append((*border, 255))
            elif ch in TIER_COLORS:
                cells.append((*TIER_COLORS[ch], 255))
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


def write_svg(grid) -> None:
    rects = []
    for y, row in enumerate(grid):
        for x, (r, g, b, a) in enumerate(row):
            if a == 0:
                continue
            rects.append(f'<rect x="{x}" y="{y}" width="1" height="1" fill="#{r:02x}{g:02x}{b:02x}"/>')
    svg = (
        '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">'
        f'{''.join(rects)}</svg>'
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


def write_icns() -> None:
    full = macos_grid(ICNS_IMAGES[-1][1])
    chunks = []
    for kind, size in ICNS_IMAGES:
        grid = full if size == ICNS_IMAGES[-1][1] else resample_down(full, size)
        data = png_bytes(grid)
        chunks.append(kind.encode() + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    ICNS_OUT.parent.mkdir(parents=True, exist_ok=True)
    ICNS_OUT.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)
    print(f"wrote {ICNS_OUT.relative_to(ROOT)}")
    write_png(full, MACOS_PNG_OUT)


def _bezier(p0, p1, p2, p3, t: float) -> tuple[float, float]:
    mt = 1.0 - t
    a = mt * mt * mt
    b = 3.0 * mt * mt * t
    c = 3.0 * mt * t * t
    d = t * t * t
    return (
        a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0],
        a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1],
    )


def _corner_x(yn: float) -> float:
    """The x-coordinate (normalized by corner radius) of the corner curve at height yn."""
    for p0, p1, p2, p3 in CORNER:
        if yn <= p3[1]:
            break
    else:
        return 0.0
    lo, hi = 0.0, 1.0
    for _ in range(48):
        mid = (lo + hi) / 2.0
        if _bezier(p0, p1, p2, p3, mid)[1] < yn:
            lo = mid
        else:
            hi = mid
    return _bezier(p0, p1, p2, p3, (lo + hi) / 2.0)[0]


def macos_grid(size: int) -> list[list[tuple[int, int, int, int]]]:
    """The mark centred on a solid-blue continuous-corner (Apple) squircle."""
    logo = load_grid(border=WHITE)
    cell = max(1, round(size * MACOS_MARK_FRACTION / SIZE))
    logo_px = cell * SIZE
    origin = (size - logo_px) // 2
    radius = CORNER_RADIUS_FRACTION * size
    cut = CORNER_CUT * radius

    # Left boundary tabulated at eighth-pixel vertical resolution for the top
    # half; the corner curve is steep near the top edge, so a single row-centre
    # sample under-renders the edge.
    samples = 8
    table_len = size * samples // 2 + 1
    left_tab = [0.0] * table_len
    for q in range(table_len):
        dy = q / samples
        left_tab[q] = 0.0 if dy >= cut else _corner_x(dy / radius) * radius

    grid: list[list[tuple[int, int, int, int]]] = []
    for y in range(size):
        in_logo_row = origin <= y < origin + logo_px
        logo_row = logo[(y - origin) // cell] if in_logo_row else None
        row: list[tuple[int, int, int, int]] = []
        for x in range(size):
            cov = 0.0
            for k in range(samples):
                dy = min(y + (k + 0.5) / samples, size - y - (k + 0.5) / samples)
                left = left_tab[int(dy * samples + 0.5)]
                right = size - left
                lo = x if x > left else left
                hi = (x + 1.0) if (x + 1.0) < right else right
                if hi > lo:
                    cov += hi - lo
            alpha = round(255 * cov / samples)
            if logo_row is not None and origin <= x < origin + logo_px:
                lr, lg, lb, la = logo_row[(x - origin) // cell]
                if la and alpha:
                    row.append((lr, lg, lb, alpha))
                    continue
            row.append((*MACOS_BG, alpha))
        grid.append(row)
    return grid


def resample_down(grid, target: int) -> list[list[tuple[int, int, int, int]]]:
    """Box-filter downsample an arbitrary-size RGBA grid, in premultiplied space."""
    src = len(grid)
    out: list[list[tuple[int, int, int, int]]] = []
    for ty in range(target):
        y0 = ty * src / target
        y1 = (ty + 1) * src / target
        row: list[tuple[int, int, int, int]] = []
        for tx in range(target):
            x0 = tx * src / target
            x1 = (tx + 1) * src / target
            row.append(_box_average(grid, x0, y0, x1, y1))
        out.append(row)
    return out


def _box_average(
    grid, x0: float, y0: float, x1: float, y1: float
) -> tuple[int, int, int, int]:
    pr = pg = pb = pa = 0.0
    y_start = max(0, int(y0))
    y_end = min(len(grid), int(y1) + 1)
    x_start = max(0, int(x0))
    x_end = min(len(grid[0]), int(x1) + 1)
    for py in range(y_start, y_end):
        oy0 = max(py, y0)
        oy1 = min(py + 1, y1)
        if oy1 <= oy0:
            continue
        hy = oy1 - oy0
        row = grid[py]
        for px in range(x_start, x_end):
            ox0 = max(px, x0)
            ox1 = min(px + 1, x1)
            if ox1 <= ox0:
                continue
            weight = (ox1 - ox0) * hy
            r, g, b, a = row[px]
            if a == 0:
                continue
            fa = a / 255.0
            pr += r * fa * weight
            pg += g * fa * weight
            pb += b * fa * weight
            pa += fa * weight
    area = (x1 - x0) * (y1 - y0)
    if area <= 0.0 or pa <= 0.0:
        return TRANSPARENT
    return (round(pr / pa), round(pg / pa), round(pb / pa), round(255 * pa / area))


def write_rc() -> None:
    ico_path = ICO_OUT.relative_to(ROOT).as_posix()
    RC_OUT.write_text(f'1 ICON "{ico_path}"\n', encoding="utf-8")
    print(f"wrote {RC_OUT.relative_to(ROOT)}")


def main() -> int:
    grid = load_grid()
    write_svg(grid)
    write_png(scale_to(grid, 1024), PNG_OUT)
    write_ico(grid)
    write_icns()
    write_rc()
    return 0


if __name__ == "__main__":
    sys.exit(main())
