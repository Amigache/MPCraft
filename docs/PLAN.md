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
- [ ] Parameter access as a capability (`read_params`, `write_params`), which is the one part of
      upstream's script surface the granted set does not cover — see row 9.4
- [ ] Headless swarm/formation control

There is no embedded scripting engine, and that is the decision rather than an unfinished item: the host as
upstream builds it loads every assembly in the process and puts the application objects into the scope, which
is the ambient authority ADR-007 excludes, so what a user automation can be here is a module with a manifest a
person reads before deciding to run it. The names the sandbox can grant today are `read_telemetry`,
`send_commands`, `mission_access` and `notify`; parameter access is the gap that has to be closed before a
plugin can do what upstream's scripts mostly did, which is change a parameter while a flight runs. A
capability that is not one of these is refused rather than guessed at, so adding one is a change to the
granted set that the manifest gate has to read.

The sandbox needs a WASM runtime, which is the one dependency this phase is allowed to bring in:
the rest of the workspace stays free of new crates. The runtime lives inside `mpcraft-plugin` and
no other crate knows about it, so the layering rule is not broken by it. A plugin receives its
capabilities as the host functions it is given, and a capability it was not given cannot be
reached, which is what makes a plugin safe to load at all.

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

## 10. Current state and immediate next step

The workspace, the plan, the agents and the skills are in place. The following is already
built and verified:

| Component | State |
| --- | --- |
| `mpcraft-mavlink` | Frame parser and encoder (v1/v2), CRC with CRC_EXTRA seeds, signature handling, packet inspector — **34 tests** |
| `mpcraft-transport` | `Transport` trait, `ConnectionSpec` parser, serial / UDP / TCP / file transports — **40 tests** |
| `mpcraft-geo` | Geodesy, pure-Rust UTM and MGRS — **15 tests**, validated against PROJ and two independent MGRS implementations |
| `mpcraft-vehicle` | Telemetry snapshot with derived values and staleness tracking — **10 tests** |
| `mpcraft-link` | Session, heartbeat negotiation, vehicle registry, loss accounting, telemetry aggregation, the parameter and mission protocols — **67 tests** |
| `mpcraft-params` | The parameter model, the encoding rules and the capability gate — **14 tests** |
| `mpcraft-mission` | The plan model, the MAV_CMD metadata and the wire conversions — **21 tests** |
| `mpcraft-mission` | Mission model and `MAV_CMD` metadata table — **10 tests** |
| `mpcraft-app` | The IPC contract and its stable error codes — **5 tests** |
| `mpcraft-cli` | `mpcraft-cli ports \| connect \| params \| mission` |
| `apps/desktop`, `apps/ui` | Tauri v2 shell and the typed IPC bridge |
| CI | Format, clippy, test and build gates on Windows, Linux and macOS |

**205 tests pass. `clippy -D warnings` and `rustfmt --check` are clean.**

### Phase 0, Task 2 — the transport layer: what was built

The `Transport` trait, and four transports on top of it:

- **Serial** (`tokio-serial`), with port enumeration and explicit DTR control. DTR defaults to
  *low* on open, which is what resets the ESP32-class boards that use it as a reset line.
- **UDP**, in client and server mode, with the **fan-out** behaviour upstream has: a frame is
  written to every peer that has been heard from, which is what lets two ground stations
  watch the same vehicle.
- **TCP**, in client and server mode, with the accept deferred to the first read so a
  listening station never races its own startup.
- **File**, replaying a recorded byte stream at a configurable rate. This is what makes the
  whole stack testable without hardware.

Plus `ConnectionSpec`, a parser for `scheme:target|option` connection strings with typed
errors. The `|` separator keeps Windows paths such as `C:\logs\flight.tlog` unambiguous.

Two deliberate decisions:

- **The RTS option was removed.** The serial driver exposes no RTS line control, and an
  option that silently does nothing is worse than no option at all.
- **Replaying a `.tlog` with its original timing lives in `mpcraft-log`, not here.** The
  transport is byte-oriented and knows nothing about the log format; `mpcraft-log` implements
  the same trait and does understand the records.

### Phase 0, Task 3 - the session: what was built

`mpcraft-link` now turns a byte stream into vehicle state:

