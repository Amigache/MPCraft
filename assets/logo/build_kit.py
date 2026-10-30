"""Builds the MPCraft mark and its lockups.

Two things are built here, and they are built rather than drawn by hand so that a change to one
number moves every file that depends on it:

* the wordmark, set in Space Grotesk (SIL Open Font License 1.1) and converted to paths, so no
  file here depends on a font being installed where it is opened;
* the mark, the orbit, which is two paths on a 256 grid: the ring and the delta inside it.

Every file is written to `final/`. The colour modes are the set a brand actually needs: the mark
in one ink, the mark reversed for a dark surface, and the two-colour version — the ring in the
accent the window already uses and the aircraft in the other ink.
"""

import argparse
import re
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

HERE = Path(__file__).parent
CONCEPTS = HERE / "concepts"
FINAL = HERE / "final"

# The colours the window already speaks.
INK = "#0D1117"
PAPER = "#E6EDF3"
ACCENT = "#00C896"

# The drawing, on the 256 grid the concept was built on.
RING = "M227.6 136.7 A100 100 0 1 1 119.3 28.4 L123.1 72.2 A56 56 0 1 0 183.8 132.9 Z"
DELTA = "M172 84 L135 172 L84 121 Z"

# Lockup metrics. The gap is measured from the mark's ink, not from its box, which is why the
# number is small: the ring's right edge is 28 units inside the box.
SYMBOL = 160
GAP = 20
CAP = 72

# The stacked lockup: the initials, the air they track, and how far they sit under the mark.
INITIALS_CAP = 44
INITIALS_TRACKING = 8.0
STACK_GAP = 16


def _font(path, weight):
    font = TTFont(path)
    if "fvar" in font:
        font = instantiateVariableFont(font, {"wght": weight})
    return font


def _run(font, text, size, tracking):
    """Returns (path data, advance width) for a run set on its baseline at y=0."""
    upem = font["head"].unitsPerEm  # noqa: F841 - read for clarity of the scale below
    cap = font["OS/2"].sCapHeight
    scale = size / cap
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()
    hmtx = font["hmtx"]

    parts = []
    x = 0.0
    for char in text:
        name = cmap[ord(char)]
        pen = SVGPathPen(glyphs)
        TransformPen(pen, (scale, 0, 0, -scale, x, 0)).addComponent(name, (1, 0, 0, 1, 0, 0))
        parts.append(pen.getCommands())
        x += hmtx[name][0] * scale + tracking
    return "".join(parts), x - tracking


def wordmarks(font_path):
    """Writes the two runs of type the kit uses: the name and the initials."""
    heavy = _font(font_path, 700)

    name, width = _run(heavy, "MPCraft", CAP, -2.0)
    _write(
        FINAL / "wordmark.svg",
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.1f} {CAP}" '
        f'width="{width:.1f}" height="{CAP}" role="img" aria-labelledby="title">\n'
        f'  <title id="title">MPCraft wordmark</title>\n'
        f'  <path fill="{INK}" transform="translate(0 {CAP})" d="{name}" />\n</svg>\n',
    )

    # The initials sit under the mark, tracked out so the three letters read as a badge rather
    # than as the beginning of a word, and set narrower than the mark so the pair reads as one
    # object rather than as a word with a picture above it.
    initials, initials_width = _run(heavy, "MPC", INITIALS_CAP, INITIALS_TRACKING)
    _write(
        FINAL / "wordmark-mpc.svg",
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {initials_width:.1f} '
        f'{INITIALS_CAP}" width="{initials_width:.1f}" height="{INITIALS_CAP}" '
        f'role="img" aria-labelledby="title">\n'
        f'  <title id="title">MPC initials</title>\n'
        f'  <path fill="{INK}" transform="translate(0 {INITIALS_CAP})" d="{initials}" />\n</svg>\n',
    )

    return {
        "name": (name, width),
        "initials": (initials, initials_width, INITIALS_CAP),
    }


