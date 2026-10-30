"""Writes the app icons from the mark.

The icon is the mark on the window's own ground: a rounded tile of `#0D1117` with the ring in
the accent and the aircraft in paper, which is the `accent-reversed` mode the project chose. The
tile is what makes the mark an app icon rather than a drawing: it keeps the shape of the mark
away from the edge of the dock, the taskbar and the installer.

The sizes are the set Tauri and the three platforms ask for, and they are rendered by Chrome at
the size they are written, not scaled down from one big picture: a mark that is resampled from
1024 px is a soft mark at 16 px, and the ring is the first thing to go.

The `.ico` and the `.icns` are built here rather than by a tool of one platform: the first is a
directory of PNGs, which Pillow writes, and the second is the same idea with Apple's four-letter
codes. Neither needs macOS.
"""

import struct
import subprocess
import sys
from pathlib import Path

import build_kit

HERE = Path(__file__).parent
ICONS = HERE.parent.parent / "apps" / "desktop" / "icons"
PUBLIC = HERE.parent.parent / "apps" / "ui" / "public"

CHROME = Path(r"C:\Program Files\Google\Chrome\Application\chrome.exe")

# The mark's box inside the tile, as a share of the tile. The ink is 200 of the box's 256 units,
# so this puts the drawing at about 61 % of the tile — the air an app icon is read with.
MARK = 0.78
RADIUS = 0.225


def tile(size: int, accent: str, paper: str) -> str:
    box = size * MARK
    offset = (size - box) / 2
    scale = box / 256

    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" width="{size}" height="{size}">
  <rect width="{size}" height="{size}" rx="{size * RADIUS:.1f}" fill="{build_kit.INK}" />
  <g transform="translate({offset:.1f} {offset:.1f}) scale({scale:.6f})">
    <path fill="{accent}" d="{build_kit.RING}" />
    <path fill="{paper}" d="{build_kit.DELTA}" />
  </g>
</svg>
"""


def render(svg: str, size: int, out: Path):
    """Renders one tile at one size, with the corners transparent."""
    source = out.parent / f".icon-{size}.svg"
    source.write_text(svg, encoding="utf-8")

    subprocess.run(
        [
            str(CHROME),
            "--headless",
            "--disable-gpu",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            "--default-background-color=00000000",
            f"--window-size={size},{size}",
            f"--screenshot={out}",
            source.resolve().as_uri(),
        ],
        check=True,
        capture_output=True,
        timeout=120,
    )
    source.unlink(missing_ok=True)


def windows_icon(sizes, out: Path):
    """A `.ico` that carries every size it was given, rather than one that is scaled by the shell.

    Each entry holds the PNG that was rendered at that size: a shell that asks for 32 px gets the
    32 px drawing, not a 256 px one shrunk by whatever filter the shell happens to use.
    """
    header = struct.pack("<HHH", 0, 1, len(sizes))
    offset = len(header) + 16 * len(sizes)
    entries = b""
    data = b""

    for size in sizes:
        png = (out.parent / f".icon-{size}.png").read_bytes()
        dimension = 0 if size >= 256 else size
        entries += struct.pack(
            "<BBBBHHII",
            dimension,
            dimension,
            0,
            0,
            1,
            32,
            len(png),
            offset + len(data),
        )
        data += png

    out.write_bytes(header + entries + data)


def macos_icon(sizes, out: Path):
    """An `.icns`: a header and a list of PNGs, each with the code that names its size."""
    codes = {
        16: b"icp4",
        32: b"icp5",
        64: b"icp6",
        128: b"ic07",
        256: b"ic08",
        512: b"ic09",
        1024: b"ic10",
    }

    entries = b""
    for size in sizes:
        code = codes[size]
        data = (out.parent / f".icon-{size}.png").read_bytes()
        entries += code + struct.pack(">I", len(data) + 8) + data

    out.write_bytes(b"icns" + struct.pack(">I", len(entries) + 8) + entries)


def main():
    if not CHROME.exists():
        print(f"chrome is not at {CHROME}", file=sys.stderr)
        return 1

    ICONS.mkdir(parents=True, exist_ok=True)
    PUBLIC.mkdir(parents=True, exist_ok=True)

    # The tile is written once per size, because the ring is a stroke of geometry: rendering it
    # at the size it is used keeps its thickness the same fraction of the icon.
    wanted = [16, 32, 64, 128, 256, 512, 1024]
    for size in wanted:
        render(tile(size, build_kit.ACCENT, build_kit.PAPER), size, ICONS / f".icon-{size}.png")

    # The names Tauri reads.
    for name, size in {
        "16x16.png": 16,
        "32x32.png": 32,
        "64x64.png": 64,
        "128x128.png": 128,
        "128x128@2x.png": 256,
        "icon.png": 512,
        "1024x1024.png": 1024,
    }.items():
        (ICONS / name).write_bytes((ICONS / f".icon-{size}.png").read_bytes())

    # The `.ico` carries 256 and 48 as well, which Windows asks for and the list above does not.
    render(tile(48, build_kit.ACCENT, build_kit.PAPER), 48, ICONS / ".icon-48.png")
    windows_icon([16, 32, 48, 64, 128, 256], ICONS / "icon.ico")
    macos_icon([16, 32, 64, 128, 256, 512, 1024], ICONS / "icon.icns")

    # The page of the application, when it is opened in a browser.
    (PUBLIC / "favicon.svg").write_text(
        tile(64, build_kit.ACCENT, build_kit.PAPER), encoding="utf-8"
    )

    for leftover in ICONS.glob(".icon-*.png"):
        leftover.unlink()
    for leftover in ICONS.glob(".ico-*.png"):
        leftover.unlink()
    for leftover in ICONS.glob(".icns-*.png"):
        leftover.unlink()

    print(f"wrote the icons into {ICONS}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
