---
name: Port a Mission Planner Feature
description: Use when implementing a specific Mission Planner feature in MPCraft from the parity matrix — how to read the upstream C#, decide what to keep, and verify parity.
metadata:
  opencode/autoinvoke: true
---

# Porting a Mission Planner feature to MPCraft

Every feature has a row in `docs/PARITY.md`. This is how you implement one.

## 1. Find the upstream implementation

```bash
rg -n "FeatureName" _ref_missionplanner/
```

Upstream layout:

| Location | Contains |
| --- | --- |
| `_ref_missionplanner/GCSViews/` | The main screens (`FlightData.cs` 281 KB, `FlightPlanner.cs` 336 KB) |
| `_ref_missionplanner/GCSViews/ConfigurationView/` | ~80 `Config*.cs` hardware pages |
| `_ref_missionplanner/ExtLibs/ArduPilot/` | Vehicle domain, link logic |
| `_ref_missionplanner/ExtLibs/ArduPilot/Mavlink/` | `MAVLinkInterface.cs` (5808 lines) — the core |
| `_ref_missionplanner/ExtLibs/Mavlink/` | MAVLink protocol |
| `_ref_missionplanner/ExtLibs/Comms/` | Transports |
| `_ref_missionplaver/ExtLibs/Utilities/` | Geodesy, terrain, params, settings |
| `_ref_missionplanner/Log/` | Log browser and download |
| `_ref_missionplanner/Controls/`, `ExtLibs/Controls/` | Custom widgets |
| `_ref_missionplanner/Plugin/`, `Plugins/` | Plugin system |

Upstream files are huge. **Search by symbol, do not read top to bottom.** Use `rg` with the method
name.

## 2. Separate behaviour from implementation

Ask of each upstream piece: *what does the user see, and what does the vehicle do?*

| Keep | Change |
| --- | --- |
| The protocol sequence | The concurrency model |
| The validation rules | The UI framework |
| The file formats | The error handling style |
| The parameter semantics | The state management |
| The defaults and thresholds | The visual layout |

**Do not port the layout.** WinForms `DataGridView` event handlers do not translate to React.

## 3. Identify the upstream traps

Upstream has known sharp edges. Do not reproduce them.

| Trap | Where | What to do instead |
| --- | --- | --- |
| `giveComport` cooperative mutex | `MAVLinkInterface` | Actor model with per-transaction waiters |
| `CurrentState` computed properties | `CurrentState.cs` | Push-update snapshot |
| `MainV2` static globals | `MainV2.cs` | Explicit `AppState` |
| `#if LIB` conditional compilation | throughout | Platform traits |
| Linear `MAVLINK_MESSAGE_INFOS` scan | `MavlinkUtil.cs` | Generated lookup |
| Byte-at-a-time reads | `readPacketAsync` | Buffered parser |
| Blocking UI during long ops | `ProgressReporterDialogue` | Async commands with progress events |
| Silent failures | pervasive | Typed errors with stable codes |

## 4. Decide the layering

| Feature kind | Rust crate | Frontend |
| --- | --- | --- |
| Protocol operation | `mpcraft-link` | calls a Tauri command |
| Domain model | `mpcraft-mission`, `-vehicle`, `-params`, `-geo` | renders it |
| File format | `mpcraft-mission`, `-log` | file dialog + command |
| Pure computation | `mpcraft-geo`, `-mission` | — |

**The frontend must never need protocol knowledge.** If the UI needs to know a `MAV_CMD` integer
or a message id, the design is wrong — expose an intent-level operation instead.

## 5. Implement in this order

1. **Core logic in Rust**, with tests, no UI.
2. **Expose it** through `mpcraft-app` as a typed command or event.
3. **Add the Tauri command** in `apps/desktop`.
4. **Build the UI** on top of the typed contract.
5. **Update the parity matrix** — same change, never later.

## 6. Verify parity

Parity is not "the button exists". Verify:

- [ ] The behaviour matches upstream, including edge cases and error paths.
- [ ] The data files are interoperable — a mission written by upstream opens in MPCraft and vice
      versa.
- [ ] The protocol traffic matches upstream's sequence, byte for byte where it matters.
- [ ] Failure modes are *better*: upstream often silently swallows errors. MPCraft surfaces them.

For protocol features, capture upstream's actual traffic and use it as a golden file.

## 7. Report

State plainly:

- What upstream behaviour you matched.
- What you improved, and why.
- What you deliberately dropped, and the rationale (and update the matrix).

## Checklist

- [ ] Upstream implementation located and understood
- [ ] Rust core implemented with tests
- [ ] Exposed via `mpcraft-app`
- [ ] Tauri command added with typed errors
- [ ] Frontend implemented without protocol knowledge
- [ ] Interoperability verified against real data files
- [ ] `docs/PARITY.md` row updated in the same change
