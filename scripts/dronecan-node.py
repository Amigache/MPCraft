#!/usr/bin/env python3
"""A DroneCAN node behind a vehicle that forwards its bus, for testing without a CAN bus.

The station reaches the nodes of a vehicle by asking it to forward the bus
(`MAV_CMD_CAN_FORWARD`) and then speaking DroneCAN over `CAN_FRAME` messages. This script is the
other end of that: a MAVLink vehicle on a UDP port, one node on its bus, and the four things a
sheet asks of a node — what it is, its parameters, saving them, and a restart.

The DroneCAN side is written here from the standard's own definitions (the DSDL of each type and the
bit order it prescribes), not from the Rust crate, so a green from the station's area says two
implementations agreed. pydronecan was used only to read the data type signatures, which are facts
about the standard.

    pip install pymavlink
    python scripts/dronecan-node.py --port 14650 --node 42

Point the station at `udp:127.0.0.1:14650` and open the DroneCAN sheet.
"""

import argparse
import socket
import struct
import sys
import time

from pymavlink import mavutil

CAN_FRAME = 386
CAN_FORWARD = 32000
COMMAND_LONG = 76
COMMAND_ACK = 77

# The service ids and signatures of the types this node answers.
GET_NODE_INFO = (1, 0xEEF653B17FE6493C)
RESTART_NODE = (5, 0x72BE70311CD2E2C3)
EXECUTE_OPCODE = (10, 0x38F4821566929FDC)
PARAM_GET_SET = (11, 0xA71A635CF734A0DA)
NODE_STATUS = (341, 0x6CC993F80F6DF80F)

SIGNATURES = {
    GET_NODE_INFO[0]: GET_NODE_INFO[1],
    RESTART_NODE[0]: RESTART_NODE[1],
    EXECUTE_OPCODE[0]: EXECUTE_OPCODE[1],
    PARAM_GET_SET[0]: PARAM_GET_SET[1],
    NODE_STATUS[0]: NODE_STATUS[1],
}

MAGIC_RESTART = 0xACCE551B1E
PRIORITY = 16


def crc16(data, seed=0xFFFF):
    """CRC-16/CCITT-FALSE, the checksum of one multi-frame transfer."""

    crc = seed

    for byte in data:
        crc ^= byte << 8

        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF

    return crc


