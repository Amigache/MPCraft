"""Measures the kit: where the ink of every lockup sits inside its canvas.

A lockup is judged by its margins, not by its path data: a canvas cropped to the ink touches the
edge of its box, and one with uneven air around the mark looks off-centre in a slide even when the
geometry is exact. This is the check the sheet is read with, and it is the same arithmetic the
builder uses, so a change to a metric shows up as a different margin rather than as a surprise.
"""

import re
import sys
from pathlib import Path

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.transformPen import TransformPen

from build_kit import (
    CAP,
    GAP,
    INITIALS_CAP,
    INITIALS_TRACKING,
    STACK_GAP,
    SYMBOL,
    _font,
)

FINAL = Path(__file__).parent / "final"


def run_bounds(text, size, tracking):
    """The ink of a run of type: its box and the width it advances."""
    font = _font(sys.argv[1], 700)
    scale = size / font["OS/2"].sCapHeight
    glyphs = font.getGlyphSet()
    cmap = font.getBestCmap()
    hmtx = font["hmtx"]

    x = 0.0
    lo = [1e9, 1e9]
    hi = [-1e9, -1e9]
    for char in text:
        name = cmap[ord(char)]
        pen = BoundsPen(glyphs)
        TransformPen(pen, (scale, 0, 0, -scale, x, 0)).addComponent(name, (1, 0, 0, 1, 0, 0))
        x0, y0, x1, y1 = pen.bounds
        lo = [min(lo[0], x0), min(lo[1], y0)]
        hi = [max(hi[0], x1), max(hi[1], y1)]
        x += hmtx[name][0] * scale + tracking
    return lo, hi, x - tracking


def report():
    name_lo, name_hi, name_advance = run_bounds("MPCraft", CAP, -2.0)
    initials_lo, initials_hi, initials_advance = run_bounds("MPC", INITIALS_CAP, INITIALS_TRACKING)

    # The mark's ink inside its 256 box: the ring's own bounds.
    ring = (28, 28, 227.6, 228)
    scale = SYMBOL / 256

    for path in sorted(FINAL.glob("lockup-*.svg")):
        match = re.search(r'viewBox="0 0 ([\d.]+) ([\d.]+)"', path.read_text(encoding="utf-8"))
        width, height = float(match.group(1)), float(match.group(2))

        if "horizontal" in path.name:
            symbol = (ring[0] * scale, 48 + ring[1] * scale, ring[2] * scale, 48 + ring[3] * scale)
            type_lo = (SYMBOL + GAP + name_lo[0], 128 + CAP / 2 + name_lo[1])
            type_hi = (SYMBOL + GAP + name_hi[0], 128 + CAP / 2 + name_hi[1])
        else:
            offset = (width - SYMBOL) / 2
            symbol = (
                offset + ring[0] * scale,
                ring[1] * scale,
                offset + ring[2] * scale,
                ring[3] * scale,
            )
            shift = (width - initials_advance) / 2
            baseline = SYMBOL + STACK_GAP + INITIALS_CAP
            type_lo = (shift + initials_lo[0], baseline + initials_lo[1])
            type_hi = (shift + initials_hi[0], baseline + initials_hi[1])

        x0 = min(symbol[0], type_lo[0])
        y0 = min(symbol[1], type_lo[1])
        x1 = max(symbol[2], type_hi[0])
        y1 = max(symbol[3], type_hi[1])

        print(
            f"{path.name:40s} {width:5.0f}x{height:5.0f}  "
            f"margins L{x0:4.0f} R{width - x1:4.0f} T{y0:4.0f} B{height - y1:4.0f}"
        )


if __name__ == "__main__":
    report()