def _write(path, text):
    path.write_text(text, encoding="utf-8")
    print(f"wrote {path.relative_to(HERE)}")


def symbol_svg(ring, delta, box=256):
    return (
        f'  <path fill="{ring}" d="{RING}" />\n'
        f'  <path fill="{delta}" d="{DELTA}" />'
    )


def symbol_files():
    # name: (ring, delta, type)
    modes = {
        "ink": (INK, INK, INK),
        "paper": (PAPER, PAPER, PAPER),
        "accent": (ACCENT, INK, INK),
        "accent-reversed": (ACCENT, PAPER, PAPER),
        "inverse": (INK, ACCENT, INK),
    }

    for name, (ring, delta, _) in modes.items():
        _write(
            FINAL / f"symbol-{name}.svg",
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 256 256" width="256" '
            f'height="256" role="img" aria-labelledby="title">\n'
            f'  <title id="title">MPCraft mark</title>\n'
            f"{symbol_svg(ring, delta)}\n</svg>\n",
        )

    return modes


def lockups(type_runs, modes):
    name_path, name_width = type_runs["name"]
    initials_path, initials_width, _ = type_runs["initials"]

    for mode, (ring, delta, ink) in modes.items():
        # The type takes the mode's own ink: a lockup whose letters are a different colour from
        # its mark is two marks pretending to be one.

        # The canvas carries the same air on the right as on the left: the last letter's side
        # bearing is not padding, and a lockup cropped to its ink touches the edge of its box.
        horizontal_width = SYMBOL + GAP + name_width + 13
        _write(
            FINAL / f"lockup-horizontal-{mode}.svg",
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {horizontal_width:.0f} 256" '
            f'width="{horizontal_width:.0f}" height="256" role="img" aria-labelledby="title">\n'
            f'  <title id="title">MPCraft lockup</title>\n'
            f'  <g transform="translate(0 48) scale({SYMBOL / 256:.4f})">\n'
            f"{symbol_svg(ring, delta)}\n"
            f"  </g>\n"
            f'  <g transform="translate({SYMBOL + GAP} {128 + CAP / 2})">\n'
            f'    <path fill="{ink}" d="{name_path}" />\n'
            f"  </g>\n</svg>\n",
        )

        # Stacked: the mark, then the initials beneath it. The width is the mark's, unless the
        # initials are wider, which they are not at this size.
        stacked_width = max(SYMBOL, initials_width)
        # The air below the initials matches the air beside the mark. A box cut at the baseline
        # clips the overshoot of the round letters by a unit, and a box with three units under it
        # looks like the type fell out of the bottom.
        stacked_height = SYMBOL + STACK_GAP + INITIALS_CAP + 16
        _write(
            FINAL / f"lockup-stacked-{mode}.svg",
            f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {stacked_width:.0f} '
            f'{stacked_height:.0f}" width="{stacked_width:.0f}" height="{stacked_height:.0f}" '
            f'role="img" aria-labelledby="title">\n'
            f'  <title id="title">MPCraft stacked lockup</title>\n'
            f'  <g transform="translate({(stacked_width - SYMBOL) / 2:.1f} 0) '
            f'scale({SYMBOL / 256:.4f})">\n'
            f"{symbol_svg(ring, delta)}\n"
            f"  </g>\n"
            f'  <g transform="translate({(stacked_width - initials_width) / 2:.1f} '
            f'{SYMBOL + STACK_GAP + INITIALS_CAP})">\n'
            f'    <path fill="{ink}" d="{initials_path}" />\n'
            f"  </g>\n</svg>\n",
        )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--font", required=True, help="Space Grotesk, variable or static")
    args = parser.parse_args()

    FINAL.mkdir(exist_ok=True)
    modes = symbol_files()
    lockups(wordmarks(args.font), modes)


if __name__ == "__main__":
    main()
