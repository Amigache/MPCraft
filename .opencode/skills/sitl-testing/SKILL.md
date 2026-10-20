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
- The same container runs in CI and locally.
- It is the only way to verify mission upload, parameter write, and mode changes end-to-end.

## Prerequisites

```bash
docker pull ghcr.io/ardupilot/sitl:latest
```

SITL needs a `SITL_WORKING_DIR` and a writable parameters file. The container handles both.

## Starting SITL

```bash
docker run --rm -it \
  -e HOME=/tmp \
  --name mpcraft-sitl \
  -p 14550:14550 \
  ghcr.io/ardupilot/sitl:latest \
  /bin/sh -c "cp -r /home/ardupilot/ardupilot /tmp/ardupilot && cd /tmp/ardupilot && \
    Tools/autotest/default_params.py Copter > /tmp/ardupilot/Parameters.parm && \
    Tools/autotest/sim_vehicle.py -v ArduCopter -D --out=UDP:127.0.0.1:14550 --model + --speedup 1"
```

The vehicle is now listening on UDP 14550. The MAVLink stream comes from
`127.0.0.1:14550`; `sim_vehicle.py` also exposes a simulator control port on 5501.

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
`cargo test --workspace` does not require Docker.

```rust
#![cfg(feature = "sitl-tests")]

#[tokio::test]
async fn uploads_mission_and_reads_it_back() {
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

## Timeouts

SITL is slower than real hardware but not instant. Use generous timeouts (30 s for connect,
5 s for transactions) and **never** assert on wall-clock timing tighter than that. A CI machine
under load will fail a flaky timing assertion and teach everyone to ignore the suite.

## Cleanup

Always stop the container, even on test failure:

```bash
docker compose -f infra/sitl/docker-compose.yml down -v
```

## Red flags

- An integration test is flaky on repeated runs — it is asserting on timing, not behaviour.
- A test needs a hardcoded real-world vehicle state.
- The suite requires Docker for `cargo test --workspace` — it must be behind a feature flag.
