# MPCraft — Development Plan

**Status:** Planning complete, scaffolding in place.
**Reference implementation:** `ardupilot/MissionPlanner` (cloned read-only at `_ref_missionplanner/`)
**Target:** Windows, Linux, macOS (v1)

---

## 1. Objective & non-goals

### Objective

Deliver a Ground Control Station that provides **the same functional capability** as Mission Planner
for ArduPilot and PX4 vehicles, implemented with a modern architecture and stack, and shipping in
usable increments rather than as a single big-bang rewrite.

### Non-goals

| Not doing | Why |
| --- | --- |
| Porting C# line-by-line | The legacy design has structural debt that must not be reproduced |
| Reproducing WinForms visual style | We keep *capability*, not *appearance* |
| 1:1 plugin ABI compatibility | C# plugin assemblies cannot be loaded by Rust; we define a new sandboxed model |
| Supporting every peripheral in v1 | Radio flashing, DroneCAN tooling and camera grids land in later phases |
| Bundling a firmware flasher in phase 1 | High risk, needs USB DFU; scheduled separately |

---

## 2. Design principles

1. **One reader, many consumers.** A single task owns the byte stream. It parses frames and
   publishes them on a `tokio::broadcast` channel. Request/response transactions subscribe and
   filter. *This is the single biggest departure from Mission Planner*, which has both a background
   `SerialReader` and every transaction competing for the same socket behind a `giveComport`
   semaphore.
2. **State is a snapshot, not a computation.** `CurrentState` in Mission Planner exposes ~400
   properties that each re-derive their value from `getPacketLast(msgid)` on every access — the UI
   re-reads them at 10 Hz. MPCraft decodes once per packet into an `ArcSwap<Telemetry>` snapshot.
3. **Pure Rust core, thin shell.** Everything except `mpcraft-app` is UI-free and headless-testable.
   This is what makes a CLI and CI possible.
4. **Typed protocol, no stringly-typed dispatch.** Message IDs, `MAV_CMD`, `MAV_FRAME` and
   `MAV_PARAM_TYPE` come from the generated `mavlink` crate, not from hand-written constants.
5. **Fail loudly at the boundary.** Every transaction has a timeout, a retry policy, and a typed
   error. Silent failures are the legacy system's worst property.
6. **Test the wire, not the mock.** Protocol tests use real captured byte sequences from Mission
   Planner's own test corpus and SITL, not hand-written expectations.

---

## 3. Technology decisions

| Layer | Choice | Rationale |
| --- | --- | --- |
| Language | Rust 2021, MSRV 1.88 | Memory safety without GC; `tokio` for async I/O |
| MAVLink | `mavlink` v0.20 (`mavlink-core`) | Upstream-generated bindings from the same XML dialects; already has CRC, framing, signing, and TCP/UDP/serial transports |
| Async runtime | `tokio` | Industry standard; `broadcast`/`oneshot` fit the transaction model exactly |
| Desktop shell | Tauri v2 | Small binaries, native WebView, first-class Rust backend |
| Frontend | React 19 + TypeScript + Vite | Type safety end-to-end, huge ecosystem for the dense data UIs a GCS needs |
| Maps | MapLibre GL JS | Vector/raster tiles, no API key required for OSM, GPU-accelerated, actively maintained |
| Graphs | uPlot | Fastest-in-class time-series rendering; handles high-rate telemetry at 60 fps |
| Data grid | TanStack Table + TanStack Virtual | Virtualised rows for large parameter sets and log samples |
| Geodesy | `geo` + `geo-types` + `proj` | Well-tested geometry and projection crates |
| Terrain | `reqwest` + `image` + custom HGT/HGT-style reader | SRTM via the ArduPilot terrain service, DTED locally |
| State | `ArcSwap` / `DashMap` | Lock-free reads on the UI hot path |
| Errors | `thiserror` (libs) + `anyhow` (binaries) | Typed errors across crate boundaries |
| Logging | `tracing` + `tracing-subscriber` | Structured, filterable, spans for request tracing |

### Deliberate departures from upstream

