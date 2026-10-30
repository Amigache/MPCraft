# SITL — ArduPilot in the loop

A real ArduPilot simulator, not a stand-in: the flight code, the arming checks, the mission
protocol and the parameter table are ArduPilot's own, built from its published source at the tag in
`Dockerfile`. One container is one vehicle; the router in the compose joins them into one link, so
the station sees three vehicles on one socket the way it sees three boards on a shared radio.

## What it is not

The scripted vehicle in `scripts/simulated-vehicle.py` answers MAVLink from the specification and
is useful for the transport and the framing, but it is not a flight controller: it has no arming
checks, no estimators and no mission semantics. This is what to use when the question is about what
the vehicle does rather than about the bytes it sends.

## Running it

```sh
# the first time, or after changing the ArduPilot tag: a few minutes
docker compose -f docker/sitl/compose.yaml build

docker compose -f docker/sitl/compose.yaml up -d
docker compose -f docker/sitl/compose.yaml logs -f sitl-1
```

In the window, connect with **UDP** to `127.0.0.1:14550`. That address is the router: every vehicle
is on it, and the connection sheet lists all of them.

A single vehicle can also be reached directly, without the router, on `tcp:127.0.0.1:5760`
(`sitl-1`), `tcp:127.0.0.1:5761` (`sitl-2`) and `tcp:127.0.0.1:5762` (`sitl-3`).

Stop it with:

```sh
docker compose -f docker/sitl/compose.yaml down
```

## Adding a vehicle

Copy one `sitl-*` block, give it a new `SYSID`, a `VEHICLE_HOME` a few hundred metres from the
others (three vehicles at one place are one place reported three times), and a new published TCP
port. Then add `tcpc:sitl-N:5760` to the router's command.

```yaml
  sitl-4:
    image: mpcraft/sitl:4.7.2
    environment:
      VEHICLE: plane
      SYSID: "4"
      VEHICLE_HOME: "-35.359000,149.170000,584,353"
    volumes:
      - ./vehicle.sh:/sitl/vehicle.sh:ro
    ports:
      - "5763:5760/tcp"
```

`VEHICLE` is one of `copter`, `plane`, `rover` or `sub`; `FRAME` overrides the default frame of the
type (`+` for a quadcopter, `plane`, `rover`, `vectored`).

## What the system id is for

A station tells vehicles apart by the pair `(system_id, component_id)`, not by the port they arrive
on. Two instances that both call themselves 1 are one vehicle with two bodies on the wire: the
state of one overwrites the state of the other, and every command goes to whichever answered first.
`vehicle.sh` sets `SYSID_THISMAV` per container for that reason.

The router is a ground station on the link and announces itself as one. MPCraft does not count a
station as a vehicle — they are not one, and they carry no vehicle state — which is what keeps
"three vehicles" meaning three.

## Limits

- The build pins `Copter-4.7.2`; changing `ARDUPILOT_REF` builds whatever that tag holds, which is
  how the simulator is kept on the same code as the board.
- The simulator runs without a graphics front end and without a physics model in the loop: ArduPilot
  flies its internal model. What it does not reproduce is what a real airframe does — wind, vibration,
  a failing GPS. For those, the same image accepts ArduPilot's own `--model` options.
- Nothing here is a substitute for a board on the bench when the question is about the board: a
  WinUSB device, a bootloader or a serial adapter cannot be simulated.
