"""Composes the horizontal lockup: the mark at the left, the outlined wordmark at the right.

The wordmark comes from `wordmark.svg`, which `build_wordmark.py` writes from Space Grotesk (SIL
Open Font License 1.1). Nothing here reads a font: the letters are already paths, so the lockup
renders the same on a machine that has never seen the typeface.
"""

import re
import sys
from pathlib import Path

HERE = Path(__file__).parent
CONCEPTS = HERE / "concepts"

HEIGHT = 256
SYMBOL = 160  # the mark's box, about 2x the wordmark's cap height
GAP = 48
CAP = 72


def wordmark():
    source = (CONCEPTS / "wordmark.svg").read_text(encoding="utf-8")
    paths = re.findall(r"<path[^>]*/>", source)
    return "\n    ".join(paths)


def symbol(path):
    """The body of a concept file, without its svg wrapper and title."""
    text = path.read_text(encoding="utf-8")
    body = re.search(r"<title[^>]*>.*?</title>\s*(.*?)\s*</svg>", text, re.S).group(1)
    return body.strip()


def build(name):
    source = symbol(CONCEPTS / f"{name}.svg")
    scale = SYMBOL / 256
    offset_y = (HEIGHT - SYMBOL) / 2
    baseline = HEIGHT / 2 + CAP / 2
    transform = f"translate(0 {baseline:.1f})"

    width = SYMBOL + GAP + 402.7
    text = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0f} {HEIGHT}" width="{width:.0f}" height="{HEIGHT}" role="img" aria-labelledby="title">
  <title id="title">MPCraft â€” {name} lockup</title>
  <g id="symbol" transform="translate(0 {offset_y:.0f}) scale({scale:.4f})">
    {source}
  </g>
  <g id="wordmark" fill="#111111" transform="{transform}">
    <g transform="translate({SYMBOL + GAP} 0)">
    {wordmark()}
    </g>
  </g>
</svg>
"""
    out = CONCEPTS / f"{name}-lockup.svg"
    out.write_text(text, encoding="utf-8")
    print(f"wrote {out.name}  {width:.0f} x {HEIGHT}")


if __name__ == "__main__":
    for name in sys.argv[1:]:
        build(name)
