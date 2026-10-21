import io
import math
import os
import struct
import sys

# How many bytes a value of each type takes.
SIZES = {
    "B": 1,
    "b": 1,
    "H": 2,
    "h": 2,
    "I": 4,
    "i": 4,
    "L": 4,
    "e": 4,
    "f": 4,
    "d": 8,
    "Q": 8,
    "N": 1,
    "Z": 16,
    "M": 1,
    "A": 128,
}

# The messages of the log, with the type of each of their values.
MESSAGES = [
    (128, "FMT", "BBnNZ", ["Type", "Length", "Name", "Format"]),
    (130, "GPS", "QILLEe", ["TimeUS", "Status", "Lat", "Lng", "Alt", "Spd"]),
    (
        131,
        "ATT",
        "Qffffff",
        ["TimeUS", "Roll", "Pitch", "Yaw", "RollSpd", "PitchSpd", "YawSpd"],
    ),
    (132, "MODE", "QI", ["TimeUS", "Mode"]),
]

# The values of the messages, in the order the declaration gives them.
SAMPLES = [
    (
        "GPS",
        [
            ("Q", lambda step, angle: 1_000_000 + step * 100_000),
            ("I", lambda step, angle: 3),
            ("L", lambda step, angle: int(-35.3547178 * 1e7)),
            ("L", lambda step, angle: int(149.1696673 * 1e7)),
            ("e", lambda step, angle: int((585.0 + step * 0.1) * 100)),
            ("e", lambda step, angle: int(245 + 5 * math.sin(angle))),
        ],
    ),
    (
        "ATT",
        [
            ("Q", lambda step, angle: 1_000_000 + step * 100_000),
            ("f", lambda step, angle: 0.1 * math.sin(angle)),
            ("f", lambda step, angle: 0.05 * math.cos(angle)),
            ("f", lambda step, angle: angle % 6.28),
            ("f", lambda step, angle: 0.0),
            ("f", lambda step, angle: 0.0),
            ("f", lambda step, angle: 0.05),
        ],
    ),
    (
        "MODE",
        [
            ("Q", lambda step, angle: 1_000_000 + step * 100_000),
            ("I", lambda step, angle: 5),
        ],
    ),
]

# How a value of each type is packed.
PACK = {
    "B": "<B",
    "b": "<b",
    "H": "<H",
    "h": "<h",
    "I": "<I",
    "i": "<i",
    "L": "<Q",
    "Q": "<Q",
    "e": "<i",
    "f": "<f",
    "d": "<d",
}


def build_text(path):
    """Writes a log of a short flight, in the text format."""

    lines = []

    for identifier, name, types, fields in MESSAGES:
        length = 3 + sum(SIZES.get(character, 1) for character in types)
        lines.append(
            "FMT, %d, %d, %s, %s, %s" % (identifier, length, name, types, ",".join(fields))
        )

    for step in range(400):
        angle = step * 0.05

        for name, values in SAMPLES:
            parts = [name]

            for kind, value in values:
                number = value(step, angle)

                if kind == "L":
                    parts.append(str(number))
                elif kind == "e":
                    parts.append(str(number))
                else:
                    parts.append(repr(number) if kind == "f" else str(int(number)))

            lines.append(", ".join(parts))

    with io.open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write("\n".join(lines) + "\n")

    return len(lines)


def build_binary(path):
    """Writes a log of a short flight, in the binary format."""

    with io.open(path, "wb") as handle:
        for identifier, name, types, fields in MESSAGES:
            length = 3 + sum(SIZES.get(character, 1) for character in types)
            payload = bytearray()
            payload.append(identifier)
            payload.append(length)
            payload.extend(name.encode("ascii")[:4].ljust(4, b"\x00"))
            payload.extend(types.encode("ascii")[:16].ljust(16, b"\x00"))
            payload.extend(",".join(fields).encode("ascii")[:64].ljust(64, b"\x00"))
            handle.write(bytes([0xA3, 0x95, 0x80]))
            handle.write(bytes(payload))

        for step in range(400):
            angle = step * 0.05

            for name, values in SAMPLES:
                kind = next(
                    identifier for identifier, other, _, _ in MESSAGES if other == name
                )
                payload = bytearray()

                for letter, value in values:
                    number = value(step, angle)

                    if letter == "L":
                        payload.extend(struct.pack("<i", int(number)))
                    elif letter == "e":
                        payload.extend(struct.pack("<i", int(number)))
                    else:
                        payload.extend(struct.pack(PACK[letter], number))

                handle.write(bytes([0xA3, 0x95, kind]))
                handle.write(bytes(payload))

    return 400 * len(SAMPLES)


if __name__ == "__main__":
    binary = "--binary" in sys.argv
    target = next((arg for arg in sys.argv[1:] if not arg.startswith("--")), None)
    target = target or ("flight.bin" if binary else "flight.log")

    count = build_binary(target) if binary else build_text(target)

    print(
        "%d records written to %s (%d bytes)"
        % (count, target, os.path.getsize(target))
    )
