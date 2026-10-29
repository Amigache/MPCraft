#!/usr/bin/env python3
"""Advertise a vehicle on the local network, so a search has something to find.

A vehicle that speaks MAVLink over UDP can publish `_mavlink._udp.local.` so that a ground station
does not have to be told its address. Mission Planner resolves that service (`MainV2.cs`) and offers
to connect; MPCraft lists what answered and opens the one the operator picks.

This script is the advertiser for the reading, written with `zeroconf` — not the library MPCraft uses
— so a green says two implementations agreed rather than one crate with itself.

    pip install zeroconf
    python scripts/advertise-vehicle.py [--port 14550] [--name SITL]

Nothing has to be listening on the port for the search to find it; point it at a running simulator
when the link is wanted as well.
"""

import argparse
import socket
import sys
import time

from zeroconf import ServiceInfo, Zeroconf

SERVICE_TYPE = "_mavlink._udp.local."


def local_address() -> str:
    """The address this machine answers with on its default route."""

    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

    try:
        # No packet is sent: connecting a datagram socket only chooses a route, which is the
        # address the network knows this machine by.
        probe.connect(("192.0.2.1", 9))

        return probe.getsockname()[0]
    finally:
        probe.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=14550, help="the port its MAVLink is at")
    parser.add_argument("--name", default="Bench", help="the name advertised")
    parser.add_argument("--address", default=None, help="the address to advertise")
    parser.add_argument(
        "--seconds",
        type=float,
        default=0,
        help="stop after this many seconds; zero keeps it registered until stopped",
    )
    arguments = parser.parse_args()

    address = arguments.address or local_address()
    name = f"{arguments.name}.{SERVICE_TYPE}"

    info = ServiceInfo(
        SERVICE_TYPE,
        name,
        addresses=[socket.inet_aton(address)],
        port=arguments.port,
        properties={"name": arguments.name},
        server=f"{socket.gethostname()}.local.",
    )

    zeroconf = Zeroconf()
    zeroconf.register_service(info)

    print(f"advertising {name} at {address}:{arguments.port}", flush=True)
    print("the TXT record says name, which is what the sheet lists", flush=True)

    try:
        if arguments.seconds > 0:
            time.sleep(arguments.seconds)
        else:
            while True:
                time.sleep(1)
    except KeyboardInterrupt:
        pass
    finally:
        zeroconf.unregister_service(info)
        zeroconf.close()

    return 0


if __name__ == "__main__":
    sys.exit(main())
