#!/usr/bin/env python3
"""A simulated ArduPilot vehicle, for demonstrating and testing the ground station.

It answers over UDP exactly as a vehicle does: a heartbeat every second, the telemetry
a ground station displays every frame, the parameters it is asked for, the plan it
holds, and the acknowledgements for the commands it is given.

The MAVLink framing here is written from the specification, independently of the Rust
implementation, so the two agreeing is evidence that both are right.

    python scripts/simulated-vehicle.py --port 14550

# The field order

MAVLink 2 reorders the fields of a message: the larger types come first, so that the
payload needs no padding. The extension fields are the exception: they keep the order
in which they are declared, and they are only present in MAVLink 2.

The rule is implemented once, in :func:`encode`, and every message goes through it.
"""

import argparse
import math
import socket
import struct
import sys
import time

STX_V1 = 0xFE
STX_V2 = 0xFD

# The type of every field, with the size class it belongs to. The size class is what
# the reordering sorts on: a char array is a run of eight-bit values, so it sorts with
# the eight-bit fields.
FORMATS = {
    "u8": "<B",
    "i8": "<b",
    "u16": "<H",
    "i16": "<h",
    "u32": "<I",
    "i32": "<i",
    "u64": "<Q",
    "i64": "<q",
    "f32": "<f",
    "f64": "<d",
    "char50": "<50s",
}

CLASSES = {
    "u8": 1,
    "i8": 1,
    "char16": 1,
    "char50": 1,
    "u16": 2,
    "i16": 2,
    "u32": 4,
    "i32": 4,
    "f32": 4,
    "u64": 8,
    "i64": 8,
    "f64": 8,
}

# The MAVLink parameter types.
#
# The numbering is not the obvious one: the types are numbered from the smallest to
# the largest unsigned type first, and the signed types follow their unsigned
# counterparts.
PARAM_UINT8 = 1
PARAM_INT8 = 2
PARAM_UINT16 = 3
PARAM_INT16 = 4
PARAM_UINT32 = 5
PARAM_INT32 = 6
PARAM_UINT64 = 7
PARAM_INT64 = 8
PARAM_REAL32 = 9
PARAM_REAL64 = 10

# The plan types.
MISSION_MISSION = 0
MISSION_FENCE = 1
MISSION_RALLY = 2

# The command results.
RESULT_ACCEPTED = 0
RESULT_TEMPORARILY_REJECTED = 3

# The commands the vehicle understands.
CMD_NAV_WAYPOINT = 16
CMD_NAV_RETURN_TO_LAUNCH = 20
CMD_NAV_LAND = 21
CMD_NAV_TAKEOFF = 22
CMD_FENCE_RETURN_POINT = 5000
CMD_FENCE_VERTEX_INCLUSION = 5001
CMD_FENCE_VERTEX_EXCLUSION = 5002
CMD_FENCE_CIRCLE_INCLUSION = 5003
CMD_NAV_RALLY_POINT = 5100
CMD_DO_SET_MODE = 176
CMD_DO_JUMP = 177
CMD_COMPONENT_ARM_DISARM = 400


class Message:
    """One message of the dialect: its id, its seed and its fields."""

    def __init__(self, message_id, name, crc_extra, base, extensions=()):
        self.id = message_id
        self.name = name
        self.crc_extra = crc_extra
        # The base fields, in the order the specification declares them.
        self.base = list(base)
        # The extension fields, which only travel in MAVLink 2 and are not reordered.
        self.extensions = list(extensions)

    def wire_order(self):
        """The fields in the order they travel: the base reordered, then the extensions."""
        ordered = sorted(self.base, key=lambda field: CLASSES[field[1]], reverse=True)
        return ordered + list(self.extensions)


def message(message_id, name, crc_extra, base, extensions=()):
    return Message(message_id, name, crc_extra, base, extensions)


