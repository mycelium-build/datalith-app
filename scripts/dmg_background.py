#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow==12.0.0", "resvg-py==0.5.0"]
# ///
"""Generate the macOS DMG window backgrounds in assets/dmg/.

The artwork is the closing frame of the datalith.work home page hero (a pixel lake landscape where
the monolith floats in its halo), ported from the site's `src/lib/hero-scene.ts`.
The Datalith app icon takes the monolith's place; Finder draws it, and the Applications link, over
the background at the positions set in `packager.<channel>.toml` `[dmg]`, so the layout constants
below must stay in sync with that config.

Each channel gets its own accent: blue for Stable, yellow for Preview.
The scene is built as an SVG of scene pixels plus the "Datalith" title set in Pixeloid Sans,
then rendered by resvg at 2x and tagged 144 dpi, so Finder shows it at the window's size in points
and crisp on Retina displays.

Usage:
  uv run scripts/dmg_background.py [--svg-dir <dir>]
"""

import argparse
import io
import math
from pathlib import Path

from PIL import Image
import resvg_py

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "assets" / "dmg"
FONT = ROOT / "assets" / "fonts" / "Pixeloid" / "PixeloidSans-Bold.ttf"

# Window content geometry, in points. Keep in sync with `[dmg]` in packager.stable.toml and packager.preview.toml,
# whose window-size adds Finder's 28 pt title bar to the height.
WINDOW = (660, 420)
# Finder lays out items without a stored position (the volume's hidden files, when shown) below the
# background image, so the image runs on past the window in the page colour to keep them out of view.
OVERFLOW = 240
APP = (164, 180)
APPLICATIONS = (496, 180)
ICON = 128  # create-dmg's default icon size.

# One scene pixel is PIXEL points, like the site's 4 CSS pixels.
PIXEL = 4
SCALE = 2
COLS = WINDOW[0] // PIXEL
ROWS = WINDOW[1] // PIXEL
HORIZON = 70
TITLE_SIZE = 36  # Pixeloid Sans is drawn on a 9 px em, so 36 pt puts one font pixel on one scene pixel.
TITLE_BASELINE = 16  # In scene rows.

CHANNELS = {
    "stable": {"accent": "#1e8dff", "suffix": None},
    "preview": {"accent": "#e8b920", "suffix": "Preview"},
}

BG = (0xF9, 0xF9, 0xF9)
FG = (0, 0, 0)
WHITE = (255, 255, 255)
SKY_LEVELS = 6
BAYER = [(v + 0.5) / 16 for v in (0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5)]
# The site's still frame for reduced motion: the intro's end plus 1.5 s.
TIME = 6900


def seeded(seed):
    """mulberry32, the site's seeded generator, so the scenery matches it."""
    state = seed & 0xFFFFFFFF

    def imul(a, b):
        return (a * b) & 0xFFFFFFFF

    def next_value():
        nonlocal state
        state = (state + 0x6D2B79F5) & 0xFFFFFFFF
        t = state
        t = imul(t ^ (t >> 15), t | 1)
        t ^= (t + imul(t ^ (t >> 7), t | 61)) & 0xFFFFFFFF
        return ((t ^ (t >> 14)) & 0xFFFFFFFF) / 4294967296

    return next_value


def bayer(x, y):
    return BAYER[(y & 3) * 4 + (x & 3)]


def clamp(value, low, high):
    return min(high, max(low, value))


def hex_rgb(value):
    return tuple(int(value[i : i + 2], 16) for i in (1, 3, 5))


def mix(a, b, t):
    return tuple(a[i] + (b[i] - a[i]) * t for i in range(3))


def palette(accent):
    """The site's light-theme palette (`buildPalette`), derived from one accent colour."""
    acc = hex_rgb(accent)
    sky_max = 0.28
    horizon = mix(BG, acc, sky_max * 0.7)
    ink = mix(FG, acc, 0.25)
    cloud = mix(BG, WHITE, 0.7)
    return {
        "accent": acc,
        "sky": [mix(BG, acc, level / SKY_LEVELS * sky_max) for level in range(SKY_LEVELS + 1)],
        "far": mix(horizon, ink, 0.2),
        "near": mix(horizon, ink, 0.42),
        "water": mix(horizon, ink, 0.06),
        "water_deep": mix(horizon, mix(acc, ink, 0.3), 0.4),
        "shimmer": mix(horizon, WHITE, 0.75),
        "cloud": cloud,
        "cloud_shade": mix(cloud, acc, 0.12),
        "glow": acc,
        # A touch stronger than the site's 0.2: the DMG window is much smaller than the hero.
        "glow_max": 0.28,
    }


