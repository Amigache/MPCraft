"""Builds the outlined wordmark used in the MPCraft lockups.

The type is Space Grotesk, released under the SIL Open Font License 1.1, so a logo may be
made from it and the outlines may be shipped. The file is read from the path given on the
command line; nothing here downloads anything.

Two weights are set side by side: "MP" heavier than "Craft", so the line reads as one name
with two tempos rather than as two words. The letters are converted to paths, so the lockup
does not depend on a font being installed anywhere.
"""

import argparse
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont


def load(path, weight):
    font = TTFont(path)
    if "fvar" in font:
        font = instantiateVariableFont(font, {"wght": weight})
    return font


def run(font, text, size, tracking):
    """Returns (path data, advance width) for a run of text set on its baseline at y=0."""
    upem = font["head"].unitsPerEm
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
        # SVG y grows downwards; the font's y grows upwards, and the baseline sits at 0.
        TransformPen(pen, (scale, 0, 0, -scale, x, 0)).addComponent(name, (1, 0, 0, 1, 0, 0))
        parts.append(pen.getCommands())
        x += hmtx[name][0] * scale + tracking
    return "".join(parts), x - tracking, upem


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--font", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--cap", type=float, default=72, help="cap height in the lockup's units")
    parser.add_argument("--tracking", type=float, default=-1.5)
    args = parser.parse_args()

    heavy = load(args.font, 700)
    light = load(args.font, 400)
    mp, mp_width, _ = run(heavy, "MP", args.cap, args.tracking)
    craft, craft_width, _ = run(light, "Craft", args.cap, args.tracking * 0.6)
    gap = args.tracking * 1.2
    width = mp_width + gap + craft_width

    Path(args.out).write_text(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.1f} {args.cap:.1f}" '
        f'width="{width:.1f}" height="{args.cap:.1f}" role="img" aria-labelledby="title">\n'
        f'  <title id="title">MPCraft wordmark</title>\n'
        f'  <g id="wordmark" fill="#111111" transform="translate(0 {-args.cap:.1f})">\n'
        f'    <path d="{mp}" />\n'
        f'    <path transform="translate({mp_width + gap:.1f} 0)" d="{craft}" />\n'
        f"  </g>\n</svg>\n",
        encoding="utf-8",
    )
    print(f"wrote {args.out}  {width:.1f} x {args.cap:.1f}")


if __name__ == "__main__":
    main()
