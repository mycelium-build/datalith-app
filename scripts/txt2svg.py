"""Convert Datalith pixel-art .txt icons to SVGs.

Pixel format (one row per line):
  x  solid pixel
  s  half-transparent pixel (50% alpha)
  space  empty pixel

Leading spaces are meaningful (they position the art in the grid). The grid is
always 7×7: rows are padded with trailing spaces and any row wider than 7 (or
more than 7 rows) is an error.

Usage:
  uv run scripts/txt2svg.py <path> [--output <dir>]

  <path>  a single .txt icon, or a folder (every .txt inside is converted).
  --output <dir>  write generated .svg files here instead of next to the input.
"""

import argparse
import pathlib
import sys

SOLID = "x"
HALF = "s"


def parse_text(path: pathlib.Path) -> tuple[int, int, list[str], list[str]]:
    """Return (width, height, solid_rows, half_rows) for one .txt icon.

    Rows keep their trailing spaces so the 7×7 canvas is preserved; the grid is
    always 7×7 and any row wider than 7 (or more than 7 rows) is an error.
    `*_rows` hold the source line for each row so the caller can map characters
    back to coordinates.
    """
    lines = path.read_text(encoding="utf-8").splitlines()
    rows = list(lines)
    if not rows or all(not row.strip() for row in rows):
        raise ValueError(f"{path.name}: icon is empty")

    width = max((len(row) for row in rows), default=0)
    height = len(rows)
    if width > 7 or height > 7:
        raise ValueError(
            f"{path.name}: grid is {width}x{height}, expected at most 7x7"
        )

    # Pad to a uniform 7×7 canvas.
    rows = [row.ljust(7) for row in rows]
    while len(rows) < 7:
        rows.append("       ")

    solid_rows: list[str] = []
    half_rows: list[str] = []
    for row_index, row in enumerate(rows, start=1):
        for col_index, char in enumerate(row):
            if char == SOLID:
                solid_rows.append(f"{row_index}:{col_index}")
            elif char == HALF:
                half_rows.append(f"{row_index}:{col_index}")
            elif char != " ":
                raise ValueError(
                    f"{path.name}:{row_index}:{col_index + 1}: "
                    f"unexpected character {char!r} (expected {SOLID!r}, {HALF!r} or space)"
                )

    return width, height, solid_rows, half_rows


def to_svg(path: pathlib.Path) -> str:
    _width, _height, solid_rows, half_rows = parse_text(path)

    rects: list[str] = []
    for cell in solid_rows:
        row, col = (int(part) for part in cell.split(":"))
        rects.append(
            f'<rect x="{col}" y="{row - 1}" width="1" height="1"/>'
        )
    for cell in half_rows:
        row, col = (int(part) for part in cell.split(":"))
        rects.append(
            f'<rect x="{col}" y="{row - 1}" width="1" height="1" opacity="0.25"/>'
        )

    return (
        '<svg xmlns="http://www.w3.org/2000/svg" '
        'width="7" height="7" viewBox="0 0 7 7" '
        'fill="currentColor" shape-rendering="crispEdges">'
        f'{"".join(rects)}'
        "</svg>"
    )


def convert_file(source: pathlib.Path, output_dir: pathlib.Path) -> pathlib.Path:
    destination = output_dir / f"{source.stem}.svg"
    destination.write_text(to_svg(source), encoding="utf-8")
    return destination


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Convert Datalith pixel-art .txt icons to SVGs."
    )
    parser.add_argument("path", type=pathlib.Path, help="a .txt icon or a folder of icons")
    parser.add_argument(
        "--output",
        type=pathlib.Path,
        help="directory to write .svg files to (default: next to each input)",
    )
    args = parser.parse_args()

    sources = (
        [args.path]
        if args.path.is_file()
        else sorted(args.path.glob("*.txt")) if args.path.is_dir() else []
    )
    if not sources:
        print(f"error: no .txt icons found at {args.path}", file=sys.stderr)
        return 1

    failures = 0
    for source in sources:
        output_dir = args.output or source.parent
        try:
            output_dir.mkdir(parents=True, exist_ok=True)
            destination = convert_file(source, output_dir)
        except (ValueError, OSError) as error:
            failures += 1
            print(f"error: {error}", file=sys.stderr)
            continue
        print(f"{source.name} -> {destination.name}")

    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
