---
name: Run SITL Integration Tests
description: Use when writing or running integration tests against an ArduPilot SITL simulator, or when a change needs end-to-end verification with a live vehicle.
metadata:
  opencode/autoinvoke: true
---

# Running SITL integration tests

Unit tests prove logic. SITL proves the whole stack works against a real autopilot.

## Why SITL

- Deterministic, repeatable, no hardware needed.
- It is the only way to verify mission upload, parameter write, and mode changes end-to-end.
- A live vehicle adds what a scripted peer cannot imitate: its own values, its own encoding, its own
  streaming, and its own decisions — including refusals, which are the vehicle's answer and not a failure
  of this code.

Two ways to have one, and they are not interchangeable:

| | CI | This machine |
| --- | --- | --- |
| Vehicle | `ghcr.io/ardupilot/sitl` container | Mission Planner's local `sitl\ArduCopter.exe` |
| Address | UDP `127.0.0.1:14550` | TCP `127.0.0.1:5760 + 10 × instance` |
| Connection | one per test area | **one per run** — the process exits when its client goes away |
| Parameters | `Parameters.parm` in the working dir | `default_params\copter.parm`, plus a fresh `eeprom.bin` per directory |

## Starting SITL — the container (CI)

```bash
docker pull ghcr.io/ardupilot/sitl:latest

docker run --rm -it \
  -e HOME=/tmp \
  --name mpcraft-sitl \
  -p 14550:14550 \
  ghcr.io/ardupilot/sitl:latest \
  /bin/sh -c "cp -r /home/ardupilot/ardupilot /tmp/ardupilot && cd /tmp/ardupilot && \
    Tools/autotest/default_params.py Copter > /tmp/ardupilot/Parameters.parm && \
    Tools/autotest/sim_vehicle.py -v ArduCopter -D --out=UDP:127.0.0.1:14550 --model + --speedup 1"
```

SITL needs a `SITL_WORKING_DIR` and a writable parameters file; the container handles both. The stream comes
from `127.0.0.1:14550`, and `sim_vehicle.py` also exposes a simulator control port on 5501 — which is what
lets a test feed the radio check.

## Starting SITL — the local binaries (this machine)

Mission Planner's build of SITL does not take a device string for UDP, so it is reached over TCP on the port
it reserves per instance. `scripts/sitl.ps1` starts one instance in a working directory of its own, because the
binary writes its parameter state (`eeprom.bin`) and the terrain cells it fetches wherever it was started from:
started from the repository, the state of one run is where the next run reads it.

```powershell
.\scripts\sitl.ps1 -Instance 5            # clears the state, prints tcp:127.0.0.1:5810
$env:MPCRAFT_SITL_TARGET = "tcp:127.0.0.1:5810"
cargo +stable-x86_64-pc-windows-gnu test -p mpcraft-link --features sitl-tests --test sitl -- --nocapture --test-threads=1
```

Three things about this build that change how a suite is written against it:

- **The port is `5760 + 10 × instance`.** Two instances need two addresses; a test that hardcodes one cannot
  run two vehicles.
- **It binds only the telemetry port**, not the RC input port (`5501 + 10 × instance`). Nothing travels
  through the radio, so a pre-arm check that reads the radio cannot be fed and must be said as uncovered
  rather than asserted.
- **It exits when the client disconnects**, so one run holds one connection and walks through its areas
  inside it — opening a connection per area takes the vehicle down after the first one. Run the suite with
  `--test-threads=1`. The same rule decides how many instances a run needs: an instance serves one connection,
  so two live test binaries need two instances, and the second one finds no peer otherwise — a failure that
  belongs to the environment rather than to the code. The gate reads each live area against its own address,
  `MPCRAFT_SITL_TARGET` and `MPCRAFT_SITL_TABLE_TARGET`, and names which one answered which.

The state is what carries a value between two runs, which is why a fresh instance must not inherit it. Three
runs on the same instance showed the rule firing: a write of `THR_DZ` came back as **7.0** when 7.5 was asked
for — the vehicle's value, not the one requested — the next run with `-KeepState` read the same 7.0 back, and
the run after it, which cleared the state, read the default. `-KeepState` is for when the state of a particular
flight is what is being looked at, which is rare enough to be an explicit choice.

## Manual verification

```bash
# Connect and watch telemetry
cargo run -p mpcraft-cli -- connect udp:127.0.0.1:14550 --monitor

# List parameters
cargo run -p mpcraft-cli -- connect udp:127.0.0.1:14550 -- params list

# Read one parameter
cargo run -p mpcraft-cli -- connect udp:127.0.0.1:14550 -- params get ARMING_CHECK

# Write a parameter
cargo run -p mpcraft-cli -- connect udp:127.0.0.1:14550 -- params set ARMING_CHECK 1
```

## Writing an integration test

Integration tests live in `crates/mpcraft-link/tests/` and are gated behind a feature flag so
`cargo test --workspace` does not require a simulator — Docker or otherwise.

