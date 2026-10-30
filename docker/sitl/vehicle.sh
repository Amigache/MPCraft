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
#
# Both names are written because ArduPilot renamed the parameter: `SYSID_THISMAV` is what the
# metadata still carries and `MAV_SYSID` is what the build reports, and a vehicle that is given the
# name it does not know keeps the id it had — which is 1, the same one as the next container.
parameters=/tmp/vehicle.parm
cat "/ardupilot/Tools/autotest/default_params/$defaults" > "$parameters"
printf 'SYSID_THISMAV %s\n' "$SYSID" >> "$parameters"
printf 'MAV_SYSID %s\n' "$SYSID" >> "$parameters"

# ArduPilot disarms itself ten seconds after arming when it believes it has landed, which on a
# simulator with no radio and no airframe is every time: a board that arms and then goes back to
# disarmed by itself reads as a station that cannot arm it. A bench session is not a flight, and
# the vehicle stays armed until it is told otherwise.
printf 'DISARM_DELAY 0\n' >> "$parameters"

echo "SITL $VEHICLE, system id $SYSID, home $VEHICLE_HOME"

exec "/ardupilot/build/sitl/bin/$binary" \
  -S \
  -I0 \
  --home "$VEHICLE_HOME" \
  --model "$frame" \
  --speedup "$SPEEDUP" \
  --defaults "$parameters"
