"""Writes the colour hint shown with the recommended concept: the mark and the lockup on the
window's own background, in the accent the interface already uses."""

import re
from pathlib import Path

HERE = Path(__file__).parent
CONCEPTS = HERE / "concepts"

BACKGROUND = "#0d1117"
ACCENT = "#00c896"
PAPER = "#e6edf3"
CAP = 72
MARK = 200


def wordmark():
    paths = re.findall(r"<path[^>]*/>", (CONCEPTS / "wordmark.svg").read_text(encoding="utf-8"))
    return "\n    ".join(p.replace('fill="#111111"', f'fill="{PAPER}"') for p in paths)


source = re.search(
    r"<title[^>]*>.*?</title>\s*(.*?)\s*</svg>",
    (CONCEPTS / "a-orbit.svg").read_text(encoding="utf-8"),
    re.S,
).group(1).replace('fill="#111111"', f'fill="{ACCENT}"')

scale = MARK / 256
baseline = 210 + CAP / 2
text = f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 940 420" width="940" height="420">
  <rect width="940" height="420" rx="28" fill="{BACKGROUND}" />
  <g transform="translate(64 110) scale({scale:.4f})">
    {source}
  </g>
  <g fill="{PAPER}" transform="translate(320 {baseline:.0f})">
    {wordmark()}
  </g>
</svg>
"""
out = CONCEPTS / "recommended-colour.svg"
out.write_text(text, encoding="utf-8")
print(f"wrote {out}")
