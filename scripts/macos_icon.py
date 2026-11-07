#!/usr/bin/env python3
# /// script
# requires-python = ">=3.12"
# dependencies = ["pillow==12.0.0"]
# ///
"""Generate the layered macOS app icons of the Stable, Preview and Dev channels with Icon Composer.

For each channel this writes an Icon Composer document, assets/logo/<channel>/datalith.icon,
whose layers are the monolith's parts (outline, top, left and right faces, glyphs) cut from
assets/logo/datalith.txt over a channel-colored background, so macOS applies Liquid Glass to each
of them. The document is then compiled with Xcode's actool and exported with Icon Composer's
ictool into:

  assets/logo/<channel>/Assets.car         the Liquid Glass icon (Info.plist CFBundleIconName)
  assets/logo/<channel>/datalith.icns      the flattened fallback (CFBundleIconFile)
  assets/logo/<channel>/datalith-macos.png the 1024 px default rendition, full bleed (DMG background)
  assets/logo/<channel>/datalith-dock.png  the same on Apple's icon grid, an 824 px icon centred in
                                           1024 px, which unbundled runs set as their Dock icon

This needs macOS with Xcode 26 or later, which ships Icon Composer; the script checks for it first.

Usage: uv run scripts/macos_icon.py
"""

import importlib.util
import json
import pathlib
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOGO = ROOT / "assets" / "logo" / "datalith.txt"

# The icon name actool compiles: Assets.car holds the "datalith" app icon and actool writes datalith.icns.
# Must match CFBundleIconName in assets/logo/macos-Info.plist.
ICON_NAME = "datalith"
MINIMUM_TOOL_VERSION = 26
CANVAS = 1024
# Apple's macOS icon grid: the icon body is 824 px of a 1024 px image, the rest is margin.
GRID_BODY = 824

# Height of the monolith as a fraction of the icon canvas.
MARK_FRACTION = 0.82

_spec = importlib.util.spec_from_file_location("app_icon", pathlib.Path(__file__).with_name("app_icon.py"))
app_icon = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(app_icon)

# (channel, monolith color, background color): Stable has its own background, the others use their monolith color.
CHANNELS = tuple(
    (channel, primary, app_icon.MACOS_BG if channel == "stable" else primary)
    for channel, primary in app_icon.CHANNELS
)

# Layers, front to back: (layer name, cells of datalith.txt it covers, color of the layer).
# The outline is white on macOS so the monolith stands out of the channel-colored background.
LAYERS = (
    ("glyphs", "I", lambda primary: app_icon.WHITE),
    ("top", "0", lambda primary: app_icon.WHITE),
    ("right", "1", lambda primary: app_icon.whiten(primary, app_icon.RIGHT_SIDE_WHITEN)),
    ("left", "2", lambda primary: app_icon.whiten(primary, app_icon.LEFT_SIDE_WHITEN)),
    ("outline", "M", lambda primary: app_icon.WHITE),
)


class ToolError(RuntimeError):
    pass


def _major(version: str) -> int:
    match = re.match(r"(\d+)", version)
    return int(match.group(1)) if match else 0


def find_tools() -> tuple[pathlib.Path, pathlib.Path]:
    """Return (ictool, actool), or raise ToolError saying why Icon Composer cannot be used."""
    if platform.system() != "Darwin":
        raise ToolError("Icon Composer only runs on macOS; build the macOS icons on a Mac.")
    try:
        developer = pathlib.Path(subprocess.check_output(["xcode-select", "-p"], text=True).strip())
    except (OSError, subprocess.CalledProcessError) as error:
        raise ToolError(f"Xcode is not installed or not selected (xcode-select -p failed: {error}).") from error
    candidates = [
        developer.parent / "Applications" / "Icon Composer.app",
        pathlib.Path("/Applications/Icon Composer.app"),
    ]
    ictool = next(
        (app / "Contents" / "Executables" / "ictool" for app in candidates
         if (app / "Contents" / "Executables" / "ictool").is_file()),
        None,
    )
    if ictool is None:
        raise ToolError(
            f"Icon Composer not found (looked in {', '.join(str(c) for c in candidates)}). "
            f"Install Xcode {MINIMUM_TOOL_VERSION} or later and select it with `sudo xcode-select -s`."
        )
    try:
        ictool_version = json.loads(subprocess.check_output([str(ictool), "--version"], text=True))
        ictool_version = ictool_version["short-bundle-version"]
    except (OSError, subprocess.CalledProcessError, ValueError, KeyError) as error:
        raise ToolError(f"{ictool} --version failed: {error}") from error
    if _major(ictool_version) < MINIMUM_TOOL_VERSION:
        raise ToolError(f"Icon Composer {ictool_version} is too old; need {MINIMUM_TOOL_VERSION} or later.")

    try:
        actool = pathlib.Path(subprocess.check_output(["xcrun", "--find", "actool"], text=True).strip())
        output = subprocess.check_output([str(actool), "--version"], text=True)
    except (OSError, subprocess.CalledProcessError) as error:
        raise ToolError(f"Xcode's actool is unavailable: {error}") from error
    match = re.search(r"<key>short-bundle-version</key>\s*<string>([^<]+)</string>", output)
    actool_version = match.group(1) if match else "unknown"
    if _major(actool_version) < MINIMUM_TOOL_VERSION:
        raise ToolError(
            f"actool {actool_version} cannot compile Icon Composer documents; "
            f"need Xcode {MINIMUM_TOOL_VERSION} or later."
        )
    print(f"using Icon Composer {ictool_version} ({ictool}) and actool {actool_version}")
    return ictool, actool


def _hex(rgb: tuple[int, int, int]) -> str:
    return "#{:02x}{:02x}{:02x}".format(*rgb)