class Writer:
    """Packs fields the way the standard does: little-endian values, most significant bit first."""

    def __init__(self):
        self.bits = []

    def uint(self, value, width):
        whole, partial = divmod(width, 8)
        data = value.to_bytes(whole + (1 if partial else 0), "little")

        if partial:
            data = data[:-1] + bytes([(data[-1] << (8 - partial)) & 0xFF])

        for index in range(width):
            self.bits.append((data[index // 8] >> (7 - index % 8)) & 1)

    def int(self, value, width):
        self.uint(value & ((1 << width) - 1), width)

    def bytes(self, data):
        for byte in data:
            self.uint(byte, 8)

    def skip(self, count):
        for _ in range(count):
            self.bits.append(0)

    def length(self, count, maximum):
        self.uint(count, maximum.bit_length())

    def finish(self):
        while len(self.bits) % 8:
            self.bits.append(0)

        out = bytearray(len(self.bits) // 8)

        for index, bit in enumerate(self.bits):
            if bit:
                out[index // 8] |= 1 << (7 - index % 8)

        return bytes(out)


class Reader:
    """The other direction of :class:`Writer`."""

    def __init__(self, data):
        self.data = data
        self.position = 0

    def bits_left(self):
        return len(self.data) * 8 - self.position

    def bit(self):
        if self.position >= len(self.data) * 8:
            raise ValueError("the payload ended early")

        value = (self.data[self.position // 8] >> (7 - self.position % 8)) & 1
        self.position += 1

        return value

    def uint(self, width):
        whole, partial = divmod(width, 8)
        data = bytearray(whole + (1 if partial else 0))

        for index in range(width):
            if self.bit():
                data[index // 8] |= 1 << (7 - index % 8)

        if partial:
            data[-1] >>= 8 - partial

        return int.from_bytes(data, "little")

    def int(self, width):
        value = self.uint(width)

        return value - (1 << width) if value & (1 << (width - 1)) else value

    def bytes(self, count):
        return bytes(self.uint(8) for _ in range(count))

    def length(self, maximum):
        return self.uint(maximum.bit_length())

    def skip(self, bits):
        for _ in range(bits):
            self.bit()


def encode_value(writer, kind, value):
    """A parameter value: a three-bit tag and the field it selects."""

    if kind == "empty":
        writer.uint(0, 3)
    elif kind == "integer":
        writer.uint(1, 3)
        writer.int(value, 64)
    elif kind == "real":
        writer.uint(2, 3)
        writer.uint(struct.unpack("<I", struct.pack("<f", value))[0], 32)
    elif kind == "boolean":
        writer.uint(3, 3)
        writer.uint(value, 8)
    else:
        data = value.encode("ascii") if isinstance(value, str) else value
        writer.uint(4, 3)
        writer.length(len(data), 128)
        writer.bytes(data)


def decode_value(reader):
    tag = reader.uint(3)

    if tag == 0:
        return ("empty", None)
    if tag == 1:
        return ("integer", reader.int(64))
    if tag == 2:
        return ("real", struct.unpack("<f", struct.pack("<I", reader.uint(32)))[0])
    if tag == 3:
        return ("boolean", reader.uint(8))

    return ("string", reader.bytes(reader.length(128)).decode("utf-8", "replace"))


class Node:
    """One node on the bus, with the parameters it holds."""

    def __init__(self, node_id, name, parameters):
        self.node_id = node_id
        self.name = name
        self.parameters = parameters
        self.started = time.time()
        self.order = sorted(parameters)

    def node_info(self):
        writer = Writer()
        uptime = int(time.time() - self.started)

        writer.uint(uptime, 32)
        writer.uint(0, 2)  # health: ok
        writer.uint(0, 3)  # mode: operational
        writer.uint(0, 3)  # sub mode
        writer.uint(0, 16)  # vendor status

        writer.uint(1, 8)  # software major
        writer.uint(2, 8)  # software minor
        writer.uint(0, 8)  # flags: no commit, no image hash
        writer.uint(0, 32)
        writer.uint(0, 64)

        writer.uint(4, 8)  # hardware major
        writer.uint(5, 8)  # hardware minor

        for byte in bytes(range(16)):
            writer.uint(byte, 8)

        writer.length(0, 255)  # certificate of authenticity
        writer.bytes(self.name.encode("ascii"))

        return writer.finish()

    def parameter(self, index, requested):
        """The answer to a read or a write, as the standard lays it out."""

        writer = Writer()

        if requested is None:
            # A read by index.
            name = self.order[index] if index < len(self.order) else None

            if name is None:
                writer.uint(0, 3)  # value: empty
            else:
                kind, value = self.parameters[name]
                encode_value(writer, kind, value)

            writer.skip(5)
            writer.uint(0, 3)  # default: empty
            writer.skip(6)
            writer.uint(0, 2)  # max: empty
            writer.skip(6)
            writer.uint(0, 2)  # min: empty
            writer.skip(6)
            writer.bytes(name.encode("ascii") if name else b"")

            return writer.finish()

        kind, value, name = requested

        if name not in self.parameters:
            writer.uint(0, 3)
            writer.skip(5)
            writer.uint(0, 3)
            writer.skip(6)
            writer.uint(0, 2)
            writer.skip(6)
            writer.uint(0, 2)
            writer.skip(6)

            return writer.finish()

        self.parameters[name] = (kind, value)

        current_kind, current = self.parameters[name]

        # The answer carries what the node holds now, which is what a station shows after a write.
        writer = Writer()
        encode_value(writer, current_kind, current)
        writer.skip(5)

        writer.uint(0, 3)
        writer.skip(6)
        writer.uint(0, 2)
        writer.skip(6)
        writer.uint(0, 2)
        writer.skip(6)
        writer.bytes(name.encode("ascii"))

        return writer.finish()


class Bus:
    """The frames the vehicle forwards, and the answers this node puts back."""

    def __init__(self, node, station_id):
        self.node = node
        self.station_id = station_id
        self.pending = {}

    def frames(self):
        """The NodeStatus this node broadcasts, so the bus is not silent."""

        writer = Writer()
        writer.uint(int(time.time() - self.node.started), 32)
        writer.uint(0, 2)
        writer.uint(0, 3)
        writer.uint(0, 3)
        writer.uint(0, 16)

        can_id = (PRIORITY << 24) | (NODE_STATUS[0] << 8) | self.node.node_id

        return split(can_id, 0, writer.finish(), NODE_STATUS[1])

    def receive(self, can_id, data):
        """Reads one frame, and returns the frames to answer with when a request is whole."""

        source = can_id & 0x7F

        if source != self.station_id:
            return []

        service = bool(can_id & (1 << 7))
        request = bool(can_id & (1 << 15))

        if not service or not request:
            return []

        service_id = (can_id >> 16) & 0xFF
        sequence = self.pending.setdefault(source, {"payload": b"", "checksum": None, "toggle": False})

        tail = data[-1]
        start = bool(tail & 0x80)
        end = bool(tail & 0x40)
        toggle = bool(tail & 0x20)
        transfer_id = tail & 0x1F

        body = data[:-1]

        if start and end:
            return self.answer(service_id, transfer_id, body)

        if start:
            sequence["payload"] = body[2:]
            sequence["checksum"] = body[0] | (body[1] << 8)
            sequence["toggle"] = False
            sequence["transfer_id"] = transfer_id
            return []

        if sequence["payload"] == b"" or toggle == sequence["toggle"]:
            return []

        sequence["payload"] += body
        sequence["toggle"] = toggle

        if not end:
            return []

        signature = SIGNATURES[service_id]
        expected = crc16(struct.pack("<Q", signature))
        expected = crc16(sequence["payload"], expected)

        if expected != sequence["checksum"]:
            print("a transfer did not check out", flush=True)
            return []

        payload = sequence["payload"]
        sequence["payload"] = b""

        return self.answer(service_id, transfer_id, payload)

    def answer(self, service_id, transfer_id, payload):
        if service_id == GET_NODE_INFO[0]:
            body = self.node.node_info()
            signature = GET_NODE_INFO[1]
        elif service_id == PARAM_GET_SET[0]:
            reader = Reader(payload)
            index = reader.uint(13)
            kind, value = decode_value(reader)
            name = reader.bytes(reader.bits_left() // 8).decode("ascii", "replace")

            requested = None if (kind == "empty" and not name) else (kind, value, name) if name else None
            body = self.node.parameter(index, requested)
            signature = PARAM_GET_SET[1]
        elif service_id == RESTART_NODE[0]:
            writer = Writer()
            writer.uint(1, 8)
            body = writer.finish()
            signature = RESTART_NODE[1]
        elif service_id == EXECUTE_OPCODE[0]:
            writer = Writer()
            writer.int(0, 48)
            writer.uint(1, 8)
            body = writer.finish()
            signature = EXECUTE_OPCODE[1]
        else:
            return []

        can_id = (PRIORITY << 24) | (service_id << 16) | (self.station_id << 8) | (1 << 7) | self.node.node_id

        return split(can_id, transfer_id, body, signature)


def split(can_id, transfer_id, payload, signature):
    """Cuts a payload into frames, with the checksum in front of a transfer of more than one."""

    if len(payload) <= 7:
        return [(can_id, payload + bytes([0xC0 | (transfer_id & 0x1F)]))]

    checksum = crc16(struct.pack("<Q", signature))
    checksum = crc16(payload, checksum)

    frames = []
    written = 0
    toggle = 0
    first = True

    while written < len(payload):
        data = bytearray()

        if first:
            data += struct.pack("<H", checksum)

        room = 7 - len(data)
        take = min(room, len(payload) - written)
        data += payload[written : written + take]
        written += take

        start = 0x80 if first else 0
        end = 0x40 if written == len(payload) else 0
        data.append(start | end | (toggle << 5) | (transfer_id & 0x1F))

        frames.append((can_id, bytes(data)))

        first = False
        toggle ^= 1

    return frames


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=14650, help="the UDP port the vehicle answers on")
    parser.add_argument("--node", type=int, default=42, help="the DroneCAN node id")
    parser.add_argument("--name", default="com.example.bench", help="the node's name")
    arguments = parser.parse_args()

    network = mavutil.mavlink_connection(
        f"udpin:127.0.0.1:{arguments.port}",
        source_system=1,
        source_component=1,
    )

    node = Node(
        arguments.node,
        arguments.name,
        {
            "BENCH_GAIN": ("integer", 15),
            "BENCH_RATE": ("real", 2.5),
            "BENCH_LABEL": ("string", "bench"),
        },
    )
    bus = Bus(node, station_id=127)

    print(f"the node {arguments.node} answers on udp:127.0.0.1:{arguments.port}", flush=True)
    print(f"a station reaches it by forwarding bus 1", flush=True)

    wanted = socket.gethostname()
    last_heartbeat = 0.0
    last_status = 0.0
    bus_number = 0

    while True:
        message = network.recv_match(blocking=True, timeout=0.2)
        now = time.time()

        if message is not None:
            kind = message.get_type()

            if kind == "COMMAND_LONG" and message.command == CAN_FORWARD:
                bus_number = int(message.param1)
                network.mav.command_ack_send(getattr(message, "command", CAN_FORWARD), 0)
                print(f"forwarding bus {bus_number}", flush=True)
            elif kind == "CAN_FRAME" and int(message.bus) == bus_number and bus_number > 0:
                for can_id, data in bus.receive(int(message.id), bytes(message.data[: message.len])):
                    network.mav.can_frame_send(
                        message.target_system,
                        message.target_component,
                        message.bus,
                        len(data),
                        can_id,
                        data + bytes(8 - len(data)),
                    )

        if now - last_heartbeat >= 1.0:
            last_heartbeat = now
            network.mav.heartbeat_send(
                mavutil.mavlink.MAV_TYPE_GENERIC,
                mavutil.mavlink.MAV_AUTOPILOT_ARDUPILOTMEGA,
                0,
                0,
                0,
                mavutil.mavlink.MAV_STATE_ACTIVE,
            )

        if bus_number and now - last_status >= 1.0:
            last_status = now

            for can_id, data in bus.frames():
                network.mav.can_frame_send(
                    255,
                    0,
                    bus_number,
                    len(data),
                    can_id,
                    data + bytes(8 - len(data)),
                )


if __name__ == "__main__":
    sys.exit(main())