```rust
#![cfg(feature = "sitl-tests")]

#[tokio::test]
async fn uploads_mission_and_reads_it_back() {
    // The address comes from MPCRAFT_SITL_TARGET in a real run; it is written here to show the shape.
    let session = Session::connect(ConnectionSpec::udp("127.0.0.1:14550")).await.unwrap();
    session.wait_for_vehicle(Duration::from_secs(30)).await.unwrap();

    let mission = MissionItem::takeoff(0.0, 0.0, 30.0);
    session.upload_mission(&[mission]).await.unwrap();

    let read_back = session.download_mission().await.unwrap();
    assert_eq!(read_back.len(), 1);
    assert_eq!(read_back[0].command, MAV_CMD::NAV_TAKEOFF);
}
```

Run them with:

```bash
docker compose -f infra/sitl/docker-compose.yml up -d
cargo test --workspace --features sitl-tests
```

Against the local binaries the address comes from `MPCRAFT_SITL_TARGET` rather than being hardcoded, so the same
suite answers against either build:

```powershell
$env:MPCRAFT_SITL_TARGET = "tcp:127.0.0.1:5760"
cargo +stable-x86_64-pc-windows-gnu test -p mpcraft-link --features sitl-tests --test sitl -- --nocapture --test-threads=1
```

Set `MPCRAFT_SITL_LIVE=1` when a vehicle is really on the wire. `scripts/run-gates.js` says which one answered
the gate: without it a scripted vehicle written from the specification stands in, and the gate reports
`SUSTITUIDA`, because that stand-in covers the protocols but not timing, a radio, or a board. A green that rests
on a stand-in is not a pass against a vehicle. When a vehicle does answer, the gate shows the report of the live
binary rather than the first count the run prints, which is the count of the unit tests and says nothing about
the vehicle.

## What to verify in SITL

| Area | Test |
| --- | --- |
| Connect | Heartbeat received, vehicle type detected as `ArduCopter` |
| Telemetry | Position, attitude, and battery update within 2 s of connect |
| Parameters | Full list downloads; a written value reads back identically |
| Mission | Upload → download → byte-identical round trip |
| Modes | Mode change is reflected in telemetry within 1 s |
| Commands | `COMMAND_LONG` for takeoff returns `MAV_RESULT_ACCEPTED` |
| Reconnect | Killing SITL surfaces a disconnect; restarting it triggers reconnect |

Two areas need a live vehicle rather than a scripted one, and they read as the vehicle's answer:

- **Arming** is refused or accepted by the board. With Mission Planner's defaults `ARMING_CHECK` is 1 — the full
  checks — and a fresh simulator refuses for its own reason while printing it on the wire ("Arm: Gyros
  inconsistent", "Arm: Need Position Estimate"). Assert that the answer arrives as the vehicle's own, kept apart
  from a board that never answered; prove acceptance by writing the check off and polling, not by asserting a
  duration, because the estimate the check reads is still the vehicle's decision. Then put the parameter back as
  it was found.
- **Sequence 0 of a plan** is not an item: the specification puts the home position there, so whatever was
  uploaded is replaced on read-back by the home, in the absolute frame above mean sea level. Compare the items
  from sequence 1 onwards and assert sequence 0 as the home.

What cannot be asserted against this build is said rather than assumed: the disconnect, because a test that
connects to a peer cannot stop that peer, and the radio check, because these builds do not bind the RC input
port. A command that answers with a refusal in every state tried — `DO_CHANGE_SPEED` did — asserts how the
refusal is read, not an acceptance this build does not give.

## Timeouts

SITL is slower than real hardware but not instant. Use generous timeouts (30 s for connect,
5 s for transactions) and **never** assert on wall-clock timing tighter than that. A CI machine
under load will fail a flaky timing assertion and teach everyone to ignore the suite.

A state the vehicle decides for itself has no duration that can be asserted — an arming, an EKF that has to
converge, a message that arrives on a rate. Poll with a bounded deadline instead: the deadline says what the
machine is allowed to take, and the assertion says what the vehicle said. The recorded runs show why: the same
check refused twenty times and accepted on the twenty-first.

## Cleanup

Always stop the container, even on test failure:

```bash
docker compose -f infra/sitl/docker-compose.yml down -v
```

The local build leaves its own traces in the directory it was started from — `eeprom.bin` and the terrain cells
it fetched (`terrain/S36E149.DAT`). They are the state of the previous run, so a fresh instance must not inherit
them; `scripts/sitl.ps1` keeps them in a directory per instance outside the repository and clears them before
starting, which is why they are ignored rather than committed.

## Red flags

- An integration test is flaky on repeated runs — it is asserting on timing, not behaviour.
- A test needs a hardcoded real-world vehicle state.
- The suite requires Docker for `cargo test --workspace` — it must be behind a feature flag.
- A live assertion claims an acceptance this build does not give, or reads a refusal as a failure of this code
  rather than as the board's own answer.
- The target is hardcoded instead of read from `MPCRAFT_SITL_TARGET`, so the same suite cannot answer against
  both builds; or the gate reports a stand-in as though a vehicle had answered it.
- One connection per area against the local binaries, which take the vehicle down after the first one.