def box(center, width, height, pad=0):
    """A rect in scene pixels around a centre given in points."""
    x, y = center
    return (
        (x - width / 2) / PIXEL - pad,
        (y - height / 2) / PIXEL - pad,
        (x + width / 2) / PIXEL + pad,
        (y + height / 2) / PIXEL + pad,
    )


def inside(rect, x, y):
    return rect[0] <= x <= rect[2] and rect[1] <= y <= rect[3]


class Scene:
    def __init__(self, colors, icon):
        self.p = colors
        self.icon = icon
        self.fb = [[BG] * COLS for _ in range(ROWS)]
        self.kinds = [[0] * COLS for _ in range(ROWS)]
        self.cx = round(APP[0] / PIXEL)
        self.cy = round(APP[1] / PIXEL)
        self.app_box = box(APP, ICON, ICON, 2)

    def render(self, title_box):
        self.title_box = title_box
        self.place_scenery()
        self.paint_base()
        self.draw_clouds()
        self.draw_glow()
        self.draw_icon_for_reflection()
        self.draw_water()
        self.paint_base(rows=range(0, HORIZON), only_icon=True)
        self.draw_glow(only_icon=True)
        self.draw_arrow()
        self.draw_mist()
        return self.fb

    def set_kind(self, x, y, kind):
        if 0 <= x < COLS and 0 <= y < HORIZON:
            self.kinds[y][x] = kind

    def place_scenery(self):
        rnd = seeded(7)
        k = 1.4
        ridge_max = 6
        p1, p2, p3 = rnd() * 6, rnd() * 6, rnd() * 6
        for x in range(COLS):
            wave = 0.55 + 0.25 * math.sin(x * 0.045 + p1) + 0.12 * math.sin(x * 0.11 + p2) + 0.08 * math.sin(x * 0.29 + p3)
            for y in range(HORIZON - round(ridge_max * wave), HORIZON):
                self.set_kind(x, y, 1)

        def pine(px, height):
            tier = max(3, round(height / 4))
            trunk = max(1, round(height * 0.1))
            for r in range(height):
                half = 0 if r >= height - trunk else math.floor((r % tier) * 0.7 + r * 0.2)
                for dx in range(-half, half + 1):
                    self.set_kind(px + dx, HORIZON - height + r, 2)

        edge = round(COLS * 0.11)
        for left in (True, False):
            start = -2 * k if left else COLS - edge
            stop = edge if left else COLS + 2 * k
            x = start
            while x < stop:
                toward = 1 - x / edge if left else (x - (COLS - edge)) / edge
                pine(round(x), round((6 + rnd() * 5 + clamp(toward, 0, 1) * 8) * k))
                x += (2 + rnd() * 2.5) * k
            for x in range(math.floor(start), math.ceil(stop)):
                for y in range(HORIZON - 2, HORIZON):
                    self.set_kind(x, y, 2)

        self.clouds = [(4, 24, 15), (124, 19, 22), (86, 7, 12)]
        water_rows = ROWS - HORIZON
        self.mist_rows = min(round(water_rows * 0.45), 26)
        self.streaks = []
        for _ in range(round(COLS * water_rows / 45)):
            self.streaks.append(
                (
                    math.floor(rnd() * COLS),
                    1 + math.floor(rnd() * max(1, water_rows - self.mist_rows)),
                    2 + math.floor(rnd() * 3),
                    rnd() * math.pi * 2,
                    0.6 + rnd() * 1.4,
                )
            )
        q1, q2, q3 = rnd() * 6, rnd() * 6, rnd() * 6
        self.mist = [0.5 * math.sin(x * 0.13 + q1) + 0.3 * math.sin(x * 0.37 + q2) + 0.2 * math.sin(x * 0.71 + q3) for x in range(COLS)]

    def paint_base(self, rows=range(HORIZON), only_icon=False):
        P = self.p
        spread = COLS * 0.22
        for y in rows:
            t = (y + 1) / HORIZON
            ambient = t**2.4 * 0.6
            lift = t**3 * 0.45
            for x in range(COLS):
                if only_icon and not inside(self.app_box, x, y):
                    continue
                kind = self.kinds[y][x]
                if kind == 1:
                    self.fb[y][x] = P["far"]
                elif kind == 2:
                    self.fb[y][x] = P["near"]
                else:
                    glow = math.exp(-(((x - self.cx) / spread) ** 2))
                    level = min(1, ambient + glow * lift) * SKY_LEVELS
                    step = math.floor(level)
                    if level - step > bayer(x, y):
                        step += 1
                    self.fb[y][x] = P["sky"][min(step, SKY_LEVELS)]

    def draw_clouds(self):
        for x0, y0, w in self.clouds:
            band = max(1, round(w / 8))
            for dy, start, stop in ((0, 0.3, 0.62), (band, 0.12, 0.85), (2 * band, 0, 1)):
                for r in range(band):
                    y = y0 + dy + r
                    color = self.p["cloud_shade"] if dy == 2 * band and r == band - 1 else self.p["cloud"]
                    x = x0 + round(w * start)
                    while x < x0 + w * stop:
                        if 0 <= x < COLS and self.kinds[y][x] == 0 and not inside(self.title_box, x, y):
                            self.fb[y][x] = color
                        x += 1

    def draw_glow(self, only_icon=False):
        radius = 32 * 0.75
        for y in range(max(0, math.floor(self.cy - radius)), min(HORIZON - 1, math.ceil(self.cy + radius)) + 1):
            for x in range(max(0, math.floor(self.cx - radius)), min(COLS - 1, math.ceil(self.cx + radius)) + 1):
                if only_icon and not inside(self.app_box, x, y):
                    continue
                d = math.hypot(x - self.cx, y - self.cy) / radius
                if d >= 1:
                    continue
                amount = math.floor((1 - d) ** 2 * self.p["glow_max"] * 4 + bayer(x, y)) / 4
                if amount > 0:
                    self.fb[y][x] = mix(self.fb[y][x], self.p["glow"], amount)

    def draw_icon_for_reflection(self):
        """Paints the app icon so the lake mirrors it; Finder's real icon then covers this copy."""
        size = ICON // PIXEL
        left = round(APP[0] / PIXEL - size / 2)
        top = round(APP[1] / PIXEL - size / 2)
        for y in range(size):
            for x in range(size):
                r, g, b, a = self.icon.getpixel((x, y))
                if a > 128 and 0 <= top + y < HORIZON:
                    self.fb[top + y][left + x] = (r, g, b)

    def draw_water(self):
        P = self.p
        water_rows = ROWS - HORIZON
        for y in range(HORIZON, ROWS):
            depth = (y - HORIZON) / water_rows
            source = max(0, 2 * HORIZON - 1 - y)
            offset = round(math.sin(y * 0.9 + TIME * 0.0026) * (0.5 + depth * 2.5))
            water = mix(P["water"], P["water_deep"], depth)
            calm = math.sin(y * 1.7 + TIME * 0.0013) > 0.8
            strength = 0.3 if y == HORIZON else 0.85 if calm else 0.45 + depth * 0.35
            tint = P["shimmer"] if y == HORIZON else water
            row = [mix(self.fb[source][clamp(x + offset, 0, COLS - 1)], tint, strength) for x in range(COLS)]
            self.fb[y] = row
        for x0, row, length, phase, speed in self.streaks:
            if math.sin(TIME * speed / 1000 + phase) < 0.55:
                continue
            y = HORIZON + row
            for x in range(x0, min(COLS, x0 + length)):
                self.fb[y][x] = mix(self.fb[y][x], P["shimmer"], 0.6)

    def arrow_span(self):
        gap = ICON / 2 + 18
        return round((APP[0] + gap) / PIXEL), round((APPLICATIONS[0] - gap) / PIXEL), round(APP[1] / PIXEL)

    def draw_arrow(self):
        """A pixel arrow from the app to Applications: a dashed shaft, then a chevron."""
        x0, x1, y = self.arrow_span()
        color = self.p["accent"]
        head = 4
        for x in range(x0, x1 - head):
            if (x - x0) % 3 != 2:
                self.fb[y][x] = color
        for i in range(head + 1):
            for dy in (-i, i):
                self.fb[y + dy][x1 - i] = color
                self.fb[y + dy][x1 - i - 1] = color

    def draw_mist(self):
        start = ROWS - self.mist_rows
        for y in range(max(0, start), ROWS):
            t = (y - start + 1) / self.mist_rows
            for x in range(COLS):
                if t * 1.5 - 0.25 + self.mist[x] * 0.25 > bayer(x, y):
                    self.fb[y][x] = BG