| Upstream | MPCraft | Reason |
| --- | --- | --- |
| `MAVLINK_MESSAGE_INFOS` linear scan | Generated lookup tables | Upstream is O(n) on a hot path |
| `readPacketAsync` byte-at-a-time | Buffered `tokio` read + state-machine parser | Upstream reads one byte per syscall |
| CRC-failure resync by scanning last 20 bytes | Explicit parser state machine | Deterministic and testable |
| `CurrentState` computed properties | `Telemetry` snapshot struct | Removes UI-coupled recomputation |
| `giveComport` cooperative mutex | Per-transaction `oneshot` channels | Eliminates the global bottleneck |
| Plaintext sniffing for diagnostics | Preserved as an explicit `LinkDiagnostic` channel | Genuinely useful for debugging boards |
| `mavcmd.xml` column definitions | `mavcmd.toml` in the same schema | Same data, modern format |
| `ParameterMetaData.xml` | Same XML, parsed natively | Avoids a 4 MB blob being re-parsed per lookup |
| C# Roslyn plugin compilation | WASM plugin sandbox (phase 6) | Language-neutral, memory-safe |

---

## 4. Workspace layout

```
MPCraft/
├── Cargo.toml
├── crates/
│   ├── mpcraft-mavlink/    # framing, CRC, signing, dialect re-exports, packet inspection
│   ├── mpcraft-transport/  # Transport trait; serial, UDP, TCP, file, NTRIP
│   ├── mpcraft-vehicle/    # Vehicle, Telemetry snapshot, derived state, mode tables
│   ├── mpcraft-link/       # Session, transactions, params, missions, commands
│   ├── mpcraft-mission/    # MissionItem model, MAV_CMD metadata, QGC WPL import/export
│   ├── mpcraft-geo/        # geodesy, UTM/MGRS, terrain (SRTM/DTED)
│   ├── mpcraft-params/     # parameter metadata repository, types, ranges, bitmasks
│   ├── mpcraft-log/        # tlog, DataFlash (.log/.bin), ULog, KML export
│   ├── mpcraft-settings/   # persisted config, plugin registry, recent-connections
│   └── mpcraft-app/        # application orchestration, event bus, app-wide state
├── apps/
│   ├── desktop/            # Tauri v2 binary (thin: delegates to mpcraft-app)
│   └── ui/                 # React + TS frontend
└── docs/
```

Dependency direction is strictly downward. `mpcraft-app` is the only crate that knows about the
UI-facing application model; no core crate may depend on it.

---

## 5. Phased roadmap

Each phase ends with a **working, demonstrable application** — not a partial library.

### Phase 0 — Foundation *(2 weeks)*

**Goal:** a headless binary that connects to SITL and streams decoded telemetry over stdout.

- [ ] `mpcraft-mavlink`: framing parser for MAVLink v1/v2, CRC-16/MCRF4XX, CRC_EXTRA seed
      accumulation, signature generation/verification, resync on CRC failure
- [ ] `mpcraft-transport`: `Transport` trait + serial, UDP, TCP, file implementations
- [ ] `mpcraft-vehicle`: `Telemetry` snapshot, `Vehicle` registry keyed by `(sysid, compid)`
- [ ] `mpcraft-link`: `Session`, heartbeat negotiation, vehicle-type detection, packet-loss
      accounting, `broadcast` fan-out
- [ ] `mpcraft-settings`: config file persistence
- [ ] `mpcraft-cli`: `mpcraft` CLI with `connect`, `monitor`, `params list` subcommands
- [ ] Test corpus: capture real MAVLink byte streams; assert parse → re-serialise is byte-identical

**Exit criteria:** `mpcraft connect udp:127.0.0.1:14550 --monitor` prints live telemetry from a
running SITL with no panics and no packet loss accounting regressions.

---

### Phase 1 — Flight Data MVP *(3 weeks)*

**Goal:** the app connects, shows live telemetry, a map, and a HUD. This is the "does it fly?" phase.

- [ ] `mpcraft-app`: session lifecycle, event bus, app-wide state, reconnect logic
- [ ] Tauri v2 shell: window, menus, connection dialog, serial/UDP/TCP port enumeration
- [ ] Frontend foundation: routing, theme system, layout shell, component library
- [ ] Live telemetry view: attitude/position/velocity/battery/mode/ECKF readouts
- [ ] MapLibre GL JS map with vehicle marker, trail, and home position
- [ ] Artificial horizon HUD (SVG/Canvas, gyro-compensated)
- [ ] Live telemetry graphs (uPlot) with selectable channels
- [ ] Action buttons: arm/disarm, mode change, RTL, takeoff, land, loiter, mission start/pause
- [ ] Message console (STATUSTEXT stream) with severity colouring
- [ ] Pre-flight checklist