# The catalogue of the messages this vehicle uses. The ids and the CRC seeds come from
# the MAVLink dialect; the field lists come from the message definitions.
MESSAGES = {
    0: message(
        0,
        "HEARTBEAT",
        50,
        [
            ("custom_mode", "u32"),
            ("mavtype", "u8"),
            ("autopilot", "u8"),
            ("base_mode", "u8"),
            ("system_status", "u8"),
            ("mavlink_version", "u8"),
        ],
    ),
    1: message(
        1,
        "SYS_STATUS",
        124,
        [
            ("onboard_control_sensors_present", "u32"),
            ("onboard_control_sensors_enabled", "u32"),
            ("onboard_control_sensors_health", "u32"),
            ("load", "u16"),
            ("voltage_battery", "u16"),
            ("current_battery", "i16"),
            ("drop_rate_comm", "u16"),
            ("errors_comm", "u16"),
            ("errors_count1", "u16"),
            ("errors_count2", "u16"),
            ("errors_count3", "u16"),
            ("errors_count4", "u16"),
            ("battery_remaining", "i8"),
        ],
    ),
    11: message(
        11,
        "SET_MODE",
        89,
        [("custom_mode", "u32"), ("target_system", "u8"), ("base_mode", "u8")],
    ),
    20: message(
        20,
        "PARAM_REQUEST_READ",
        214,
        [("param_index", "i16"), ("target_system", "u8"), ("target_component", "u8")],
        [("param_id", "char16")],
    ),
    21: message(
        21,
        "PARAM_REQUEST_LIST",
        159,
        [("target_system", "u8"), ("target_component", "u8")],
    ),
    22: message(
        22,
        "PARAM_VALUE",
        220,
        [
            ("param_value", "f32"),
            ("param_count", "u16"),
            ("param_index", "u16"),
        ],
        [("param_id", "char16"), ("param_type", "u8")],
    ),
    23: message(
        23,
        "PARAM_SET",
        168,
        [("param_value", "f32"), ("target_system", "u8"), ("target_component", "u8")],
        [("param_id", "char16"), ("param_type", "u8")],
    ),
    24: message(
        24,
        "GPS_RAW_INT",
        24,
        [
            # The fields of MAVLink 2 travel in the order of their size, so the
            # sixty-four bit ones come first and the eight bit ones last.
            ("time_usec", "u64"),
            ("lat", "i32"),
            ("lon", "i32"),
            ("alt", "i32"),
            ("eph", "u16"),
            ("epv", "u16"),
            ("vel", "u16"),
            ("cog", "u16"),
            ("fix_type", "u8"),
            ("satellites_visible", "u8"),
        ],
    ),
    27: message(
        27,
        "RAW_IMU",
        144,
        [
            ("time_usec", "u64"),
            ("xacc", "i16"),
            ("yacc", "i16"),
            ("zacc", "i16"),
            ("xgyro", "i16"),
            ("ygyro", "i16"),
            ("zgyro", "i16"),
            ("xmag", "i16"),
            ("ymag", "i16"),
            ("zmag", "i16"),
        ],
        # The identifier of the sensor and its temperature only travel in MAVLink 2.
        extensions=[
            ("id", "u8"),
            ("temperature", "i16"),
        ],
    ),
    29: message(
        29,
        "SCALED_PRESSURE",
        115,
        [
            ("time_boot_ms", "u32"),
            ("press_abs", "f32"),
            ("press_diff", "f32"),
            ("temperature", "i16"),
        ],
        extensions=[
            ("temperature_press_diff", "i16"),
        ],
    ),
    36: message(
        36,
        "SERVO_OUTPUT_RAW",
        222,
        [
            ("time_usec", "u32"),
            ("port", "u8"),
            ("servo1_raw", "u16"),
            ("servo2_raw", "u16"),
            ("servo3_raw", "u16"),
            ("servo4_raw", "u16"),
            ("servo5_raw", "u16"),
            ("servo6_raw", "u16"),
            ("servo7_raw", "u16"),
            ("servo8_raw", "u16"),
        ],
        extensions=[
            ("servo9_raw", "u16"),
            ("servo10_raw", "u16"),
            ("servo11_raw", "u16"),
            ("servo12_raw", "u16"),
            ("servo13_raw", "u16"),
            ("servo14_raw", "u16"),
            ("servo15_raw", "u16"),
            ("servo16_raw", "u16"),
        ],
    ),
    65: message(
        65,
        "RC_CHANNELS",
        118,
        [
            ("time_boot_ms", "u32"),
            ("chancount", "u8"),
            ("chan1_raw", "u16"),
            ("chan2_raw", "u16"),
            ("chan3_raw", "u16"),
            ("chan4_raw", "u16"),
            ("chan5_raw", "u16"),
            ("chan6_raw", "u16"),
            ("chan7_raw", "u16"),
            ("chan8_raw", "u16"),
            ("chan9_raw", "u16"),
            ("chan10_raw", "u16"),
            ("chan11_raw", "u16"),
            ("chan12_raw", "u16"),
            ("chan13_raw", "u16"),
            ("chan14_raw", "u16"),
            ("chan15_raw", "u16"),
            ("chan16_raw", "u16"),
            ("chan17_raw", "u16"),
            ("chan18_raw", "u16"),
            ("rssi", "u8"),
        ],
    ),
    62: message(
        62,
        "NAV_CONTROLLER_OUTPUT",
        183,
        [
            ("nav_roll", "f32"),
            ("nav_pitch", "f32"),
            ("alt_error", "f32"),
            ("aspd_error", "f32"),
            ("xtrack_error", "f32"),
            ("nav_bearing", "i16"),
            ("target_bearing", "i16"),
            ("wp_dist", "u16"),
        ],
    ),
    147: message(
        147,
        "BATTERY_STATUS",
        154,
        [
            ("current_consumed", "i32"),
            ("energy_consumed", "i32"),
            ("temperature", "i16"),
            # The voltages of the cells travel as an array of ten; the wire does not care
            # about the name, only about the order.
            ("cell1", "u16"),
            ("cell2", "u16"),
            ("cell3", "u16"),
            ("cell4", "u16"),
            ("cell5", "u16"),
            ("cell6", "u16"),
            ("cell7", "u16"),
            ("cell8", "u16"),
            ("cell9", "u16"),
            ("cell10", "u16"),
            ("current_battery", "i16"),
            ("id", "u8"),
            ("battery_function", "u8"),
            ("mavtype", "u8"),
            ("battery_remaining", "i8"),
        ],
    ),
    178: message(
        178,
        "AHRS2",
        47,
        [
            ("roll", "f32"),
            ("pitch", "f32"),
            ("yaw", "f32"),
            ("altitude", "f32"),
            ("lat", "i32"),
            ("lng", "i32"),
        ],
    ),
    193: message(
        193,
        "EKF_STATUS_REPORT",
        71,
        [
            ("velocity_variance", "f32"),
            ("pos_horiz_variance", "f32"),
            ("pos_vert_variance", "f32"),
            ("compass_variance", "f32"),
            ("terrain_alt_variance", "f32"),
            ("flags", "u16"),
        ],
        extensions=[
            ("airspeed_variance", "f32"),
        ],
    ),
    241: message(
        241,
        "VIBRATION",
        90,
        [
            ("time_usec", "u64"),
            ("vibration_x", "f32"),
            ("vibration_y", "f32"),
            ("vibration_z", "f32"),
            ("clipping_0", "u32"),
            ("clipping_1", "u32"),
            ("clipping_2", "u32"),
        ],
    ),
    74: message(
        74,
        "VFR_HUD",
        20,
        [
            ("airspeed", "f32"),
            ("groundspeed", "f32"),
            ("heading", "i16"),
            ("throttle", "u16"),
            ("alt", "f32"),
            ("climb", "f32"),
        ],
    ),
    30: message(
        30,
        "ATTITUDE",
        39,
        [
            ("time_boot_ms", "u32"),
            ("roll", "f32"),
            ("pitch", "f32"),
            ("yaw", "f32"),
            ("rollspeed", "f32"),
            ("pitchspeed", "f32"),
            ("yawspeed", "f32"),
        ],
    ),
    33: message(
        33,
        "GLOBAL_POSITION_INT",
        104,
        [
            ("time_boot_ms", "u32"),
            ("lat", "i32"),
            ("lon", "i32"),
            ("alt", "i32"),
            ("relative_alt", "i32"),
            ("vx", "i16"),
            ("vy", "i16"),
            ("vz", "i16"),
            ("hdg", "u16"),
        ],
    ),
    39: message(
        39,
        "MISSION_ITEM",
        254,
        [
            ("param1", "f32"),
            ("param2", "f32"),
            ("param3", "f32"),
            ("param4", "f32"),
            ("x", "f32"),
            ("y", "f32"),
            ("z", "f32"),
            ("seq", "u16"),
            ("command", "u16"),
            ("target_system", "u8"),
            ("target_component", "u8"),
            ("frame", "u8"),
            ("current", "u8"),
            ("autocontinue", "u8"),
        ],
        [("mission_type", "u8")],
    ),
    40: message(
        40,
        "MISSION_REQUEST",
        230,
        [("seq", "u16"), ("target_system", "u8"), ("target_component", "u8")],
        [("mission_type", "u8")],
    ),
    41: message(
        41,
        "MISSION_SET_CURRENT",
        28,
        [("seq", "u16"), ("target_system", "u8"), ("target_component", "u8")],
    ),
    42: message(
        42,
        "MISSION_CURRENT",
        28,
        [
            ("seq", "u16"),
            ("total", "u16"),
            ("mission_state", "u8"),
            ("mission_mode", "u8"),
            ("mission_id", "u32"),
            ("fence_id", "u32"),
            ("rally_points_id", "u32"),
        ],
    ),
    43: message(
        43,
        "MISSION_REQUEST_LIST",
        132,
        [("target_system", "u8"), ("target_component", "u8")],
        [("mission_type", "u8")],
    ),
    44: message(
        44,
        "MISSION_COUNT",
        221,
        [("count", "u16"), ("target_system", "u8"), ("target_component", "u8")],
        [("mission_type", "u8"), ("opaque_id", "u32")],
    ),
    45: message(
        45,
        "MISSION_CLEAR_ALL",
        232,
        [("target_system", "u8"), ("target_component", "u8")],
        [("mission_type", "u8")],
    ),
    47: message(
        47,
        "MISSION_ACK",
        153,
        [("target_system", "u8"), ("target_component", "u8"), ("mavtype", "u8")],
        [("mission_type", "u8"), ("opaque_id", "u32")],
    ),
    51: message(
        51,
        "MISSION_REQUEST_INT",
        196,
        [("seq", "u16"), ("target_system", "u8"), ("target_component", "u8")],
        [("mission_type", "u8")],
    ),
    73: message(
        73,
        "MISSION_ITEM_INT",
        38,
        [
            ("param1", "f32"),
            ("param2", "f32"),
            ("param3", "f32"),
            ("param4", "f32"),
            ("x", "i32"),
            ("y", "i32"),
            ("z", "f32"),
            ("seq", "u16"),
            ("command", "u16"),
            ("target_system", "u8"),
            ("target_component", "u8"),
            ("frame", "u8"),
            ("current", "u8"),
            ("autocontinue", "u8"),
        ],
        [("mission_type", "u8")],
    ),
    76: message(
        76,
        "COMMAND_LONG",
        152,
        [
            ("param1", "f32"),
            ("param2", "f32"),
            ("param3", "f32"),
            ("param4", "f32"),
            ("param5", "f32"),
            ("param6", "f32"),
            ("param7", "f32"),
            ("command", "u16"),
            ("target_system", "u8"),
            ("target_component", "u8"),
            ("confirmation", "u8"),
        ],
    ),
    77: message(
        77,
        "COMMAND_ACK",
        143,
        [("command", "u16"), ("result", "u8")],
        [
            ("progress", "u8"),
            ("result_param2", "i32"),
            ("target_system", "u8"),
            ("target_component", "u8"),
        ],
    ),
    253: message(
        253,
        "STATUSTEXT",
        83,
        [
            ("severity", "u8"),
            ("text", "char50"),
            ("id", "u16"),
            ("chunk_seq", "u8"),
        ],
    ),
}


