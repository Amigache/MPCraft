#!/usr/bin/env python3
"""Generate a MAVLink test stream, independently of the Rust implementation.

This script is a second, independent implementation of the MAVLink wire format. It exists
so that the Rust parser is checked against something other than itself: when the two
disagree, one of them has a bug in the framing, the CRC or the header layout.

The generated file is committed as a test fixture. Regenerate it with:

    python scripts/generate-test-stream.py crates/mpcraft-mavlink/tests/fixtures/stream.tlog

The layout follows the MAVLink 2 specification:

    0      1      2      3      4      5      6      7 8 9
    STX    len    incomp compat seq    sysid  compid msgid
    10..        payload
    ..+2        crc16 (X.25, with the message's CRC_EXTRA seed)
"""

import math
import struct
import sys

STX = 0xFD

TYPES = {
    "float": 4,
    "double": 8,
    "char": 1,
    "int8_t": 1,
    "uint8_t": 1,
    "int16_t": 2,
    "uint16_t": 2,
    "int32_t": 4,
    "uint32_t": 4,
    "int64_t": 8,
    "uint64_t": 8,
}


def field_seed(name, fields):
    """The CRC_EXTRA seed, worked out from the message's own field definitions.

    The rule is the CRC-16/MCRF4XX over the message name, then each *base* field in wire
    order — sorted by type length descending, ties keeping the definition order — as
    `type name `, with the array length appended when the field is an array. The result is
    folded to a byte.

    Extensions are left out on purpose: MAVLink 2 puts them after the base fields, and a
    seed that counted them would change whenever a dialect added a field, which is exactly
    what the seed must not do.
    """
    base = [field for field in fields if not field[3]]
    base = sorted(base, key=lambda field: -TYPES[field[0]])

    data = (name + " ").encode()
    for type_name, field_name, array_length, _extension in base:
        data += (type_name + " " + field_name + " ").encode()
        if array_length:
            data += bytes([array_length])

    crc = crc16(data)
    return (crc & 0xFF) ^ (crc >> 8)


def crc16(data):
    """CRC-16/MCRF4XX, the checksum MAVLink uses."""
    crc = 0xFFFF

    for byte in data:
        tmp = (byte ^ (crc & 0xFF)) & 0xFF
        tmp = (tmp ^ (tmp << 4)) & 0xFF
        crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4)) & 0xFFFF

    return crc


# The messages the fixture carries, with their fields in definition order: type, name,
# array length, and whether the field is an extension.
MESSAGES = {
    0: ("HEARTBEAT", [
        ("uint8_t", "type", 0, False),
        ("uint8_t", "autopilot", 0, False),
        ("uint8_t", "base_mode", 0, False),
        ("uint32_t", "custom_mode", 0, False),
        ("uint8_t", "system_status", 0, False),
        ("uint8_t", "mavlink_version", 0, False),
    ]),
    1: ("SYS_STATUS", [
        ("uint32_t", "onboard_control_sensors_present", 0, False),
        ("uint32_t", "onboard_control_sensors_enabled", 0, False),
        ("uint32_t", "onboard_control_sensors_health", 0, False),
        ("uint16_t", "load", 0, False),
        ("uint16_t", "voltage_battery", 0, False),
        ("int16_t", "current_battery", 0, False),
        ("int8_t", "battery_remaining", 0, False),
        ("uint16_t", "drop_rate_comm", 0, False),
        ("uint16_t", "errors_comm", 0, False),
        ("uint16_t", "errors_count1", 0, False),
        ("uint16_t", "errors_count2", 0, False),
        ("uint16_t", "errors_count3", 0, False),
        ("uint16_t", "errors_count4", 0, False),
        ("uint32_t", "onboard_control_sensors_present_extended", 0, True),
        ("uint32_t", "onboard_control_sensors_enabled_extended", 0, True),
        ("uint32_t", "onboard_control_sensors_health_extended", 0, True),
    ]),
    30: ("ATTITUDE", [
        ("uint32_t", "time_boot_ms", 0, False),
        ("float", "roll", 0, False),
        ("float", "pitch", 0, False),
        ("float", "yaw", 0, False),
        ("float", "rollspeed", 0, False),
        ("float", "pitchspeed", 0, False),
        ("float", "yawspeed", 0, False),
    ]),
    33: ("GLOBAL_POSITION_INT", [
        ("uint32_t", "time_boot_ms", 0, False),
        ("int32_t", "lat", 0, False),
        ("int32_t", "lon", 0, False),
        ("int32_t", "alt", 0, False),
        ("int32_t", "relative_alt", 0, False),
        ("int16_t", "vx", 0, False),
        ("int16_t", "vy", 0, False),
        ("int16_t", "vz", 0, False),
        ("uint16_t", "hdg", 0, False),
    ]),
    82: ("SET_ATTITUDE_TARGET", [
        ("uint32_t", "time_boot_ms", 0, False),
        ("uint8_t", "target_system", 0, False),
        ("uint8_t", "target_component", 0, False),
        ("uint8_t", "type_mask", 0, False),
        ("float", "q", 4, False),
        ("float", "body_roll_rate", 0, False),
        ("float", "body_pitch_rate", 0, False),
        ("float", "body_yaw_rate", 0, False),
        ("float", "thrust", 0, False),
        # The 3D thrust setpoint is an extension, so it is not part of the seed.
        ("float", "thrust_body", 3, True),
    ]),
    242: ("HOME_POSITION", [
        ("int32_t", "latitude", 0, False),
        ("int32_t", "longitude", 0, False),
        ("int32_t", "altitude", 0, False),
        ("float", "x", 0, False),
        ("float", "y", 0, False),
        ("float", "z", 0, False),
        ("float", "q", 4, False),
        ("float", "approach_x", 0, False),
        ("float", "approach_y", 0, False),
        ("float", "approach_z", 0, False),
        # The timestamp is an extension, so it is not part of the seed.
        ("uint64_t", "time_usec", 0, True),
    ]),
}