**Exit criteria:** A user can connect to a vehicle, watch it fly on a map with live graphs, and
issue commands — entirely through the new UI.

---

### Phase 2 — Mission Planning *(4 weeks)*

**Goal:** full mission authoring round-trip with the vehicle.

- [ ] `mpcraft-mission`: `MissionItem` model, `MAV_CMD` metadata table, frame/param semantics
- [ ] Mission download/upload transactions (`MISSION_COUNT` / `MISSION_ITEM` / `MISSION_REQUEST`)
      incl. `MISSION_ITEM_INT` when `MAV_PROTOCOL_CAPABILITY::MISSION_INT` is advertised
- [ ] Fence and rally point upload/download
- [ ] Mission editor UI: virtualised table bound to the item model
- [ ] Map interaction: add, drag, delete waypoints; select and edit on the map
- [ ] MapLibre GL Draw integration for polygon/fence/rally geometry
- [ ] QGC `.waypoints` / `.plan` import and export; KML import/export
- [ ] Per-waypoint altitude modes: absolute / relative / terrain
- [ ] Mission validation (geofence violations, unreachable commands, altitude sanity)
- [ ] Home position management
- [ ] "Fly to waypoint" in Guided mode; "goto here" from the map

**Exit criteria:** A complex mission authored in MPCraft uploads to ArduPilot, is read back
byte-identically, and flies as expected.

---

### Phase 3 — Parameters & Configuration *(3 weeks)*

**Goal:** complete parameter and hardware configuration surface.

- [ ] `mpcraft-params`: parameter metadata repository (XML parsing), type/range/bitmask handling
- [ ] Full parameter download with progress, and incremental live updates
- [ ] Parameter write with batching, `*_ENABLE` ordering, reboot-required detection
- [ ] `.param` file import/export and diff
- [ ] Parameter search, favourites, tree grouping by prefix
- [ ] Friendly parameter view: metadata-driven widgets (dropdown / bitmask / range slider)
- [ ] Hardware configuration screens: frame type, compass, accelerometer, ESC, GPS ordering,
      serial ports, RC input/output, failsafe, flight modes, OSD
- [ ] Calibration flows: accelerometer, compass, motor test

**Exit criteria:** Every parameter is editable with correct validation, and the hardware setup
screens cover the ArduPilot boardset.

---

### Phase 4 — Logs & Analysis *(3 weeks)*

**Goal:** download, browse, graph, and export flight logs.

- [ ] `mpcraft-log`: tlog parser (reuses `mpcraft-mavlink`), DataFlash text and binary parsers,
      ULog parser
- [ ] Log download over MAVLink (`LOGGING_DATA`) and FTP
- [ ] Log browser: message tree, sample grid (virtualised), graph canvas, map track
- [ ] Preset graph definitions (equivalent to upstream `graphs/mavgraphs.xml`)
- [ ] tlog playback with scrub timeline, driving live telemetry views
- [ ] KML export of flight tracks; CSV export of samples

**Exit criteria:** A downloaded DataFlash log can be browsed, graphed, cross-highlighted with the
map, and exported — matching upstream's `LogBrowse` capability.

---

### Phase 5 — Firmware & Advanced *(4 weeks)*

**Goal:** the remaining Mission Planner feature surface.

- [ ] Firmware manifest download and selection
- [ ] Firmware upload over MAVLink (`FILE_TRANSFER_PROTOCOL`) and DFU over USB
- [ ] Board detection
- [ ] MAVLink FTP client for onboard file access
- [ ] Joystick support (gamepad API) with axis/button mapping and RC override
- [ ] Camera / gimbal control and video stream integration
- [ ] Geo-referenced image placement
- [ ] Terrain follow route generation; terrain data protocol server
- [ ] OpenDroneID, ADS-B relay, RTK GPS injection (RTCM3 / SBF / u-blox parsers)

---

### Phase 6 — Extensibility *(3 weeks)*

**Goal:** the plugin and scripting story, done properly.