- **`Session`** - the reader task that owns the transport, and the actor model around it.
- **Heartbeat negotiation** - the vehicle is identified from its heartbeat, with upstream's
  preference rules. **One deliberate divergence:** upstream orders the candidate groups by how
  often each was seen, which hands a vehicle's *gimbal* to the operator instead of the vehicle.
  MPCraft implements upstream's *stated* intent - component id 1 wins - and says so in the code.
- **The vehicle registry** - keyed by `(system_id, component_id)`, with the ArduPilot vehicle
  family detected from the heartbeat.
- **Packet-loss accounting** - upstream's rolling 64-bit mask algorithm, ported exactly, so a
  reordered packet is not counted as a lost one.
- **The telemetry aggregator** - messages decoded once into a snapshot, with the derived values
  computed at decode time.
- **The board-output capture** - the bytes outside of MAVLink frames are kept, which is what
  turns "no heartbeat received" into "the board is printing at the wrong baud rate". A
  DroneCAN banner is recognised as such.

### The golden-file test

`scripts/generate-test-stream.py` is a **second, independent implementation** of the MAVLink wire
format. The Rust parser is therefore checked against something other than itself, and the two are
compared byte for byte across every frame of the fixture.

It has already earned its keep. It caught two real bugs:

1. **The message id was read from the wrong offset.** The MAVLink 2 header carries the message id
   at offset seven, not ten. The bug was invisible to the round-trip tests, because a
   self-consistent implementation is self-consistently wrong - and it would have broken **every**
   MAVLink 2 frame, which is what every modern vehicle sends.
2. **MAVLink 2 truncates the trailing zero bytes of a payload.** The generated code does this
   and the receiver pads the payload back. The fixture now exercises the truncated form, so the
   parser is tested against the shorter payload it will really receive.

### Phase 0, Task 4 - the parameter protocol: what was built

`mpcraft-params` and the parameter operations of `mpcraft-link`:

- **`ParameterStore`** - the values, the type the vehicle reported for each, and the total the
  vehicle says it has.
- **The encoding rules** - a parameter value travels in a four-byte field declared as a
  `float`, and the `param_type` says what those four bytes mean. There are two ways they can
  be filled: byte-wise, or cast to a float. The two are **not** interchangeable, and guessing
  wrong corrupts every integer parameter.
- **The capability gate** - the vehicle says which it uses, through
  `MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_BYTEWISE` or `PARAM_ENCODE_C_CAST`. Two real
  autopilots need no bit at all: ArduPilot sends floats, and PX4 sends bytes. Mission
  Planner's rule covers both, and MPCraft keeps it while also honouring the two current
  capability bits.
- **`download_parameters`**, **`read_parameter`** and **`write_parameter`** - the three
  operations of the protocol, with the timeout and retry semantics upstream has.
- **The write is confirmed by the value the vehicle stores**, not by the value that was sent.
  A vehicle that refuses a write answers with the unchanged value, which is how the ground
  station learns it failed.

### A race that the socket tests caught

The parameter list is a *stream*: the request goes out, and the vehicle answers with a
`PARAM_VALUE` per parameter. The first implementation sent the request and *then* registered
the collector - so a vehicle on a fast link could answer before anybody was listening, and the
answer was lost.

Upstream avoids this by subscribing to the message type before sending the request. MPCraft
now does the same, in two steps: `begin_collect` registers the collector, the request is sent,
and only then does the collector wait. The suite that runs a simulated vehicle on a real UDP
socket is what made the race visible.

### Phase 0, Task 5 - the mission protocol: what was built

`mpcraft-mission` and the mission operations of `mpcraft-link`:

- **The domain model** - a `MissionItem` with a stable identity, a `Plan`, and the wire
  conversions in `mpcraft-mission::wire`.
- **`download_mission`** - the vehicle reports how many items it has, and the items are then
  requested one by one.
- **`upload_mission`** - the ground station announces the number of items, and then the
  **vehicle drives the exchange** by asking for each item in turn. The write is finished by a
  `MISSION_ACK`, which says whether the plan was accepted.
- **The `MISSION_ITEM_INT` capability** - a vehicle that advertises it wants the integer form,
  whose coordinates are scaled by `1e7`. Mission Planner does the same.