def crc_accumulate(data, extra):
    """The MAVLink CRC-16/MCRF4XX."""
    crc = 0xFFFF

    for byte in data:
        tmp = (byte ^ (crc & 0xFF)) & 0xFF
        tmp = (tmp ^ (tmp << 4)) & 0xFF
        crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4)) & 0xFFFF

    tmp = (extra ^ (crc & 0xFF)) & 0xFF
    tmp = (tmp ^ (tmp << 4)) & 0xFF
    crc = ((crc >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4)) & 0xFFFF

    return crc


def pack_field(kind, value):
    """Packs one field."""
    if kind.startswith("char"):
        length = int(kind[4:])
        if isinstance(value, str):
            value = value.encode("utf-8")
        return value[:length].ljust(length, b"\x00")
    return struct.pack(FORMATS[kind], value)


def encode(message_id, values):
    """Packs a message into its payload, in the order the fields travel."""
    catalogue = MESSAGES[message_id]
    payload = b""

    for name, kind in catalogue.wire_order():
        # A field that is not given travels as zero, which is what the specification
        # says about the missing fields of a truncated payload.
        default = "" if kind.startswith("char") else 0
        payload += pack_field(kind, values.get(name, default))

    return payload


def decode(message_id, payload):
    """Unpacks a message payload into a dictionary."""
    catalogue = MESSAGES[message_id]
    values = {}
    offset = 0

    for name, kind in catalogue.wire_order():
        if kind.startswith("char"):
            length = int(kind[4:])
            chunk = payload[offset : offset + length]
            offset += length
            values[name] = chunk.split(b"\x00")[0].decode("utf-8", "replace")
            continue

        size = struct.calcsize(FORMATS[kind])
        chunk = payload[offset : offset + size]
        offset += size

        if len(chunk) < size:
            # The payload was truncated: the bytes that did not arrive are zero, which
            # is what the specification says about a short payload.
            chunk = chunk.ljust(size, b"\x00")

        values[name] = struct.unpack(FORMATS[kind], chunk)[0]

    return values