def title_runs(suffix):
    """Measures the title in Pixeloid Sans and centres it on the scene grid."""
    from PIL import ImageFont

    font = ImageFont.truetype(str(FONT), TITLE_SIZE)
    parts = [("Datalith", None)] + ([(" " + suffix, "accent")] if suffix else [])
    widths = [font.getlength(text) for text, _ in parts]
    left = round((WINDOW[0] - sum(widths)) / 2 / PIXEL) * PIXEL
    runs, x = [], left
    for (text, color), width in zip(parts, widths):
        runs.append((x, text, color))
        x += width
    baseline = TITLE_BASELINE * PIXEL
    title_box = (left / PIXEL - 3, TITLE_BASELINE - 9 - 3, x / PIXEL + 3, TITLE_BASELINE + 3 + 3)
    return runs, baseline, title_box


def css(color):
    return "#" + "".join(f"{round(v):02x}" for v in color)


def to_svg(fb, runs, baseline, accent):
    rects = []
    for y, row in enumerate(fb):
        x = 0
        while x < COLS:
            color = css(row[x])
            end = x + 1
            while end < COLS and css(row[end]) == color:
                end += 1
            rects.append(f'<rect x="{x * PIXEL}" y="{y * PIXEL}" width="{(end - x) * PIXEL}" height="{PIXEL}" fill="{color}"/>')
            x = end
    texts = "".join(
        f'<text x="{x:g}" y="{baseline}" fill="{accent if color else "#000000"}" xml:space="preserve">{text}</text>'
        for x, text, color in runs
    )
    width, height = WINDOW[0], WINDOW[1] + OVERFLOW
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" '
        f'shape-rendering="crispEdges"><rect width="{width}" height="{height}" fill="{css(BG)}"/>{"".join(rects)}'
        f'<g font-family="Pixeloid Sans" font-weight="700" font-size="{TITLE_SIZE}">{texts}</g></svg>'
    )


