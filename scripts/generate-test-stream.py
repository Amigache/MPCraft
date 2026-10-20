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

import struct
import sys

STX = 0xFD

# The CRC_EXTRA seed of each message, from the MAVLink dialect.
CRC_EXTRA = {
    0: 50,    # HEARTBEAT
    1: 124,   # SYS_STATUS
    30: 39,   # ATTITUDE
    33: 104,  # GLOBAL_POSITION_INT
    242: 104, # HOME_POSITION
}


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

    return bytes(out)


def main() -> int:
    if len(sys.argv) < 2:
        print(f"usage: {sys.argv[0]} <output file>", file=sys.stderr)
        return 1

    data = build()

    with open(sys.argv[1], "wb") as handle:
        handle.write(data)

    print(f"wrote {len(data)} bytes to {sys.argv[1]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