- **Only the coordinates of a location command are scaled.** A command whose X and Y are not a
  position keeps them as they are. The metadata table says which is which, and the scaling
  follows the table.

### The exit criterion is met

A plan downloaded from a vehicle and uploaded back is **byte-identical on the wire**, and the
suite that proves it runs a simulated vehicle on a real UDP socket.

### A second race, in the other direction

The upload had the same flaw as the download: the plan was announced *before* the matcher was
registered, and the vehicle answers the announcement immediately. The matcher is now registered
first, in two steps: `begin_request`, announce, then wait.

This is why the session has both `begin_request` and `begin_collect`. A transaction whose
answer can arrive before the request has finished being sent must register its matcher first.

### The extension field of the mission type

`MAV_MISSION_TYPE` is a MAVLink 2 **extension field**: it is absent from a MAVLink 1 frame,
and it is truncated away when it is zero. A plain mission therefore travels in the short
form, and a fence or rally plan travels with the extension present.

For a long time the bindings could not express it, and the fence and rally protocols were
marked as partial in the parity matrix. The limitation was not the bindings but a feature
flag: the upstream `mavlink` crate generates the extension fields when the
`mav2-message-extensions` feature is on, and MPCraft had it off. With it on, the generated
`ser` truncates the extension bytes at the first zero for MAVLink 2 and omits them for
MAVLink 1, and the frame length follows the truncated length — the framing layer needed no
change at all. See ADR-011.

### Phase 0, Task 6 - the desktop shell and the session supervisor

The application layer exists and works end to end:

- **`mpcraft-app`** - `SessionSupervisor`, which owns the link, publishes the state to the
  interface every 200 ms, and decides whether to keep the link open.
- **`apps/desktop`** - a thin Tauri v2 shell. It wires commands to the supervisor and owns
  nothing else.
- **`apps/ui`** - the React frontend: the connection panel, the flight data view with live
  telemetry, the attitude indicators and the message console.
- **`scripts/simulated-vehicle.py`** - a vehicle simulator, written from the MAVLink
  specification independently of the Rust implementation. It is what makes the whole stack
  demonstrable without hardware, and it is the counterpart of the golden file.

The shell connects on launch when a target is given:

```bash
mpcraft.exe --target udp:127.0.0.1:14550
```

### Three findings from putting the application on screen

1. **A ground station must announce itself.** A vehicle that only streams telemetry never
   learns the ground station's address, so its traffic goes nowhere. The session now sends a
   heartbeat when the link opens.
2. **A UDP reset is not a broken link.** On Windows a UDP socket reports the previous
   datagram's "port unreachable" as a connection reset on the next read. Treating that as a
   fatal failure tore down a perfectly good link. It is now treated as transient.
3. **The interface needs an explicit capability to talk to the development server.** Tauri v2
   restricts the IPC to the application's own origin, so a page loaded from the development
   server could not call a single command or receive a single event. The capability that grants
   it lives in `apps/desktop/capabilities/default.json`.

### Phase 0, Task 7 - closing the phase

The phase is closed. The four things it was waiting for are done:

- **The mission editor commands**: setting the current item, emptying a plan and jumping to
  an item are now operations of the session.
- **Fence and rally points**: the decision was to let the generated bindings carry the
  extension field, not to hand-roll the bytes. The `mavlink` crate has had a
  `mav2-message-extensions` feature all along; it simply was not switched on. With it on,
  `MAV_MISSION_TYPE` reaches the wire on `MISSION_REQUEST_LIST`, `MISSION_COUNT` and every
  item, so a fence plan downloads as a fence plan and an upload declares which plan it is
  writing. See ADR-011.
- **The `.waypoints` and KML import and export**: the QGC format follows upstream's reader
  and writer field for field, including the home position in the first line. The KML is
  written by hand, without an XML dependency.
- **The session supervisor**: connection profiles are stored in a hand-editable TOML file
  under the platform configuration directory, and the supervisor reconnects with a backoff
  that is injectable, so the policy is tested without a network peer and without waiting.

### The exit criterion, met

The criterion was that the `mpcraft-cli` can connect to a live vehicle, read its
parameters, download its plan, upload a modified plan and fly it — entirely from the
command line. It can:

