# MPCraft — Architecture Decision Records

Status legend: **Accepted** · **Proposed** · **Superseded**

---

## ADR-001 — Rewrite from scratch rather than port Mission Planner

**Status:** Accepted

**Context.** Mission Planner is ~500k lines of C# across ~90 projects, with global mutable state,
a cooperative mutex serialising all link traffic, and a `#if LIB` conditional-compilation split
between the desktop and mobile builds.

**Decision.** Build a new codebase that preserves *functional capability*, not implementation.

**Rationale.** A line-by-line port reproduces the structural debt while inheriting none of the
modern ecosystem. The parts of Mission Planner that are genuinely valuable — the MAVLink
protocol knowledge, the `mavcmd.xml` schema, the parameter metadata, the log format support — are
*data and specifications*, and those transfer directly.

**Consequences.** We lose 20 years of accumulated UI edge cases and must re-derive them from the
parity matrix. We gain cross-platform support without conditional compilation, and a codebase
that can be tested.

---

## ADR-002 — Use the upstream `mavlink` Rust crate instead of generating bindings ourselves

**Status:** Accepted

**Context.** Mission Planner vendors `pymavlink` and generates 35 000 lines of C# message structs
from XML. We need the same message definitions, CRC seeds, and enums.

**Decision.** Depend on `mavlink` v0.20 (`mavlink-core` + `mavlink-bindgen`) with the
`dialect-ardupilotmega` and `dialect-common` features.

**Rationale.** The crate is generated from the *same canonical XML dialects* ArduPilot publishes,
so the definitions cannot drift. It already provides CRC-16/MCRF4XX, CRC_EXTRA seeds, v1/v2
framing, and MAVLink 2 signing — all areas where a hand-rolled implementation would be a source of
subtle protocol bugs.

**Consequences.** We take on an upstream dependency and must track its releases. We do *not* take
its byte-at-a-time reader; we implement our own buffered parser on top of its primitives.

---

## ADR-003 — Single reader task with broadcast fan-out

**Status:** Accepted

**Context.** Upstream has a background `SerialReader` loop *and* every request/response
transaction reading the same socket, arbitrated by the `giveComport` boolean plus a
`SemaphoreSlim`. Miss the flag and the link deadlocks.

**Decision.** One task per session owns the byte stream. It parses and validates frames, then
publishes to a `tokio::broadcast` channel. Transactions register matchers and await dedicated
`oneshot` responders. Telemetry aggregation runs in its own task.

**Rationale.** This removes the global bottleneck entirely and makes concurrent operations safe by
construction. It also makes the packet-loss accounting exact, because there is a single place
where sequence numbers are observed.

**Consequences.** The `broadcast` channel is lossy under backpressure. Transactions must not rely
on channel buffering for their response — they use the waiter list. The `Session` API is async
rather than blocking, which is a departure from upstream's synchronous style.

---

## ADR-004 — Push-update telemetry snapshot instead of computed properties

**Status:** Accepted

**Context.** Upstream `CurrentState` exposes ~400 public properties, each re-deriving its value
from `getPacketLast(msgid)` on access. The UI reads them repeatedly at 10 Hz.

**Decision.** Decode each packet once into a `Telemetry` struct. Publish it via `ArcSwap`. Derived
values (ground speed, distance to home) are computed at decode time.

**Rationale.** Eliminates redundant work, decouples the data model from the UI entirely, and makes
the state trivially snapshot-testable.

**Consequences.** Freshness semantics change: a value is as old as the last packet that carried it.
The model must therefore also expose a per-field timestamp or age for stale-value indication —
which upstream never does, and which is a genuine usability improvement.

---

## ADR-005 — Tauri v2 + React/TypeScript for the UI

**Status:** Accepted

**Context.** The UI must show a map with live tracking, dense data tables, high-rate time-series
graphs, and a real-time HUD. It must run on Windows, Linux, and macOS.

**Decision.** Tauri v2 shell with a React 19 + TypeScript + Vite frontend. MapLibre GL JS for
maps, uPlot for time series, TanStack Table + Virtual for dense grids.

**Rationale.** The map and the data-dense grids are the hard parts of a GCS UI, and the
JavaScript ecosystem solves both far better than any pure-Rust equivalent available today
(`egui_map` has ~18k downloads and is effectively unmaintained). Tauri keeps the shell small and
the backend entirely in Rust.

**Consequences.** The UI is not Rust. We need a strict, typed IPC contract and disciplined
serialisation discipline on the Rust side. We gain access to a mature ecosystem and a modern
developer experience.

---

## ADR-006 — Cross-platform from day one

**Status:** Accepted

**Context.** Mission Planner is Windows-only for building and running, with a separate Xamarin
build for Android using `#if LIB` conditional compilation — described in upstream's own `CLAUDE.md`
as "the single most important thing to understand before touching shared code".

**Decision.** Windows, Linux, and macOS are all in scope for v1. The core crates carry no
platform-specific code; platform differences live behind traits or in the shell.

