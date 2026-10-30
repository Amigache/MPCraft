# The MPCraft logo

The orbit: a ring that opens where the aircraft leaves it. The aircraft is the delta inside the
ring; the ring is the mission that holds it. It was chosen from three concepts, and the other two
are kept beside it in `concepts/` because they are what the choice was made against.

## What is here

| Path | What it is |
| --- | --- |
| `build_kit.py` | Writes every SVG in `final/`. The mark is two paths on a 256 grid; the wordmark is type converted to paths. |
| `check_kit.py` | Measures the ink of each lockup inside its canvas, so a change to a metric shows as a different margin. |
| `final/symbol-*.svg` | The mark alone, in the five colour modes. |
| `final/lockup-horizontal-*.svg` | The mark beside the name. |
| `final/lockup-stacked-*.svg` | The mark above the initials, `MPC`. |
| `final/wordmark.svg`, `final/wordmark-mpc.svg` | The two runs of type on their own. |
| `final/variants.png` | The sheet the direction was read on. |
| `concepts/` | The three marks of the first round, in one colour. |

## Colour modes

| Mode | Ring | Aircraft | Type | For |
| --- | --- | --- | --- | --- |
| `ink` | `#0D1117` | `#0D1117` | `#0D1117` | paper, one colour |
| `paper` | `#E6EDF3` | `#E6EDF3` | `#E6EDF3` | dark surfaces, one colour |
| `accent` | `#00C896` | `#0D1117` | `#0D1117` | paper, two colours |
| `accent-reversed` | `#00C896` | `#E6EDF3` | `#E6EDF3` | the window, two colours |
| `inverse` | `#0D1117` | `#00C896` | `#0D1117` | paper, the accent on the aircraft |

The accent is the one the window already uses (`--colour-accent`), so the mark and the interface
speak the same colour rather than two greens that are almost the same.

The mode the project uses is **`accent-reversed`** on the window and **`accent`** on paper: the ring
in the interface's accent, the aircraft in the ink of the surface it sits on.

## Building it

The wordmark is set in **Space Grotesk**, released under the SIL Open Font License 1.1, which
allows a logo to be made from it and its outlines to be shipped. The font file is not in this
repository: pass it to the builder.

```sh
python assets/logo/build_kit.py --font path/to/SpaceGrotesk.ttf
python assets/logo/check_kit.py --font path/to/SpaceGrotesk.ttf
```

Space Grotesk is published at <https://github.com/floriankarsten/space-grotesk>.

## What is not done yet

- The favicon and the app icons for each platform are built from `symbol-*.svg` when the colour
  mode is decided; `export_variants.py` from the logo-design skill writes that set.
- The mark has not been searched against the trademark registers. That search is the operator's,
  and no designer can promise clearance.
