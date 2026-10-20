#!/usr/bin/env python3
"""A simulated ArduPilot vehicle, for demonstrating and testing the ground station.

It answers over UDP exactly as a vehicle does: a heartbeat every second, and the telemetry
a ground station displays every frame. The MAVLink framing here is written from the
specification, independently of the Rust implementation, so the two agreeing is evidence
that both are right.

    python scripts/simulated-vehicle.py --port 14550
"""

import argparse
import math
import socket
import struct
import sys
import time

STX = 0xFD

# The CRC_EXTRA seed of each message, from the MAVLink dialect.
CRC_EXTRA = {
    0: 50,    # HEARTBEAT
    1: 124,   # SYS_STATUS
    30: 39,   # ATTITUDE
    33: 104,  # GLOBAL_POSITION_INT
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
    crc = crc_accumulate(body, CRC_EXTRA[message_id])

    return bytes([STX]) + body + struct.pack("<H", crc)


class Vehicle:
    """A vehicle that flies a circle and answers heartbeats."""

    def __init__(self, host, port):
        self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.socket.bind((host, port))
        self.socket.settimeout(0.1)
        self.sequence = 0
        self.start = time.monotonic()
        # The ground station that is listening. It is learned from the first frame it
        # sends: a vehicle that only streams telemetry never learns the address otherwise.
        self.client = None

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

        while True:
            # Answer whatever the ground station sends, so the link looks alive from its
            # side too.
            try:
                _, address = self.socket.recvfrom(2048)
                if self.client != address:
                    self.client = address
                    print(f"the ground station is at {address}", flush=True)
            except socket.timeout:
                pass
            except ConnectionResetError:
                # The previous datagram went to a port that was not listening. The ground
                # station that is gone will be heard from again when it returns.
                print("the ground station went away", flush=True)
            except OSError as error:
                print(f"the link failed: {error}", flush=True)
                break

            now = time.monotonic()
            elapsed = now - self.start
            lat, lng, alt = self.position()

            # A heartbeat once a second.
            if now - last_heartbeat >= 1.0:
                last_heartbeat = now
                heartbeat = struct.pack(
                    "<IBBBBB",
                    5,          # custom_mode: GUIDED
                    2,          # MAV_TYPE_QUADROTOR
                    3,          # MAV_AUTOPILOT_ARDUPILOTMEGA
                    0,          # base_mode
                    4,          # MAV_STATE_ACTIVE
                    3,          # mavlink_version
                )
                self.send(frame(0, heartbeat, self.next_sequence()))

            # The attitude, tilted as the vehicle flies the circle.
            roll = math.sin(elapsed * 0.2) * 0.15
            pitch = math.cos(elapsed * 0.3) * 0.05
            yaw = (elapsed * 0.2) % (2 * math.pi)
            attitude = struct.pack(
                "<Iffffff",
                int(elapsed * 1000),
                roll,
                pitch,
                yaw,
                0.0,
                0.0,
                0.0,
            )
            self.send(frame(30, attitude, self.next_sequence()))

            # The position.
            ground_speed = 12.0
            position = struct.pack(
                "<IiiiihhhH",
                int(elapsed * 1000),
                int(lat * 1e7),
                int(lng * 1e7),
                int(alt * 1000),
                int((alt - 584.0) * 1000),
                int(ground_speed * 100),
                0,
                0,
                int(math.degrees(yaw) * 100) % 36000,
            )
            self.send(frame(33, position, self.next_sequence()))

            # The battery, slowly draining.
            remaining = max(0.0, 87.0 - elapsed * 0.05)
            status = struct.pack(
                "<IIIHHhHHhhhhb",
                0, 0, 0,
                0,
                int((12.6 - elapsed * 0.001) * 1000),
                int(-4.2 * 100),
                0,
                0,
                0, 0, 0, 0,
                int(remaining),
            )
            self.send(frame(1, status, self.next_sequence()))

            time.sleep(0.1)

    def send(self, data):
        if self.client is None:
            # No ground station has been heard from yet, so there is nowhere to send.
            return
        try:
            self.socket.sendto(data, self.client)
        except OSError as error:
            print(f"could not send: {error}", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=14550)
    arguments = parser.parse_args()

    vehicle = Vehicle(arguments.host, arguments.port)

    try:
        vehicle.run()
    except KeyboardInterrupt:
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
