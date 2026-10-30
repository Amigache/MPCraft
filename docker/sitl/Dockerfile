# ArduPilot SITL, built from the published source rather than taken from an image of unknown age.
#
# The image this replaces carried ArduPilot 4.0.3, from 2020: it refuses to answer a request for its
# own version, which is the kind of gap that makes a station look wrong when the vehicle is what is
# old. This builds the tag named below from ArduPilot's own repository — the same source the boards
# are flashed from — and copies only what the simulator needs into a small runtime.
#
#   docker build -t mpcraft/sitl:4.7.2 docker/sitl
#
# The build takes a few minutes and a couple of gigabytes: it clones the source with its modules and
# compiles the SITL binary. The result is one binary per vehicle type and the parameter files of
# their frames.

FROM ardupilot/ardupilot-dev-base:latest AS build

# The release the simulator runs. ArduPilot tags a vehicle and a version together, and the tag is
# what a board is flashed from, so the simulator and the board can be the same code.
ARG ARDUPILOT_REF=Copter-4.7.2

RUN git clone --depth 1 --branch "${ARDUPILOT_REF}" \
      --recurse-submodules --shallow-submodules \
      https://github.com/ArduPilot/ardupilot.git /ardupilot

WORKDIR /ardupilot

# Every vehicle the station can drive. One build produces all of them: the compile is the expensive
# part, and a plane and a quadcopter are the same binary with a different frame.
RUN ./waf configure --board sitl \
 && ./waf copter \
 && ./waf plane \
 && ./waf rover \
 && ./waf sub

FROM debian:bookworm-slim AS runtime

# What the simulator binary links against, and nothing else: no build toolchain, no python, no
# network tooling. The binary is the whole simulator.
RUN apt-get update \
 && apt-get install -y --no-install-recommends libstdc++6 ca-certificates \
 && rm -rf /var/lib/apt/lists/*

COPY --from=build /ardupilot/build/sitl/bin/ /ardupilot/build/sitl/bin/
COPY --from=build /ardupilot/Tools/autotest/default_params/ /ardupilot/Tools/autotest/default_params/

COPY vehicle.sh /sitl/vehicle.sh

ENTRYPOINT ["/bin/sh", "/sitl/vehicle.sh"]
