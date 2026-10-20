---
name: Add MAVLink Message Support
description: Use when adding support for a new MAVLink message, MAV_CMD, or protocol capability to MPCraft — covers the Rust bindings, telemetry decoding, mission metadata, and frontend surfacing.
metadata:
  opencode/autoinvoke: true
---

# Adding MAVLink message support to MPCraft

Adding a MAVLink message touches five layers. Work through them in order — skipping one is how a
message ends up half-supported.

## 1. Confirm the message exists in the dialect

MPCraft uses the upstream `mavlink` crate with `dialect-ardupilotmega` and `dialect-common`.

```bash
cargo add mavlink --features dialect-ardupilotmega
```

Check the message is in the generated bindings:

```bash
rg "HEARTBEAT" ~/.cargo/registry/src/*/mavlink-0.20.0/src/ardupilotmega.rs
```

If the message is not in the dialect, it belongs in a newer XML revision. Options:

- Bump the `mavlink` crate version.
- Add the XML to a local dialect and regenerate via `mavlink-bindgen`.

**Do not hand-write a message struct.** The generated bindings carry the correct CRC_EXTRA seed,
field order, and extensions. A hand-written struct will have a wrong CRC and corrupt every frame
of that type.

## 2. Decode it into the telemetry model

`crates/mpcraft-vehicle/src/telemetry.rs`

Add the field to the `Telemetry` struct and update it in the packet handler. Follow the existing
pattern: the handler matches on the message, extracts fields, and mutates the snapshot.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vibration {
    pub gyro: Vector3,
    pub accel: Vector3,
}
```

Rules:

- Decode **once per packet**, never on access.
- Give the field a meaningful unit in its name or type (`_m`, `_ms`, `_deg`, `_mps`).
- If the value can go stale, record its age. The UI greys out stale fields.

## 3. Register it in the packet inspector

`crates/mpcraft-mavlink/src/inspector.rs` — the packet inspector shows per-message-id rates and
byte counts. New messages must appear there.

## 4. If it is a mission command, extend the metadata

`crates/mpcraft-mission/src/mavcmd.toml`

A `MAV_CMD` entry declares which parameters mean what:

```toml
[mavcmd.NAV_TAKEOFF]
name = "Takeoff"
params = ["pitch", "empty", "empty", "yaw", "lat", "lon", "alt"]
units = ["deg", "", "", "deg", "deg", "deg", "m"]
ranges = [0.0, 0.0, 0.0, 0.0, -90.0, -180.0, 0.0]
```

The `params` array has 7 entries matching P1-P4, X, Y, Z. The frontend generates editors from this
— **no per-command TypeScript**.

Upstream's schema is `_ref_missionplanner/mavcmd.xml`. Port the entry faithfully.

## 5. Surface it in the UI

The frontend must not know the message ID. Expose an **intent-level** command or a named field in
the telemetry payload.

- Telemetry field → add to the serialised `Telemetry` struct and the TypeScript type.
- User action → add a Tauri command with a typed request/response.

Then add it to a UI location:

- A readout → the flight data quick-view or a telemetry panel.
- A graph channel → the channel registry.
- A log preset → `crates/mpcraft-log` graph presets.

## 6. Test it

- Golden test: a real captured frame of this message type decodes to the expected values.
- Telemetry test: the handler updates the field.
- Round-trip: for a `MAV_CMD`, serialise → parse → serialise is byte-identical.

## Checklist

- [ ] Message exists in the generated dialect bindings
- [ ] Field added to `Telemetry` with correct units
- [ ] Packet inspector registers the message id
- [ ] `mavcmd.toml` updated (if it is a mission command)
- [ ] Frontend surfaces it without knowing the message id
- [ ] Golden test added with a real captured frame
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