**Rationale.** Rust makes this nearly free for the core. Deferring it would mean repeating
upstream's mistake of a platform-specific fork of the codebase.

**Consequences.** CI must build and test on three platforms from the first commit. macOS
notarisation and Linux packaging are real costs, scheduled in phase 7.

---

## ADR-007 — Capability-scoped WASM plugin model

**Status:** Proposed

**Context.** Upstream loads C# plugins by compiling `.cs` files at runtime with Roslyn, and hosts
an IronPython scripting engine. Both are powerful and both are unsafe — a plugin has full process
access.

**Decision.** Plugins are WebAssembly modules running in a sandbox with an explicit capability
grant: `telemetry:read`, `commands:write`, `mission:read`, `mission:write`, `params:read`,
`params:write`, `ui:panel`, `network:fetch`.

**Rationale.** Plugins are the main extension vector for a GCS, and users install third-party
ones. Sandboxing them is a security requirement, not a nicety. WASM is the only mature sandbox
that works across all three target platforms without a process boundary.

**Consequences.** Plugins cannot be written in Rust and loaded natively without a separate ABI —
WASM is the only interface. The host API must be designed carefully, since it is a public contract.
This is a phase-6 decision and may need revision once the API is exercised.

---

## ADR-008 — Phased delivery with working milestones

**Status:** Accepted

**Context.** Mission Planner took years to reach feature completeness. A big-bang rewrite would
take years with nothing usable in the interim, and would likely be abandoned.

**Decision.** Eight phases, each ending in a demonstrable application. The parity matrix
(`docs/PARITY.md`) tracks every upstream feature and requires each to be marked Done or Dropped
with a rationale.

**Rationale.** Delivers value from month one, keeps the scope honest, and makes abandonment
cheaper at every point.

**Consequences.** Some upstream features will be deliberately dropped rather than ported. That is
the point of the matrix.

---

## ADR-009 — Keep the core free of UI dependencies

**Status:** Accepted

**Context.** Upstream's `MissionPlannerLib.csproj` compiles the entire application — including
`MainV2.cs`, `GCSViews/`, and `Controls/` — as a library, guarded by `#if LIB`. This is why
upstream's own documentation flags conditional compilation as its number-one trap.

**Decision.** Only `mpcraft-app` and `apps/desktop` may depend on Tauri. All other crates are
headless and testable without a display.

**Rationale.** Enables the `mpcraft` CLI, containerised CI testing, and future embedding in other
tools. Structurally prevents upstream's problem from recurring.

**Consequences.** Application-level state that upstream keeps in `MainV2` statics must be
explicitly modelled in `mpcraft-app` and passed down.

---

## ADR-010 — Reuse upstream data schemas verbatim where they exist

**Status:** Accepted

**Context.** Several upstream artefacts are domain specifications, not implementation:
`mavcmd.xml` (MAV_CMD parameter semantics), `ParameterMetaData.xml` (parameter descriptions,
ranges, bitmasks), the DataFlash format catalogue, and the graph presets.

**Decision.** Port these as data — converting `mavcmd.xml` to `mavcmd.toml` for readability but
preserving the schema — rather than redesigning them.

**Rationale.** These encode hard-won knowledge about ArduPilot's behaviour. Redesigning them
risks subtle incompatibilities and provides no benefit.

**Consequences.** MPCraft consumes the same ecosystem artefacts and stays compatible with the
wider ArduPilot tooling.

---

## ADR-011 — Let the generated bindings carry the MAVLink 2 extension fields

**Status:** Accepted

**Context.** `MAV_MISSION_TYPE` is a MAVLink 2 *extension* field: it is absent from a MAVLink 1
frame and truncated away when it is zero. The upstream `mavlink` Rust crate generates the
bindings with a `mav2-message-extensions` feature, and MPCraft was built without it. The
consequence was that a fence or a rally plan — whose `mission_type` is not zero — could not be
expressed at all, and the parity matrix carried three rows as permanently partial.

The obvious fix, hand-rolling the extension bytes in the encoder, was the wrong one: it would
mean re-implementing the truncation rule, the checksum coverage and the length calculation
that the protocol already specifies, and it would do so in the one place where a mistake is
silent.

**Decision.** Enable the `mav2-message-extensions` feature and use the generated fields.

**Rationale.** The bindings were never the limitation; the feature flag was. With it on, the
generated `ser` truncates the extension bytes at the first zero for MAVLink 2 and omits them
for MAVLink 1, and the frame length follows the truncated length — the framing layer needs no
change at all. The plan type now reaches the wire, the fence and rally plans become
expressible, and the rule lives in one place: the protocol specification.

**Consequences.** The three partial rows of the parity matrix become achievable. The
`MissionType` reaches the vehicle on `MISSION_REQUEST_LIST`, `MISSION_COUNT` and every item,
so a download knows which plan it is reading and an upload declares which one it is writing.

