#!/bin/sh
# Runs one ArduPilot SITL instance.
#
# The image this runs in carries ArduPilot built for the simulator, and a container is one vehicle:
# the binary listens on its own TCP port inside the container, so every vehicle can use the same
# port number and the router on the outside is what joins them into one link.
#
# What decides which vehicle a station is hearing is the system id, not the port: two instances
# that both call themselves 1 are one vehicle with two bodies on the wire, and every packet would
# be attributed to whichever spoke last. `SYSID` is therefore not a detail here.

set -eu

VEHICLE="${VEHICLE:-copter}"
SYSID="${SYSID:-1}"
VEHICLE_HOME="${VEHICLE_HOME:--35.363261,149.165230,584,353}"
SPEEDUP="${SPEEDUP:-1}"
FRAME="${FRAME:-}"

case "$VEHICLE" in
  copter)
    binary=arducopter
    defaults=copter.parm
    frame="${FRAME:-+}"
    ;;
  plane)
    binary=arduplane
    defaults=plane.parm
    frame="${FRAME:-plane}"
    ;;
  rover)
    binary=ardurover
    defaults=rover.parm
    frame="${FRAME:-rover}"
    ;;
  sub)
    binary=ardusub
    defaults=sub.parm
    frame="${FRAME:-vectored}"
    ;;
  *)
    echo "the vehicle $VEHICLE is not one this image carries" >&2
    exit 1
    ;;
esac

# The parameters of the frame, and the identity this instance reports, in one file: the binary
# reads a single defaults file, and the system id is a parameter like any other.
parameters=/tmp/vehicle.parm
cat "/ardupilot/Tools/autotest/default_params/$defaults" > "$parameters"
printf 'SYSID_THISMAV %s\n' "$SYSID" >> "$parameters"

echo "SITL $VEHICLE, system id $SYSID, home $VEHICLE_HOME"

exec "/ardupilot/build/sitl/bin/$binary" \
  -S \
  -I0 \
  --home "$VEHICLE_HOME" \
  --model "$frame" \
  --speedup "$SPEEDUP" \
  --defaults "$parameters"