def build(channel, svg_dir=None):
    config = CHANNELS[channel]
    colors = palette(config["accent"])
    size = ICON // PIXEL
    icon = Image.open(ROOT / "assets" / "logo" / channel / "datalith-macos.png").convert("RGBA").resize((size, size), Image.BOX)
    runs, baseline, title_box = title_runs(config["suffix"])
    fb = Scene(colors, icon).render(title_box)
    svg = to_svg(fb, runs, baseline, config["accent"])
    if svg_dir:
        Path(svg_dir).mkdir(parents=True, exist_ok=True)
        (Path(svg_dir) / f"{channel}.svg").write_text(svg)
    png = resvg_py.svg_to_bytes(
        svg_string=svg,
        zoom=SCALE,
        skip_system_fonts=True,
        font_files=[str(FONT)],
        shape_rendering="crisp_edges",
        text_rendering="optimize_speed",
    )
    image = Image.open(io.BytesIO(bytes(png))).convert("RGB")
    assert image.size == (WINDOW[0] * SCALE, (WINDOW[1] + OVERFLOW) * SCALE), image.size
    OUTPUT.mkdir(parents=True, exist_ok=True)
    path = OUTPUT / f"background-{channel}.png"
    image.save(path, dpi=(72 * SCALE, 72 * SCALE), optimize=True)
    print(f"Wrote {path.relative_to(ROOT)} ({image.width}x{image.height} px, {72 * SCALE} dpi)")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--svg-dir", help="Also write the intermediate SVGs here")
    args = parser.parse_args()
    for channel in CHANNELS:
        build(channel, args.svg_dir)


if __name__ == "__main__":
    main()