```text
mpcraft-cli --target udp:127.0.0.1:14550 params list
mpcraft-cli --target udp:127.0.0.1:14550 params set FENCE_MARGIN 25
mpcraft-cli --target udp:127.0.0.1:14550 mission download --output plan.waypoints
mpcraft-cli --target udp:127.0.0.1:14550 mission upload plan.waypoints
mpcraft-cli --target udp:127.0.0.1:14550 mission jump 2
mpcraft-cli --target udp:127.0.0.1:14550 arm
mpcraft-cli --target udp:127.0.0.1:14550 mode GUIDED
mpcraft-cli --target udp:127.0.0.1:14550 takeoff 30
mpcraft-cli --target udp:127.0.0.1:14550 disarm
```

The vehicle it talks to is `scripts/simulated-vehicle.py`, which answers the parameter,
mission and command protocols. It was written from the MAVLink specification, independently
of the Rust implementation, so the two agreeing is evidence that both are right.

### A race the parameter protocol hid

The encoding of a parameter value follows the autopilot the vehicle reports, and the
vehicle only says what it is in its heartbeat. A ground station that reads a parameter
before the heartbeat has arrived guesses the encoding, and the guess reads the wrong
numbers — an integer parameter comes back as the bit pattern of a float.

The session now waits for the vehicle to *identify itself* before a parameter operation
begins. Waiting for the vehicle to merely *appear* is not enough: it appears on the first
frame it sends, whatever that frame is, and a vehicle that sends telemetry before its
heartbeat is still unidentified at that point.

### Phase 1 - the flight data window

Phase 1 is the flight data view proper, and it starts from a complaint: the interface did
not resemble Mission Planner's. The **distribution and the elements** are Mission Planner's,
because that is what an operator's hands already know; the **style** is modern, because
that is what the rest of the world expects.

The distribution was read out of upstream's WinForms designer rather than remembered:

- the window has a **menu strip** at the top and a **status strip** at the bottom;
- the **left column is 300 pixels and fixed**: the artificial horizon above, a **tabbed
  panel** below;
- the tabs are upstream's: **Quick**, **Actions**, **Messages**, **Gauges**, **Status**;
- the **map** takes everything that is left.

What is on the screen:

- The **HUD**: a horizon that tilts with the roll and moves with the pitch, a pitch
  ladder, a bank scale and a heading ribbon.
- The **map**: the vehicle, the track history, the plan and the home position, with a
  follow toggle and a zoom control.
- The **Actions tab**: the mode selector, arm and disarm, the quick flight modes, the
  waypoint actions and the plan transfer.
- The **Gauges tab**: the ground speed, the altitude, the vertical speed and the heading.
- The **Quick tab**: the tiles an operator glances at, and the choice of which ones.
- The **Messages tab**: the vehicle's own `STATUSTEXT`, which the session now captures.

### What the backend had to grow for it

The interface asked for things the backend did not have:

- **The vehicle's messages.** `STATUSTEXT` was never captured; the console showed the
  notices of the link, not what the vehicle said. The session now keeps the last two
  hundred of them, with their severity.
- **The flight commands over the IPC boundary.** Arm, disarm, takeoff, the mode change,
  the plan transfer, the current item, the jump and the clearing of a plan. They live in
  `mpcraft-app`, not in the shell, so the rule that a command needs an open link is
  written once and tested without a display server.
- **The mode table.** It lives in `mpcraft-vehicle`, because the numbers are the contract
  with the autopilot and not ours to choose.
- **The state of the positioning.** The aggregator did not read `GPS_RAW_INT`, so the fix,
  the satellites and the dilution of precision were unknown.

### The graphs

The graphs are the reason a ground station exists, and they are the last piece of the
flight data window: uPlot over the telemetry history, with the presets of upstream's
`mavgraphs.xml`.

Upstream's catalogue is a hundred and eight graphs, and each one is a set of expressions
over the telemetry. The expressions are evaluated in Rust, in `mpcraft-vehicle::graph`,
because they name MAVLink fields and the interface must not know the protocol: it
receives numbers with names.

The catalogue is ported the same way `mavcmd.xml` was: the presets that draw on messages
MPCraft decodes are kept, with the expressions rewritten over its own fields, and the ones
that draw on messages it does not decode yet are left out rather than shown empty. The
presets MPCraft adds for the values upstream does not graph are marked as its own.

