---
name: MPCraft Conventions
description: Core architectural rules, crate boundaries, and coding conventions for the MPCraft Rust codebase. Load before writing any Rust or TypeScript for MPCraft.
metadata:
  opencode/autoinvoke: true
---

# MPCraft conventions

MPCraft is a Rust rewrite of Mission Planner (an ArduPilot GCS). Read `docs/ARCHITECTURE.md` and
`docs/PARITY.md` before substantial work.

## Crate boundaries

```
mpcraft-mavlink  →  mpcraft-transport  →  mpcraft-link  →  mpcraft-app  →  apps/desktop
                                  ↘  mpcraft-vehicle / -mission / -params / -geo / -log
```

- Dependencies point **downward only**.
- **Only `mpcraft-app` and `apps/desktop` may use `tauri`.** A core crate importing Tauri is a bug.
- Core crates must build and test **headless** — no display server, no Tauri.
- `apps/desktop` is a thin shell: command wiring only, zero business logic.

## Rust conventions

- `#![forbid(unsafe_code)]` in every crate.
- `thiserror` for library errors, `anyhow` only in binaries.
- **No `unwrap()`, `expect()`, or `panic!` outside tests.**
- `cargo clippy --workspace --all-targets -- -D warnings` is a hard gate.
- Doc comments explain **why**, not **what**.
- Public enums that will grow are `#[non_exhaustive]`.
- Hot paths (telemetry, 20 Hz) use `ArcSwap` / `parking_lot`, not `std::sync::Mutex`.
- Never hold a lock across an `.await`.
- Errors are typed and propagated. Silent failures are a defect.

## The concurrency model

Upstream Mission Planner serialises all link traffic with a cooperative `giveComport` boolean.
MPCraft does not.

- **One reader task per session** owns the byte stream and publishes frames on a
  `tokio::broadcast` channel.
- **Transactions** register a matcher and await a dedicated `oneshot` responder. Never rely on
  broadcast buffering for a response — it is lossy.
- **Telemetry aggregation** runs in its own task, publishing an `ArcSwap<Telemetry>` snapshot.
- Concurrent transactions are safe. A slow transaction must never starve telemetry.

If you need a global mutex around the link, you are reintroducing the upstream flaw.

## IPC conventions (Rust ↔ TypeScript)

- Every command returns `Result<T, CommandError>`.
- `CommandError` carries a stable snake_case `code`. The frontend branches on the code, never on
  message text.
- Telemetry is pushed as **one batched event per tick** (default 20 Hz), never per field.
- **The frontend knows nothing about MAVLink** — no message ids, no `MAV_CMD` integers, no
  parameter-name conditionals for behaviour. Expose intent-level operations instead.
- TypeScript types mirror Rust types exactly. No `any`.

## Protocol conventions

- **Never hand-write a MAVLink message struct.** Use the generated `mavlink` crate bindings; they
  carry the correct CRC_EXTRA seed and field order.
- CRC correctness is non-negotiable: verify against **real captured frames**, not just round-trip
  tests. A symmetrically wrong CRC passes round-trip and corrupts every mission upload.
- Honour capability-gated paths: `MAV_PROTOCOL_CAPABILITY::MISSION_INT` and `PARAM_FLOAT`.
- Any buffer sized from a wire-supplied length must be capped.
- The parser must survive truncation, garbage, and corruption without panicking.

## Mission conventions

- `MAV_CMD` metadata lives in `crates/mpcraft-mission/src/mavcmd.toml` and drives the UI. No
  per-command logic in TypeScript.
- **Round-trip fidelity is the contract**: download → upload must be byte-identical.
- `MISSION_ITEM_INT` scales lat/lon by `1e7` but does **not** scale non-location commands.
- Latitude/longitude are `f64`; rounding to wire format happens at exactly one boundary.

## Testing conventions

| Layer | Technique |
| --- | --- |
| Pure logic | Unit tests beside the code; `proptest` for parsers |
| Wire format | Golden files — real captured bytes → expected decode |
| Round-trip | serialise → parse → serialise byte equality |
| Integration | SITL — the CI container or the local binaries, behind a `sitl-tests` feature flag |
| Frontend | Vitest for stores; Playwright for critical flows |

- A test must **assert** something. Calling code is not a test.
- Never assert on tight wall-clock timing.
- SITL-dependent tests never run in the default `cargo test --workspace`.

## Project hygiene

- **`docs/PARITY.md` is updated in the same change** as the feature it tracks. No row is left
  blank.
- Run `cargo fmt --all` before committing.
- No debug logging, no commented-out code, no unrelated reformatting in a change.
- The reference clone at `_ref_missionplanner/` is **read-only** — never modify it.