# The seeds the dialect publishes for these messages. They are kept as a control: the rule
# above has to reproduce every one of them before its green is trusted, and the seed of a
# message that is not in this table — SET_ATTITUDE_TARGET — comes only from the rule.
SEEDS_FROM_DIALECT = {
    0: 50,    # HEARTBEAT
    1: 124,   # SYS_STATUS
    30: 39,   # ATTITUDE
    33: 104,  # GLOBAL_POSITION_INT
    242: 104, # HOME_POSITION
}

CRC_EXTRA = {message_id: field_seed(name, fields) for message_id, (name, fields) in MESSAGES.items()}


def check_seeds():
    """Proves the rule against the values taken from the dialect before anything is written."""
    for message_id, known in SEEDS_FROM_DIALECT.items():
        name, fields = MESSAGES[message_id]
        got = field_seed(name, fields)
        if got != known:
            raise SystemExit(
                f"the rule gives {got} for {name}, the dialect says {known}"
            )

    # The rule is only worth trusting where it was tested; say what it produced where it
    # had nothing to be tested against.
    untested = sorted(set(CRC_EXTRA) - set(SEEDS_FROM_DIALECT))
    print("seeds reproduced:", ", ".join(str(seed) for seed in SEEDS_FROM_DIALECT.values()))
    print("seeds worked out alone:", ", ".join(f"{MESSAGES[m][0]}={CRC_EXTRA[m]}" for m in untested))


def crc_accumulate(data: bytes, extra: int) -> int:
    """The MAVLink CRC-16/MCRF4XX over the frame, seeded with the message's CRC_EXTRA."""
    crc = 0xFFFF

    for byte in data:
        tmp = (byte ^ (crc & 0xFF)) & 0xFF
        tmp = (tmp ^ (tmp << 4)) & 0xFF
        crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4)) & 0xFFFF

    tmp = (extra ^ (crc & 0xFF)) & 0xFF
    tmp = (tmp ^ (tmp << 4)) & 0xFF
    crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4)) & 0xFFFF

    return crc


def frame(message_id: int, payload: bytes, sequence: int, system_id=1, component_id=1) -> bytes:
    """Builds one MAVLink 2 frame.

    MAVLink 2 removes the trailing zero bytes of the payload to save bandwidth. A receiver
    pads the payload back to the message's declared length, so both forms are valid; the
    truncated form is what a real autopilot sends, and exercising it here means the parser
    is tested against the shorter payload too.
    """
    payload = payload.rstrip(b"\x00")

    header = (
        bytes([len(payload), 0, 0, sequence, system_id, component_id])
        + message_id.to_bytes(3, "little")
    )
    body = header + payload
    crc = crc_accumulate(body, CRC_EXTRA[message_id])
    return bytes([STX]) + body + struct.pack("<H", crc)


def heartbeat(custom_mode: int) -> bytes:
    # custom_mode, type, autopilot, base_mode, system_status, mavlink_version
    payload = struct.pack("<IBBBBB", custom_mode, 2, 3, 0, 4, 3)
    return frame(0, payload, 0)


def global_position(sequence: int, lat: float, lng: float) -> bytes:
    # time_boot_ms, lat, lon, alt, relative_alt, vx, vy, vz, hdg
    payload = struct.pack(
        "<IiiiihhhH",
        sequence * 100,
        int(lat * 1e7),
        int(lng * 1e7),
        584_000,
        100_000,
        250,
        0,
        0,
        9_000,
    )
    return frame(33, payload, sequence)