def frame(message_id, payload, sequence, system_id=1, component_id=1):
    """Builds one MAVLink 2 frame, with the trailing zeroes of the payload removed."""
    # MAVLink 2 removes the trailing zero bytes of the payload.
    trimmed = payload
    while trimmed.endswith(b"\x00"):
        trimmed = trimmed[:-1]

    header = (
        bytes([len(trimmed), 0, 0, sequence & 0xFF, system_id, component_id])
        + message_id.to_bytes(3, "little")
    )
    body = header + trimmed
    crc = crc_accumulate(body, MESSAGES[message_id].crc_extra)

    return bytes([STX_V2]) + body + struct.pack("<H", crc)


def parse(data):
    """Reads a frame and returns its id, its payload and who sent it."""
    if len(data) < 8:
        return None

    if data[0] == STX_V2:
        # The header is: the marker, the length, the incompatibility flags, the
        # compatibility flags, the sequence, the system, the component and the message
        # id in three bytes.
        length = data[1]
        if len(data) < 12 + length:
            return None
        return (
            int.from_bytes(data[7:10], "little"),
            data[10 : 10 + length],
            data[5],
            data[6],
        )

    if data[0] == STX_V1:
        length = data[1]
        if len(data) < 8 + length:
            return None
        return (data[5], data[6 : 6 + length], data[3], data[4])

    return None


# The parameters the vehicle holds, in the order they are reported. The values are the
# ones an ArduPilot vehicle would plausibly have.
PARAMETERS = [
    ("ARMING_CHECK", 1, PARAM_UINT8),
    ("DISARM_DELAY", 0, PARAM_UINT8),
    ("FS_BATT_ENABLE", 1, PARAM_UINT8),
    ("FS_GCS_ENABLE", 1, PARAM_UINT8),
    ("FENCE_ENABLE", 1, PARAM_UINT8),
    ("FENCE_TYPE", 1, PARAM_UINT8),
    ("FENCE_MARGIN", 10, PARAM_UINT16),
    ("TERRAIN_FOLLOW", 0, PARAM_UINT8),
    ("WP_RADIUS", 90, PARAM_UINT16),
    ("WPNAV_SPEED", 500, PARAM_INT16),
    ("RTL_ALT", 0, PARAM_UINT16),
    ("RTL_SPEED", 0, PARAM_UINT16),
    ("SYSID_THISMAV", 1, PARAM_UINT8),
    ("MAV_TYPE", 2, PARAM_UINT8),
    ("COMPASS_OFS_X", 0, PARAM_INT16),
    ("COMPASS_OFS_Y", 0, PARAM_INT16),
    ("GPS_TYPE", 0, PARAM_UINT8),
    ("SERIAL0_BAUD", 57600, PARAM_UINT32),
    ("INS_GYR_ID", 1, PARAM_UINT8),
]


