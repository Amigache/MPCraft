"""Writes a log of DataFlash, to read it with MPCraft.

The log is the text one: a declaration of every message at the top and a line per
sample after it.
"""

import io
import math
import os
import sys

MESSAGES = [
    (128, 89, "FMT", "BBnNZ", ["Type", "Length", "Name", "Format"]),
    (130, 45, "GPS", "QILLEe", ["TimeUS", "Status", "Lat", "Lng", "Alt", "Spd"]),
    (131, 30, "ATT", "Qffffff", ["TimeUS", "Roll", "Pitch", "Yaw", "RollSpd", "PitchSpd", "YawSpd"]),
    (132, 20, "MODE", "QI", ["TimeUS", "Mode"]),
]


def build(path):
    """Writes a log of a short flight."""

    lines = []

    for identifier, length, name, types, fields in MESSAGES:
        lines.append(
            "FMT, %d, %d, %s, %s, %s" % (identifier, length, name, types, ",".join(fields))
        )

    for step in range(400):
        moment = 1_000_000 + step * 100_000
        angle = step * 0.05

        lines.append(
            "GPS, %d, 3, %d, %d, %d, %d"
            % (
                moment,
                int(-35.3547178 * 1e7),
                int(149.1696673 * 1e7),
                int((585.0 + step * 0.1) * 100),
                int(245 + 5 * math.sin(angle)),
            )
        )

        lines.append(
            "ATT, %d, %.6f, %.6f, %.6f, 0.0, 0.0, %.6f"
            % (moment, 0.1 * math.sin(angle), 0.05 * math.cos(angle), angle % 6.28, 0.05)
        )

        lines.append("MODE, %d, 5" % moment)

    with io.open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")

    return len(lines)


if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "flight.log"
    count = build(target)
    print(
        "%d lines written to %s (%d bytes)"
        % (count, target, os.path.getsize(target))
    )