def _srgb(rgb: tuple[int, int, int]) -> str:
    return "extended-srgb:" + ",".join(f"{c / 255:.5f}" for c in rgb) + ",1.00000"


def _mark_cells() -> tuple[list[str], tuple[int, int, int, int]]:
    rows = LOGO.read_text(encoding="utf-8").splitlines()
    cells = [(x, y) for y, row in enumerate(rows) for x, ch in enumerate(row) if ch != " "]
    xs = [x for x, _ in cells]
    ys = [y for _, y in cells]
    return rows, (min(xs), min(ys), max(xs) + 1, max(ys) + 1)


def layer_svg(rows: list[str], bbox, chars: str, color) -> str:
    """One layer as a full-canvas SVG, so every layer lines up with the others when centred."""
    x0, y0, x1, y1 = bbox
    cell = CANVAS * MARK_FRACTION / (y1 - y0)
    ox = (CANVAS - cell * (x1 - x0)) / 2 - cell * x0
    oy = (CANVAS - cell * (y1 - y0)) / 2 - cell * y0
    # One path of horizontal runs in grid units, scaled once; adjacent squares in a single
    # path union without the hairline seams that separate rectangles would leave.
    runs = []
    for y, row in enumerate(rows):
        x = 0
        while x < len(row):
            if row[x] in chars:
                start = x
                while x < len(row) and row[x] in chars:
                    x += 1
                runs.append(f"M{start} {y}h{x - start}v1h{start - x}z")
            else:
                x += 1
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{CANVAS}" height="{CANVAS}" '
        f'viewBox="0 0 {CANVAS} {CANVAS}">'
        f'<path transform="translate({ox:.3f} {oy:.3f}) scale({cell:.5f})" '
        f'fill="{_hex(color)}" d="{"".join(runs)}"/></svg>\n'
    )


def icon_document(background) -> dict:
    def layer(name: str) -> dict:
        return {"name": name, "image-name": f"{name}.svg", "glass": True}

    return {
        "fill": {"automatic-gradient": _srgb(background)},
        "groups": [
            {
                # The glyphs float above the monolith with their own glass and shadow.
                "name": "glyphs",
                "layers": [layer("glyphs")],
                "shadow": {"kind": "neutral", "opacity": 0.5},
                "translucency": {"enabled": True, "value": 0.1},
                "specular": True,
            },
            {
                "name": "monolith",
                "layers": [layer(name) for name, _, _ in LAYERS[1:]],
                "shadow": {"kind": "layer-color", "opacity": 0.5},
                "translucency": {"enabled": False, "value": 0},
                "specular": True,
            },
        ],
        "supported-platforms": {"squares": ["macOS"]},
    }


def write_document(directory: pathlib.Path, primary, background) -> pathlib.Path:
    document = directory / f"{ICON_NAME}.icon"
    if document.exists():
        shutil.rmtree(document)
    (document / "Assets").mkdir(parents=True)
    rows, bbox = _mark_cells()
    for name, chars, color in LAYERS:
        (document / "Assets" / f"{name}.svg").write_text(
            layer_svg(rows, bbox, chars, color(primary)), encoding="utf-8"
        )
    (document / "icon.json").write_text(
        json.dumps(icon_document(background), indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {document.relative_to(ROOT)}")
    return document


def compile_document(actool: pathlib.Path, document: pathlib.Path, directory: pathlib.Path) -> None:
    with tempfile.TemporaryDirectory() as temporary:
        out = pathlib.Path(temporary)
        result = subprocess.run(
            [str(actool), str(document), "--compile", str(out),
             "--platform", "macosx", "--minimum-deployment-target", "11.0",
             "--app-icon", ICON_NAME, "--output-partial-info-plist", str(out / "partial.plist"),
             "--errors", "--warnings", "--output-format", "human-readable-text"],
            capture_output=True, text=True,
        )
        if result.returncode != 0 or "error:" in result.stdout or "warning:" in result.stdout:
            raise ToolError(f"actool failed on {document}:\n{result.stdout}{result.stderr}")
        for name in ("Assets.car", f"{ICON_NAME}.icns"):
            if not (out / name).is_file():
                raise ToolError(f"actool did not produce {name} for {document}")
            shutil.copyfile(out / name, directory / name)
            print(f"wrote {(directory / name).relative_to(ROOT)}")


def export_png(ictool: pathlib.Path, document: pathlib.Path, path: pathlib.Path) -> None:
    subprocess.run(
        [str(ictool), str(document), "--export-image", "--output-file", str(path),
         "--platform", "macOS", "--rendition", "Default",
         "--width", str(CANVAS), "--height", str(CANVAS), "--scale", "1"],
        check=True, stdout=subprocess.DEVNULL,
    )
    if not path.is_file():
        raise ToolError(f"ictool did not export {path}")
    # ictool writes 16 bits per channel; 8 bits looks the same at a fraction of the size.
    from PIL import Image

    with Image.open(path) as image:
        image = image.convert("RGBA")
    image.save(path, optimize=True)
    print(f"wrote {path.relative_to(ROOT)}")
    dock = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    offset = (CANVAS - GRID_BODY) // 2
    dock.alpha_composite(image.resize((GRID_BODY, GRID_BODY), Image.LANCZOS), (offset, offset))
    dock_path = path.with_name("datalith-dock.png")
    dock.save(dock_path, optimize=True)
    print(f"wrote {dock_path.relative_to(ROOT)}")


def main() -> int:
    try:
        ictool, actool = find_tools()
        for channel, primary, background in CHANNELS:
            directory = ROOT / "assets" / "logo" / channel
            directory.mkdir(parents=True, exist_ok=True)
            document = write_document(directory, primary, background)
            compile_document(actool, document, directory)
            export_png(ictool, document, directory / "datalith-macos.png")
    except ToolError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