class Vehicle:
    """A vehicle that flies a circle and answers what it is asked."""

    def __init__(self, host, port, mission_int=False):
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.bind((host, port))
        self.socket.settimeout(0.1)
        self.sequence = 0
        self.start = time.monotonic()
        # The ground stations that are listening. They are learned from the frames they
        # send: a vehicle that only streams telemetry never learns the address otherwise.
        #
        # There is more than one, because a link can be watched by more than one ground
        # station, and Mission Planner's UDP transport fans out to every peer it has
        # heard from.
        self.clients = []

        # Whether the vehicle wants the integer form of the mission items.
        self.mission_int = mission_int

        # The parameters, by name.
        self.parameters = {name: value for name, value, _ in PARAMETERS}
        self.parameter_types = {name: kind for name, _, kind in PARAMETERS}

        # The plans it holds, by type.
        self.plans = {
            MISSION_MISSION: self.default_mission(),
            MISSION_FENCE: self.default_fence(),
            MISSION_RALLY: self.default_rally(),
        }

        # The item the vehicle is executing.
        self.current = 0

        # The upload in progress, if the ground station is writing a plan.
        self.upload = None

        self.armed = False
        self.mode = 5  # GUIDED

    @staticmethod
    def default_mission():
        """The plan the vehicle starts with: home, then a square."""
        return [
            {
                "seq": 0,
                "command": CMD_NAV_WAYPOINT,
                "frame": 0,
                "param1": 0.0,
                "param2": 0.0,
                "param3": 0.0,
                "param4": 0.0,
                "x": -35.2809,
                "y": 149.1300,
                "z": 640.0,
                "current": 1,
                "autocontinue": 1,
            },
            {
                "seq": 1,
                "command": CMD_NAV_WAYPOINT,
                "frame": 0,
                "param1": 0.0,
                "param2": 0.0,
                "param3": 0.0,
                "param4": 0.0,
                "x": -35.2820,
                "y": 149.1310,
                "z": 660.0,
                "current": 0,
                "autocontinue": 1,
            },
            {
                "seq": 2,
                "command": CMD_NAV_WAYPOINT,
                "frame": 0,
                "param1": 0.0,
                "param2": 0.0,
                "param3": 0.0,
                "param4": 0.0,
                "x": -35.2830,
                "y": 149.1310,
                "z": 660.0,
                "current": 0,
                "autocontinue": 1,
            },
            {
                "seq": 3,
                "command": CMD_NAV_WAYPOINT,
                "frame": 0,
                "param1": 0.0,
                "param2": 0.0,
                "param3": 0.0,
                "param4": 0.0,
                "x": -35.2830,
                "y": 149.1300,
                "z": 660.0,
                "current": 0,
                "autocontinue": 1,
            },
        ]

    @staticmethod
    def default_fence():
        """The fence the vehicle starts with.

        A square around the place it flies, a circle in the middle of it and a point that
        is excluded from it: the three kinds of thing a fence is made of.
        """
        def vertex(seq, command, lat, lng, param1=0.0):
            return {
                "seq": seq,
                "command": command,
                "frame": 0,
                "param1": param1,
                "param2": 0.0,
                "param3": 0.0,
                "param4": 0.0,
                "x": lat,
                "y": lng,
                "z": 0.0,
                "current": 0,
                "autocontinue": 1,
            }

        return [
            vertex(0, CMD_FENCE_VERTEX_INCLUSION, -35.2850, 149.1250),
            vertex(1, CMD_FENCE_VERTEX_INCLUSION, -35.2850, 149.1350),
            vertex(2, CMD_FENCE_VERTEX_INCLUSION, -35.2750, 149.1350),
            vertex(3, CMD_FENCE_VERTEX_INCLUSION, -35.2750, 149.1250),
            vertex(4, CMD_FENCE_CIRCLE_INCLUSION, -35.2809, 149.1300, 120.0),
            vertex(5, CMD_FENCE_VERTEX_EXCLUSION, -35.2800, 149.1280),
        ]

    @staticmethod
    def default_rally():
        """The rally point the vehicle starts with."""
        return [
            {
                "seq": 0,
                "command": CMD_NAV_RALLY_POINT,
                "frame": 0,
                "param1": 0.0,
                "param2": 0.0,
                "param3": 0.0,
                "param4": 0.0,
                "x": -35.2790,
                "y": 149.1320,
                "z": 30.0,
                "current": 0,
                "autocontinue": 1,
            },
        ]

    def next_sequence(self):
        self.sequence = (self.sequence + 1) & 0xFF
        return self.sequence

    def position(self):
        """The vehicle's position: a slow circle around Canberra."""
        elapsed = time.monotonic() - self.start
        angle = elapsed * 0.2
        lat = -35.2809 + math.sin(angle) * 0.002
        lng = 149.1300 + math.cos(angle) * 0.002
        return lat, lng, 584.0 + math.sin(elapsed * 0.5) * 40.0

    def run(self):
        print(f"the simulated vehicle is listening on {self.socket.getsockname()}", flush=True)

        last_heartbeat = 0.0
        last_current = 0.0

        while True:
            # Answer whatever the ground station sends, so the link looks alive from its
            # side too.
            try:
                data, address = self.socket.recvfrom(2048)
                if address not in self.clients:
                    self.clients.append(address)
                    print(f"the ground station is at {address}", flush=True)
                self.handle(data)
            except socket.timeout:
                pass
            except ConnectionResetError:
                # The previous datagram went to a port that was not listening. On Windows
                # the error arrives with the next packet, and the packet is lost with it,
                # so the ground stations are forgotten: they announce themselves again
                # every second, and until they do the vehicle has nobody to talk to.
                print("the ground station went away", flush=True)
                self.clients.clear()
            except OSError as error:
                print(f"the link failed: {error}", flush=True)
                break

            now = time.monotonic()
            elapsed = now - self.start
            lat, lng, alt = self.position()

            # A heartbeat once a second.
            if now - last_heartbeat >= 1.0:
                last_heartbeat = now
                self.send_heartbeat()

            # The attitude, tilted as the vehicle flies the circle.
            roll = math.sin(elapsed * 0.2) * 0.15
            pitch = math.cos(elapsed * 0.3) * 0.05
            yaw = (elapsed * 0.2) % (2 * math.pi)
            self.send(
                30,
                {
                    "time_boot_ms": int(elapsed * 1000),
                    "roll": roll,
                    "pitch": pitch,
                    "yaw": yaw,
                    "rollspeed": 0.0,
                    "pitchspeed": 0.0,
                    "yawspeed": 0.0,
                },
            )

            # The position.
            ground_speed = 12.0
            self.send(
                33,
                {
                    "time_boot_ms": int(elapsed * 1000),
                    "lat": int(lat * 1e7),
                    "lon": int(lng * 1e7),
                    "alt": int(alt * 1000),
                    "relative_alt": int((alt - 584.0) * 1000),
                    "vx": int(ground_speed * 100),
                    "vy": 0,
                    "vz": 0,
                    "hdg": int(math.degrees(yaw) * 100) % 36000,
                },
            )

            # The raw state of the global positioning, which is what the pre-flight
            # checklist watches: the fix, the satellites and the dilution of precision.
            self.send(
                24,
                {
                    "time_usec": int(elapsed * 1_000_000),
                    "lat": int(lat * 1e7),
                    "lon": int(lng * 1e7),
                    "alt": int(alt * 1000),
                    "eph": 110,
                    "epv": 150,
                    "vel": int(ground_speed * 100),
                    "cog": int(math.degrees(yaw) * 100) % 36000,
                    "fix_type": 3,
                    "satellites_visible": 14,
                },
            )

            # The inertial sensors. The accelerometers are in thousandths of a g, the
            # gyroscopes in thousandths of a radian per second and the magnetometers in
            # thousandths of a gauss.
            self.send(
                27,
                {
                    "time_usec": int(elapsed * 1_000_000),
                    # The vehicle hangs from its rotors, so gravity is on the third axis.
                    "xacc": int(roll * 900),
                    "yacc": int(pitch * 900),
                    "zacc": -1000 + int(roll * 120),
                    "xgyro": 0,
                    "ygyro": 0,
                    "zgyro": 0,
                    "xmag": 210,
                    "ymag": -35,
                    "zmag": -412,
                    "id": 0,
                    "temperature": 2500,
                },
            )

            # The barometer.
            self.send(
                29,
                {
                    "time_boot_ms": int(elapsed * 1000),
                    "press_abs": 1013.25 - alt * 0.008,
                    "press_diff": 0.0,
                    "temperature": 2150,
                },
            )

            # The servos: a quadcopter, two of them turning one way and two the other.
            self.send(
                36,
                {
                    "time_usec": int(elapsed * 1_000_000),
                    "port": 0,
                    "servo1_raw": 1500,
                    "servo2_raw": 1500,
                    "servo3_raw": 1800,
                    "servo4_raw": 1800,
                    "servo5_raw": 1000,
                    "servo6_raw": 1000,
                    "servo7_raw": 1800,
                    "servo8_raw": 1800,
                },
            )

            # The radio control.
            self.send(
                65,
                {
                    "time_boot_ms": int(elapsed * 1000),
                    "chancount": 8,
                    "chan1_raw": 1500,
                    "chan2_raw": 1500,
                    "chan3_raw": 1000,
                    "chan4_raw": 2000,
                    "chan5_raw": 1500,
                    "chan6_raw": 1500,
                    "chan7_raw": 1500,
                    "chan8_raw": 1500,
                    "rssi": 92,
                },
            )

            # The head-up display: the air data the pilot reads.
            self.send(
                74,
                {
                    "airspeed": ground_speed,
                    "groundspeed": ground_speed,
                    # The heading of the HUD is in degrees, not in hundredths.
                    "heading": int(math.degrees(yaw)) % 360,
                    "throttle": 45,
                    "alt": alt,
                    "climb": 0.0,
                },
            )

            # The second estimator, which the vehicle compares the first with.
            self.send(
                178,
                {
                    "roll": roll,
                    "pitch": pitch,
                    "yaw": yaw,
                    "altitude": alt,
                    "lat": int(lat * 1e7),
                    "lng": int(lng * 1e7),
                },
            )

            # What the controller asks for, against what the vehicle does.
            self.send(
                62,
                {
                    "nav_roll": roll * 0.95,
                    "nav_pitch": pitch * 0.95,
                    "alt_error": 0.0,
                    "aspd_error": 0.1,
                    "xtrack_error": 0.5,
                    "nav_bearing": int(math.degrees(yaw)) % 360,
                    "target_bearing": int(math.degrees(yaw)) % 360,
                    "wp_dist": 120,
                },
            )

            # The state of the estimator: how sure it is of what it has measured.
            self.send(
                193,
                {
                    "velocity_variance": 0.04,
                    "pos_horiz_variance": 0.25,
                    "pos_vert_variance": 0.09,
                    "compass_variance": 0.02,
                    "terrain_alt_variance": 0.0,
                    "flags": 0,
                },
            )

            # The vibration of the airframe.
            self.send(
                241,
                {
                    "time_usec": int(elapsed * 1_000_000),
                    "vibration_x": 1.2 + 0.3 * math.sin(elapsed * 3.0),
                    "vibration_y": 0.8 + 0.2 * math.cos(elapsed * 2.5),
                    "vibration_z": 0.5,
                    "clipping_0": 0,
                    "clipping_1": 0,
                    "clipping_2": 0,
                },
            )

            # The bank of the battery: the cells of the pack, one by one.
            cell = 4200
            self.send(
                147,
                {
                    "current_consumed": 0,
                    "energy_consumed": 0,
                    "temperature": 2500,
                    "cell1": cell,
                    "cell2": cell,
                    "cell3": cell,
                    "cell4": cell,
                    "cell5": cell,
                    "cell6": cell,
                    "current_battery": -1500,
                    "id": 0,
                    "battery_function": 0,
                    "mavtype": 1,
                    "battery_remaining": 85,
                },
            )

            # The battery, slowly draining.
            remaining = max(0.0, 87.0 - elapsed * 0.05)
            self.send(
                1,
                {
                    "onboard_control_sensors_present": 0,
                    "onboard_control_sensors_enabled": 0,
                    "onboard_control_sensors_health": 0,
                    "load": 0,
                    # A six cell pack, which is what a multirotor of this size carries.
                    "voltage_battery": int((25.2 - elapsed * 0.002) * 1000),
                    "current_battery": int(-15.0 * 100),
                    "drop_rate_comm": 0,
                    "errors_comm": 0,
                    "errors_count1": 0,
                    "errors_count2": 0,
                    "errors_count3": 0,
                    "errors_count4": 0,
                    "battery_remaining": int(remaining),
                },
            )

            # Which item the vehicle is executing, once a second.
            if now - last_current >= 1.0:
                last_current = now
                self.send_mission_current()

            time.sleep(0.1)

    # ------------------------------------------------------------------ sending

    def send(self, message_id, values):
        """Sends a message to the ground station.

        A message that cannot be built is a mistake in the vehicle, and it is said out
        loud — but the vehicle keeps flying: one broken message must not take the whole
        simulation down, or the mistake goes unnoticed for as long as nobody looks at
        the log.
        """
        if not self.clients:
            # No ground station has been heard from yet, so there is nowhere to send.
            return

        try:
            payload = encode(message_id, values)
        except (struct.error, ValueError, KeyError) as error:
            name = MESSAGES[message_id].name
            print(f"the message {name} could not be built: {error}", flush=True)
            return

        # The frame is built once and sent to every ground station that is listening.
        try:
            packet = frame(message_id, payload, self.next_sequence())
        except (struct.error, ValueError) as error:
            name = MESSAGES[message_id].name
            print(f"the frame of {name} could not be built: {error}", flush=True)
            return

        for client in list(self.clients):
            try:
                self.socket.sendto(packet, client)
            except OSError as error:
                print(f"could not send: {error}", flush=True)

    def send_heartbeat(self):
        """The heartbeat: the vehicle's type, its mode and its capabilities."""
        capabilities = 0
        if self.mission_int:
            capabilities |= 1 << 4  # MAV_PROTOCOL_CAPABILITY_MISSION_INT

        self.send(
            0,
            {
                "custom_mode": self.mode,
                "mavtype": 2,  # MAV_TYPE_QUADROTOR
                "autopilot": 3,  # MAV_AUTOPILOT_ARDUPILOTMEGA
                "base_mode": 1 if self.armed else 0,
                "system_status": 4,  # MAV_STATE_ACTIVE
                "mavlink_version": 3,
            },
        )

    def send_mission_current(self):
        """Which item of the plan the vehicle is executing."""
        plan = self.plans[MISSION_MISSION]
        self.send(
            42,
            {
                "seq": self.current,
                "total": len(plan),
                "mission_state": 0,  # MISSION_STATE_ACTIVE
                "mission_mode": 0,
                "mission_id": 1,
                "fence_id": 1 if self.plans[MISSION_FENCE] else 0,
                "rally_points_id": 1 if self.plans[MISSION_RALLY] else 0,
            },
        )

    def send_status_text(self, text, severity=6):
        """A message from the vehicle, as `STATUSTEXT`."""
        self.send(253, {"severity": severity, "text": text, "id": 0, "chunk_seq": 0})

    # --------------------------------------------------------------- receiving

    def handle(self, data):
        """Handles one frame from the ground station."""
        parsed = parse(data)
        if parsed is None:
            return

        message_id, payload, system_id, component_id = parsed

        if message_id not in MESSAGES:
            # A message this vehicle does not know about is ignored.
            return

        values = decode(message_id, payload)

        handler = {
            0: self.on_heartbeat,
            11: self.on_set_mode,
            20: self.on_param_request_read,
            21: self.on_param_request_list,
            23: self.on_param_set,
            39: self.on_mission_item,
            40: self.on_mission_request,
            41: self.on_mission_set_current,
            43: self.on_mission_request_list,
            44: self.on_mission_count,
            45: self.on_mission_clear_all,
            51: self.on_mission_request,
            73: self.on_mission_item,
            76: self.on_command_long,
        }.get(message_id)

        if handler is not None:
            handler(values)

    def on_heartbeat(self, values):
        """The ground station announcing itself. Nothing to answer."""
        return

    # -------------------------------------------------------------- parameters

    def on_param_request_list(self, values):
        """The ground station wants every parameter."""
        print("the parameters were requested", flush=True)

        for index, (name, value, kind) in enumerate(PARAMETERS):
            self.send_parameter(name, value, kind, index, len(PARAMETERS))

    def on_param_request_read(self, values):
        """The ground station wants one parameter."""
        name = values.get("param_id", "")
        kind = self.parameter_types.get(name)

        if kind is None:
            # An unknown parameter is reported with a value of zero, which is how a
            # vehicle says it has never heard of it.
            self.send_parameter(name, 0.0, PARAM_REAL32, -1, len(PARAMETERS))
            return

        index = [item[0] for item in PARAMETERS].index(name)
        self.send_parameter(name, self.parameters[name], kind, index, len(PARAMETERS))

    def on_param_set(self, values):
        """The ground station writes a parameter."""
        name = values.get("param_id", "")
        value = values.get("param_value", 0.0)

        if name not in self.parameters:
            print(f"an unknown parameter was written: {name}", flush=True)
            return

        self.parameters[name] = value
        print(f"{name} = {value}", flush=True)

        index = [item[0] for item in PARAMETERS].index(name)
        self.send_parameter(name, value, self.parameter_types[name], index, len(PARAMETERS))

    def send_parameter(self, name, value, kind, index, count):
        """Reports one parameter."""
        self.send(
            22,
            {
                "param_value": float(value),
                "param_count": count,
                "param_index": index & 0xFFFF,
                "param_id": name,
                "param_type": kind,
            },
        )

    # ------------------------------------------------------------------ missions

    def on_mission_request_list(self, values):
        """The ground station wants the plan. It is told how many items there are."""
        mission_type = values.get("mission_type", MISSION_MISSION)
        plan = self.plans.get(mission_type, [])

        self.send(
            44,
            {
                "count": len(plan),
                "target_system": 255,
                "target_component": 190,
                "mission_type": mission_type,
                "opaque_id": 1,
            },
        )

    def on_mission_request(self, values):
        """The ground station wants one item of the plan."""
        mission_type = values.get("mission_type", MISSION_MISSION)
        plan = self.plans.get(mission_type, [])
        seq = values.get("seq", 0)

        if seq >= len(plan):
            return

        self.send_mission_item(mission_type, plan[seq], seq)

    def send_mission_item(self, mission_type, item, seq):
        """Sends one item of the plan."""
        values = {
            "param1": item["param1"],
            "param2": item["param2"],
            "param3": item["param3"],
            "param4": item["param4"],
            "z": item["z"],
            "seq": seq,
            "command": item["command"],
            "target_system": 255,
            "target_component": 190,
            "frame": item["frame"],
            "current": item["current"],
            "autocontinue": item["autocontinue"],
            "mission_type": mission_type,
        }

        if self.mission_int:
            values["x"] = int(item["x"] * 1e7)
            values["y"] = int(item["y"] * 1e7)
            self.send(73, values)
        else:
            values["x"] = item["x"]
            values["y"] = item["y"]
            self.send(39, values)

    def on_mission_count(self, values):
        """The ground station is writing a plan. The vehicle asks for the items."""
        count = values.get("count", 0)
        mission_type = values.get("mission_type", MISSION_MISSION)

        self.upload = {
            "count": count,
            "mission_type": mission_type,
            "items": [],
        }

        print(f"a plan of {count} items is being written", flush=True)

        if count == 0:
            self.finish_upload()
            return

        self.request_item(0)

    def request_item(self, seq):
        """Asks the ground station for the next item of the plan."""
        self.send(
            40,
            {
                "seq": seq,
                "target_system": 255,
                "target_component": 190,
                "mission_type": self.upload["mission_type"],
            },
        )

    def on_mission_item(self, values):
        """One item of the plan has arrived."""
        if self.upload is None:
            return

        seq = values.get("seq", 0)
        item = {
            "seq": seq,
            "command": values.get("command", CMD_NAV_WAYPOINT),
            "frame": values.get("frame", 0),
            "param1": values.get("param1", 0.0),
            "param2": values.get("param2", 0.0),
            "param3": values.get("param3", 0.0),
            "param4": values.get("param4", 0.0),
            "x": values.get("x", 0.0),
            "y": values.get("y", 0.0),
            "z": values.get("z", 0.0),
            "current": values.get("current", 0),
            "autocontinue": values.get("autocontinue", 1),
        }

        # The integer form scales the coordinates; the float form does not.
        if values.get("command") is not None and "x" in values:
            if isinstance(item["x"], int):
                item["x"] = item["x"] / 1e7
                item["y"] = item["y"] / 1e7

        self.upload["items"].append(item)

        if len(self.upload["items"]) < self.upload["count"]:
            self.request_item(len(self.upload["items"]))
            return

        self.finish_upload()

    def finish_upload(self):
        """The plan has arrived. It is stored and acknowledged."""
        if self.upload is None:
            return

        mission_type = self.upload["mission_type"]
        items = self.upload["items"]
        self.upload = None

        self.plans[mission_type] = items
        self.current = 0

        print(f"the plan of {len(items)} items was accepted", flush=True)

        self.send(
            47,
            {
                "target_system": 255,
                "target_component": 190,
                "mavtype": 0,  # MAV_MISSION_ACCEPTED
                "mission_type": mission_type,
                "opaque_id": 1,
            },
        )

    def on_mission_set_current(self, values):
        """The ground station says which item the vehicle should be executing."""
        self.current = values.get("seq", 0)
        print(f"the current item is now {self.current}", flush=True)

    def on_mission_clear_all(self, values):
        """The ground station empties one of the plans."""
        mission_type = values.get("mission_type", MISSION_MISSION)
        self.plans[mission_type] = []
        print(f"the plan of type {mission_type} was emptied", flush=True)

        # The protocol answers the request with an acknowledgement, which says whether
        # the vehicle accepted it.
        self.send(
            47,
            {
                "target_system": 255,
                "target_component": 190,
                "mavtype": 0,  # MAV_MISSION_ACCEPTED
                "mission_type": mission_type,
                "opaque_id": 1,
            },
        )

    # ----------------------------------------------------------------- commands

    def on_set_mode(self, values):
        """The ground station changes the flight mode."""
        self.mode = values.get("custom_mode", 0)
        self.send_status_text(f"the mode is now {self.mode}")
        self.acknowledge(0, RESULT_ACCEPTED)

    def on_command_long(self, values):
        """The ground station gives a command."""
        command = values.get("command", 0)

        if command == CMD_COMPONENT_ARM_DISARM:
            # The first parameter says which way: one to arm, zero to disarm.
            self.armed = values.get("param1", 0.0) >= 0.5
            self.send_status_text("armed" if self.armed else "disarmed")
            self.acknowledge(command, RESULT_ACCEPTED)
            return

        if command == CMD_NAV_TAKEOFF:
            self.send_status_text("taking off")
            self.acknowledge(command, RESULT_ACCEPTED)
            return

        if command == CMD_DO_JUMP:
            self.current = int(values.get("param1", 0.0))
            self.send_status_text(f"jumped to item {self.current}")
            self.acknowledge(command, RESULT_ACCEPTED)
            return

        if command == CMD_NAV_RETURN_TO_LAUNCH:
            self.send_status_text("returning to launch")
            self.acknowledge(command, RESULT_ACCEPTED)
            return

        if command == CMD_DO_SET_MODE:
            # The first parameter is the base mode and the second is the custom mode.
            self.mode = int(values.get("param2", 0.0))
            self.send_status_text(f"the mode is now {self.mode}")
            self.acknowledge(command, RESULT_ACCEPTED)
            return

        self.acknowledge(command, RESULT_TEMPORARILY_REJECTED)

    def acknowledge(self, command, result):
        """Answers a command."""
        self.send(
            77,
            {
                "command": command,
                "result": result,
                "progress": 0,
                "result_param2": 0,
                "target_system": 255,
                "target_component": 190,
            },
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=14550)
    parser.add_argument(
        "--mission-int",
        action="store_true",
        help="ask for the integer form of the mission items",
    )
    arguments = parser.parse_args()

    vehicle = Vehicle(arguments.host, arguments.port, arguments.mission_int)

    try:
        vehicle.run()
    except KeyboardInterrupt:
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