**Thirteen of upstream's hundred and eight are ported so far.** The limit was never the
interface but the model: a preset draws on a message, and a message nobody decodes is a
preset with nothing to draw. The vehicle model now knows the inertial measurement, the
barometer, the servos, the radio control and the air data, and the link decodes
`RAW_IMU`, `SCALED_IMU`, `SCALED_PRESSURE`, `SERVO_OUTPUT_RAW`, `RC_CHANNELS` and
`VFR_HUD`.

The units are the ones an operator reads, and the conversion happens once, where the
packet is decoded: the accelerometers are in thousandths of a g, the gyroscopes in
thousandths of a radian per second, the magnetometers in thousandths of a gauss, the
temperatures in hundredths of a degree and the pulse widths in microseconds. The graph
engine reads the rate of the gyroscope in degrees, the way the attitude is read.

A value the vehicle did not report stays unknown rather than becoming zero: a servo at
zero microseconds is a fault, not the absence of a report, and the radio channels beyond
the count the vehicle says it has are not reported at all.

The expression language is the one upstream uses: the arithmetic operators, the power, and
a handful of functions. It is read once, when the catalogue is first needed, because a
graph that is redrawn twenty times a second must not parse its expressions twenty times a
second.

The history is bounded twice over — by the number of samples and by how long they are kept
— and it lives in the application, not in the session: the session owns the link, and the
history is a view of what the link has seen.

### The parameters and the plan

The **parameter editor** is a sheet over the window, opened from the Setup menu: the
parameters the vehicle has reported, a search box, and a value that can be edited in place.
The write waits for the vehicle to confirm it, and the interface shows the value the
vehicle **stored**, which is not always the one that was asked for: a vehicle clamps to its
range, and the operator has to see what the vehicle has rather than what they wanted.

The rules about a plan are the autopilot's, so they live in the model and not in the
interface. `mpcraft-mission::validate` holds the one rule upstream has: a waypoint with a
zero altitude would fly the vehicle to the ground. The interface shows what the rule says
and does not know why. A fence has a rule of its own: a polygon needs three vertices, and
the vehicle cannot build a fence out of two.

The editor has the three kinds of plan upstream's planner has - mission, fence and rally -
and each kind is edited with the commands that kind takes. The fence is a polygon: the
vertices that are included in it make the ring, the points that are excluded from it are
drawn in the colour of the danger, and the circles that are in it or out of it are drawn
as they are.

The commands of a plan are named the way the specification names them, and the catalogue
carries the name the operator reads them with: the plan of the vehicle says
`MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION` and the interface shows *Fence Polygon
Vertex*.

The **profile of the terrain** along the plan is the shape of the ground the plan flies
over: the elevation of the terrain at a point every few metres, and the places of the
plan on it. The tiles of the terrain come from the service of ArduPilot, compressed, and
they are kept on the disk, so that the second look at the same place is a look at the disk
and not at the service.

The **profile can be wrong** in one way only: a place the survey never reached, or the
sea, has no elevation. Those points are left out of the profile and the plan is still
drawn: a ground station that invents the height of the sea is a ground station that flies
the aircraft into it.

The plan can be read from and written to a file — the QGC `.waypoints` format and the KML
that Google Earth reads — and the file is read and written in Rust, where the formats live.

The **parameter file** is the same story: a line per parameter, the name and the value
separated by a comma, the comments starting with a hash. The format is upstream's, and the
parameters the vehicle writes for its own bookkeeping — the number of the mission, the
ground pressure of the last boot — are not part of the configuration and are left out.

The **configuration groups** are the subjects the parameters are about — *Radio
Calibration*, *EKF2*, *Multicopter Attitude Control* — and upstream builds its
configuration menu out of them. The catalogue knows seven hundred and fifty-nine of them.

A parameter the vehicle has in its memory but not in its **flash** is lost when the
vehicle loses power, and the vehicle only writes the flash when it is told to. That is
one command, and it is the one upstream asks for when the operator presses *Write*.

---

---

[`docs/ARCHITECTURE.md`]: docs/ARCHITECTURE.md
[`docs/PARITY.md`]: docs/PARITY.md
[`docs/DECISIONS.md`]: docs/DECISIONS.md
[`.opencode/agents/`]: .opencode/agents/