def sys_status(sequence: int) -> bytes:
    payload = struct.pack(
        "<IIIHHhHHhhhhb",
        0, 0, 0,
        0,
        12_340,
        -1_500,
        0,
        0,
        0, 0, 0, 0,
        87,
    )
    return frame(1, payload, sequence)


def attitude(sequence: int) -> bytes:
    # time_boot_ms, roll, pitch, yaw, rollspeed, pitchspeed, yawspeed
    payload = struct.pack("<Iffffff", sequence * 100, 0.05, -0.02, 1.57, 0.0, 0.0, 0.0)
    return frame(30, payload, sequence)


def home_position(sequence: int, lat: float, lng: float) -> bytes:
    # HOME_POSITION is exactly 52 bytes: three int32, three float32, a quaternion of four
    # float32, and three more float32.
    payload = struct.pack("<iii", int(lat * 1e7), int(lng * 1e7), 584_000)
    payload += struct.pack("<fff", 0.0, 0.0, 0.0)
    payload += struct.pack("<ffff", 1.0, 0.0, 0.0, 0.0)
    payload += struct.pack("<fff", 0.0, 0.0, 0.0)
    assert len(payload) == 52, f"the home position payload is {len(payload)} bytes"
    return frame(242, payload, sequence)


def set_attitude_target(sequence: int) -> bytes:
    """The answer the formation gives for a right angle of roll.

    The quaternion is the one the hand works out: ninety degrees about the x axis is
    `(√2/2, √2/2, 0, 0)` in the `(w, x, y, z)` order MAVLink uses. The mask is `0xff` less
    the three terms upstream subtracts — `0b00000010`, `0b01000000` and `0b10000101` — which
    is `0x38`, and the thrust is the floor of the demand.

    The payload is packed in **wire order**, not in definition order: the base fields are
    sorted by type length descending, ties keeping the definition order. The extension that
    follows is zero, so MAVLink 2 truncates it away.
    """
    root_half = math.sqrt(0.5)

    base = struct.pack(
        "<IffffffffBBB",
        0,                              # time_boot_ms, as the decision leaves it
        root_half, root_half, 0.0, 0.0,  # q, in (w, x, y, z) order
        0.0, 0.0, 0.0,            # the three body rates
        0.1,                      # thrust, at its floor
        1,                        # target_system
        1,                        # target_component
        0x38,                     # type_mask
    )

    assert len(base) == 39, f"the base fields are {len(base)} bytes"

    return frame(82, base + b"\x00" * 12, sequence)


def build() -> bytes:
    """Builds the fixture stream.

    The sequence numbers are contiguous, as they are on a real link: a gap in the sequence
    means a packet was lost, and the loss accounting should see none.
    """
    out = bytearray()
    sequence = 0

    def next_sequence() -> int:
        nonlocal sequence
        current = sequence
        sequence = (sequence + 1) & 0xFF
        return current

    # Two heartbeats so the vehicle is identified, then a burst of telemetry.
    out += frame(0, struct.pack("<IBBBBB", 5, 2, 3, 0, 4, 3), next_sequence())
    out += frame(0, struct.pack("<IBBBBB", 5, 2, 3, 0, 4, 3), next_sequence())

    for _ in range(3):
        out += frame(
            33,
            struct.pack(
                "<IiiiihhhH",
                0,
                int(35.2809 * 1e7),
                int(149.1300 * 1e7),
                584_000,
                100_000,
                250,
                0,
                0,
                9_000,
            ),
            next_sequence(),
        )
        out += frame(
            30,
            struct.pack("<Iffffff", 0, 0.05, -0.02, 1.57, 0.0, 0.0, 0.0),
            next_sequence(),
        )
        out += frame(
            1,
            struct.pack(
                "<IIIHHhHHhhhhb",
                0, 0, 0,
                0,
                12_340,
                -1_500,
                0,
                0,
                0, 0, 0, 0,
                87,
            ),
            next_sequence(),
        )

    # HOME_POSITION is exactly 52 bytes: three int32, three float32, a quaternion of four
    # float32, and three more float32.
    home = struct.pack("<iii", int(35.2809 * 1e7), int(149.1300 * 1e7), 584_000)
    home += struct.pack("<fff", 0.0, 0.0, 0.0)
    home += struct.pack("<ffff", 1.0, 0.0, 0.0, 0.0)
    home += struct.pack("<fff", 0.0, 0.0, 0.0)
    assert len(home) == 52, f"the home position payload is {len(home)} bytes"
    out += frame(242, home, next_sequence())

    # The formation's answer for a right angle of roll, so that the wire shape of the
    # decision is checked against something other than the code that writes it.
    out += set_attitude_target(next_sequence())

    return bytes(out)


def main() -> int:
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <output file>", file=sys.stderr)
        return 1

    check_seeds()

    data = build()

    with open(sys.argv[1], "wb") as handle:
        handle.write(data)

    print(f"wrote {len(data)} bytes to {sys.argv[1]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
