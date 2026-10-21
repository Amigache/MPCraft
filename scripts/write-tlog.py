"""Writes a telemetry log, to read it with MPCraft.

The log is the stream of the frames that crossed the link, each of them with the
instant it was seen: eight bytes of microseconds since the epoch, written the other
way round, and then the frame as it went over the wire.
"""

import io
import math
import os
import sys

sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

import importlib.util  # noqa: E402

_spec = importlib.util.spec_from_file_location(
    "simulated_vehicle",
    os.path.join(
        os.path.abspath(os.path.dirname(__file__)),
        "simulated-vehicle.py",
    ),
)
vehicle = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(vehicle)


def build(path):
    """Writes a log of a short flight."""

    messages = vehicle.MESSAGES

    def frame_of(message_id, values, sequence):
        payload = vehicle.encode(message_id, values)
        return vehicle.frame(message_id, payload, sequence)

    records = []

    for step in range(400):
        moment = 1_700_000_000_000_000 + step * 100_000
        angle = step * 0.05

        records.append(
            (
                moment,
                frame_of(
                    30,
                    {
                        "time_boot_ms": step * 100,
                        "roll": 0.1 * math.sin(angle),
                        "pitch": 0.05 * math.cos(angle),
                        "yaw": angle % 6.28,
                        "rollspeed": 0.0,
                        "pitchspeed": 0.0,
                        "yawspeed": 0.05,
                    },
                    step,
                ),
            )
        )

        records.append(
            (
                moment + 10_000,
                frame_of(
                    24,
                    {
                        "time_us": step * 100_000,
                        "fix_type": 3,
                        "lat": -351680000,
                        "lon": 1491300000 + step * 100,
                        "alt": 584000 + step * 100,
                        "eph": 100,
                        "epv": 100,
                        "vel": 500,
                        "cog": 9000,
                        "satellites_visible": 14,
                    },
                    step,
                ),
            )
        )

        records.append(
            (
                moment + 20_000,
                frame_of(
                    74,
                    {
                        "airspeed": 12.5,
                        "groundspeed": 11.0,
                        "heading": int(angle * 1000) % 36000,
                        "throttle": 40,
                        "alt": 584.0 + step * 0.1,
                        "climb": 0.1,
                    },
                    step,
                ),
            )
        )

    with io.open(path, "wb") as handle:
        for moment, frame in records:
            handle.write(moment.to_bytes(8, "big"))
            handle.write(frame)

    return len(records)


if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "flight.tlog"
    count = build(target)
    print(f"{count} records written to {target} ({os.path.getsize(target)} bytes)")