- [ ] WASM plugin sandbox: capability-scoped host API (telemetry read, commands, mission access)
- [ ] Plugin lifecycle: `Init` → `Loaded` → `Loop@hz` → `Exit`
- [ ] Plugin management UI with enable/disable persistence
- [ ] Embedded scripting host (Rhai or Lua) for user automation
- [ ] Headless swarm/formation control

---

### Phase 7 — Polish & Release *(2 weeks)*

- [ ] Internationalisation (i18n) with locale bundles
- [ ] Theming (light/dark/high-contrast)
- [ ] Auto-connect / auto-reconnect profiles
- [ ] Crash reporting and diagnostics export
- [ ] Cross-platform packaging: MSI, AppImage, Flatpak, `.dmg`, `.deb`
- [ ] Code signing and auto-update
- [ ] Performance profiling and packet-rate optimisation

---

## 6. Cross-cutting workstreams

These run continuously across all phases.

| Workstream | Activities |
| --- | --- |
| **Testing** | Unit tests for pure logic; protocol golden-file tests; integration tests against SITL in CI; property tests with `proptest` for the framer and parsers |
| **CI** | GitHub Actions: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, Tauri build matrix for Windows/Linux/macOS |
| **Documentation** | Crate-level `//!` docs; a `docs/` site; in-app help |
| **Telemetry performance** | Profiling of the decode path; target < 100 µs per packet; zero-allocation steady state |
| **Compatibility testing** | Maintain a corpus of real `.tlog` and `.param` files; assert MPCraft reads them identically to Mission Planner |
| **Safety** | Arm/disarm and mission-write actions require confirmation; a configurable "armed action lockout" |

---

## 7. Parallelisation & agent assignment

The work decomposes cleanly along crate boundaries, which maps directly onto parallel agents.

| Agent | Owns | Phase focus |
| --- | --- | --- |
| `protocol` | `mpcraft-mavlink`, `mpcraft-transport` | 0, 5 |
| `vehicle` | `mpcraft-vehicle`, `mpcraft-params` | 0, 3 |
| `mission` | `mpcraft-mission`, `mpcraft-geo` | 2, 5 |
| `logs` | `mpcraft-log` | 4 |
| `app` | `mpcraft-app`, `mpcraft-settings` | 1, 6 |
| `ui` | `apps/ui`, `apps/desktop` | 1, 2, 3, 4 |
| `qa` | tests, CI, parity matrix | continuous |

See [`.opencode/agents/`](.opencode/agents/) for the executable definitions.

---

## 8. Risks & mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| MAVLink dialect drift vs. ArduPilot's XML | Protocol bugs | Generate from upstream XML via `mavlink-bindgen`; pin the XML revision in CI |
| Map UX worse than GMap.NET | Users abandon the planner | Build a screenshot-parity harness for the mission editor early in phase 2 |
| Tauri WebView differences across platforms | Platform-specific bugs | Earliest possible multi-platform CI; avoid OS-specific CSS |
| Feature parity slips | Scope creep | Enforce the `docs/PARITY.md` matrix; every upstream feature must be marked Done/Dropped with rationale |
| Plugin ecosystem cannot be reproduced | Lost extensibility | Design a clean host API early; a smaller, better plugin set beats a large broken one |
| Firmware flashing is high-risk | Hardware damage | Isolate entirely behind a `mpcraft-flasher` crate with explicit safety gates; ship last |
| Team unfamiliarity with Rust | Slower velocity | Enforce `clippy` gates; keep crates small; rely heavily on AI agents for boilerplate |

---

## 9. Success criteria

MPCraft is a success when:

1. A user can perform a complete workflow — connect → configure → plan → upload → fly → monitor →
   download logs → analyse — without ever needing Mission Planner.
2. All features in `docs/PARITY.md` marked **Must** are marked **Done**.
3. Sustained 20 Hz telemetry on a mid-range laptop with < 5 % CPU.
4. Cross-platform binaries install and run on Windows, Linux, and macOS.
5. The core crates have no UI dependency and are usable as a library or CLI.

---

## 10. Immediate next step

**Phase 0, Task 1 — `mpcraft-mavlink` crate skeleton and framing parser.**

Deliverable: a crate that parses a byte stream into typed MAVLink frames, verifies CRC-16 with the
correct CRC_EXTRA seed, resynchronises after corruption, and passes golden-file tests built from
real captured traffic.

---

[`.opencode/agents/`]: .opencode/agents/
