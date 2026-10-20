---
name: Add a MAV_CMD to the Mission Editor
description: Use when adding or correcting a MAV_CMD entry in MPCraft — the parameter metadata table, the unit semantics, the frame rules, and the round-trip test that proves it survives upload and download.
metadata:
  opencode/autoinvoke: true
---

# Adding a MAV_CMD to the MPCraft mission editor

A `MAV_CMD` entry tells the mission editor what seven parameters mean. Get it wrong and the
vehicle flies to the wrong place.

## The parameter layout

Every `MAV_CMD` has seven meaningful slots: **P1, P2, P3, P4, X, Y, Z**.

| Slot | Wire field | Common meanings |
| --- | --- | --- |
| P1 | `param1` | pitch, speed, tolerance, radius, direction |
| P2 | `param2` | yaw, distance, altitude offset, empty |
| P3 | `param3` | empty, heading, acceleration |
| P4 | `param4` | yaw, empty, radius |
| X | `x` | latitude (degrees) |
| Y | `y` | longitude (degrees) |
| Z | `z` | altitude (metres) |

**X/Y/Z are not always a position.** For `NAV_TAKEOFF` they are the launch position. For
`CONDITION_DELAY` they are empty. The metadata table states this explicitly.

## The metadata table

`crates/mpcraft-mission/src/mavcmd.toml`

```toml
[mavcmd.NAV_WAYPOINT]
name = "Waypoint"
description = "Navigate to waypoint. X=latitude, Y=longitude, Z=altitude."
params = ["", "", "", "", "lat", "lon", "alt"]
units = ["", "", "", "", "deg", "deg", "m"]
ranges = [0.0, 0.0, 0.0, 0.0, -90.0, -180.0, 0.0]
```

Fields:

| Field | Meaning |
| --- | --- |
| `params` | 7 entries: the role of each slot. Allowed: `lat`, `lon`, `alt`, `speed`, `dist`, `angle`, `empty` |
| `units` | 7 entries: display unit, empty string if unitless |
| `ranges` | 7 entries: `(min, max)`, `0.0` for unbounded |
| `name` | Display name in the editor |
| `description` | Tooltip text |

Upstream's source of truth is `_ref_missionplanner/mavcmd.xml`. Its schema groups commands per
vehicle: `<AC2>` for Copter, `<APM>` for Plane/Ateryx, `<APRover>` for Rover. Port the structure
faithfully.

## Rules

1. **`lat`/`lon`/`alt` slots are lat/lon/alt. Nothing else.** If a command uses X for something
   else, it must be `empty` in the metadata and the value must not be written as a position.

2. **Altitude units are metres** on the wire. If a command's Z is an altitude in a different frame
   (e.g. `NAV_TAKEOFF` in `MAV_FRAME_GLOBAL_RELATIVE_ALT`), the *frame* carries that meaning, not
   the unit field.

3. **`MISSION_ITEM_INT` scales only real coordinates.** Lat/lon are multiplied by `1e7`. A
   non-location command's Z is scaled by `1e4` for int items, and lat/lon only when the command is
   a location command. Upstream handles this at `MAVLinkInterface.cs:3541` — match it exactly.

4. **`MAV_FRAME` matters as much as the values.** `MAV_FRAME_GLOBAL_RELATIVE_ALT` vs
   `MAV_FRAME_GLOBAL` changes the meaning of Z. The editor must let the user pick the frame, and
   the round-trip test must cover both.

5. **Empty slots must round-trip as the value the vehicle sent.** Do not silently normalise
   `0.0` to `NaN` or `f64::NAN` to `0.0`. The vehicle may treat them as meaningful.

## The round-trip test

This is the acceptance criterion for every entry. Add it to
`crates/mpcraft-mission/tests/roundtrip.rs`:

```rust
#[test]
fn nav_waypoint_survives_roundtrip() {
    for frame in [MAV_FRAME::GLOBAL, MAV_FRAME::GLOBAL_RELATIVE_ALT] {
        for use_int in [false, true] {
            let item = MissionItem {
                id: MissionItemId::new(),
                command: MAV_CMD::NAV_WAYPOINT,
                frame,
                params: [0.0, 0.0, 0.0, 0.0],
                x: -35.363261,
                y: 149.165230,
                z: 584.0,
                mission_type: MAV_MISSION_TYPE::MISSION,
                autocontinue: true,
                current: false,
                display_name: None,
            };

            let bytes = encode_item(&item, use_int);
            let decoded = decode_item(&bytes, use_int).unwrap();

            assert_eq!(decoded.command, item.command);
            assert_eq!(decoded.frame, item.frame);
            assert_eq!(decoded.x, item.x, 1e-6);
            assert_eq!(decoded.y, item.y, 1e-6);
            assert_eq!(decoded.z, item.z, 1e-3);
        }
    }
}
```

For a `MAV_CMD` with an unusual layout, add a dedicated case.

## Mission generators

Some commands are only meaningful as part of a generated pattern — a survey grid, a circle, a
spline. If a command participates in one, document the pattern in the metadata `description` so
the UI can offer the generator.

## Checklist

- [ ] `mavcmd.toml` entry added, ported faithfully from `mavcmd.xml`
- [ ] `params`, `units`, `ranges` all have exactly 7 entries
- [ ] Round-trip test added covering both float and int encodings
- [ ] Test covers the relevant `MAV_FRAME` variants
- [ ] Integer scaling verified against upstream's `MAVLinkInterface.cs:3541`
- [ ] Upstream entry located and its vehicle applicability noted
