#!/usr/bin/env python3
"""Advertise a camera on the local network, so a search has something to find.

The discovery MPCraft carries asks the local network which cameras publish RTSP over DNS-SD, and
the only way to have an answer is to have a camera (or something that behaves like one) on that
network. This script is that something: it registers one `_rtsp._udp` service with the records a
real camera publishes, and keeps it registered until it is stopped.

It is written with `zeroconf`, which is not the library MPCraft uses, so a green from the area that
reads it says the two agreed rather than that one crate agreed with itself. The service type, the
properties and the shape of the resolutions are the ones Mission Planner reads
(`ExtLibs/Utilities/ZeroConf.cs`, `Controls/Video.cs`).

    pip install zeroconf
    python scripts/advertise-camera.py [--port 8554] [--name "MPCraft Test Camera"]

The address the service is registered with is the machine's own, which is what a camera on the same
network would answer with. The port does not have to have a camera behind it for the discovery to
find it; point it at a running `mediamtx` when the picture is wanted as well.
"""

import argparse
import socket
import sys
import time

from zeroconf import ServiceInfo, Zeroconf

SERVICE_TYPE = "_rtsp._udp.local."


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
    parser.add_argument("--port", type=int, default=8554, help="the port the stream is at")
    parser.add_argument("--name", default="MPCraft Test Camera", help="the name advertised")
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
        properties={
            "name": arguments.name,
            "path": "/axis-media/media.amp",
            # A camera of the kind upstream draws names the codec and the sizes it can send.
            "codecs": "H264(1920x1080,1280x720)",
        },
        server=f"{socket.gethostname()}.local.",
    )

    zeroconf = Zeroconf()
    zeroconf.register_service(info)

    print(f"advertising {name} at {address}:{arguments.port}", flush=True)
    print("the TXT record says name, path and H264(1920x1080,1280x720)", flush=True)

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
