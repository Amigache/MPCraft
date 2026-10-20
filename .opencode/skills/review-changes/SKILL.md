---
name: Review MPCraft Changes
description: Use when reviewing a pull request or a set of changes in MPCraft — checks architecture boundaries, Rust idioms, IPC contract changes, safety, and test adequacy.
metadata:
  opencode/autoinvoke: true
---

# Reviewing changes in MPCraft

## Order of concern

Review in this order. A violation early in the list invalidates everything after it.

### 1. Architecture boundaries (blocking)

- [ ] No core crate (`crates/mpcraft-*` except `-app`) references `tauri`.
- [ ] No dependency points upward in the crate graph.
- [ ] `apps/desktop` contains no business logic — only command wiring.
- [ ] The frontend contains no protocol knowledge: no message ids, no `MAV_CMD` integers, no
      parameter-name conditionals for behaviour.
- [ ] Core crates still compile and test headless.

### 2. Safety and correctness (blocking)

- [ ] No `unsafe` anywhere.
- [ ] No `unwrap()`, `expect()`, or `panic!` outside tests.
- [ ] Every wire-supplied length is bounds-checked before allocation.
- [ ] Every long-running operation has a timeout and a cancellation path.
- [ ] Errors are typed and propagated with `?` — not swallowed with `let _ =`.
- [ ] Locks are not held across `await` points.
- [ ] `Arc` cycles do not exist.

### 3. Rust quality (should-fix)

- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] Public items are documented — the doc explains *why*.
- [ ] Public enums that will grow are `#[non_exhaustive]`.
- [ ] `thiserror` in libraries, `anyhow` only in binaries.
- [ ] Hot paths use `ArcSwap`/`parking_lot` rather than `std::sync::Mutex`.
- [ ] No unnecessary clones in the telemetry path — it runs 20 times a second.

### 4. IPC contract (blocking if changed)

- [ ] New commands return `Result<T, CommandError>` with a stable snake_case `code`.
- [ ] The frontend branches on `code`, never on message text.
- [ ] Telemetry events are batched — no per-field events.
- [ ] Breaking changes bump the API version and update every call site.
- [ ] TypeScript types mirror the Rust types exactly, with no `any`.

### 5. Tests (should-fix)

- [ ] New behaviour has a test that *asserts* something, not just calls code.
- [ ] Protocol code has golden-file tests against real captured frames.
- [ ] Round-trip tests exist for anything serialised.
- [ ] Error paths are tested, not just the happy path.
- [ ] No test asserts on tight wall-clock timing.
- [ ] SITL-dependent tests are behind a feature flag.

### 6. UX and safety (should-fix)

- [ ] Destructive actions (arm, mission write, param write, flash) confirm with a specific
      consequence message.
- [ ] The destructive action is not the default focus.
- [ ] Loading, error, and empty states exist.
- [ ] Keyboard navigation and visible focus work.
- [ ] Stale telemetry is visually distinguished.

### 7. Project hygiene

- [ ] `docs/PARITY.md` updated in the same change.
- [ ] No debug logging left behind; no commented-out code.
- [ ] No unrelated reformatting mixed into the change.

## Reporting format

Group findings by severity. For each:

```
[blocking] crates/mpcraft-link/src/session.rs:142
  The transaction timeout path does not cancel the waiter registration, so
  a timed-out request leaks an entry in the waiter list.

  Fix: drop the registration guard on the timeout branch —
  `let _guard = self.register(...).await?;` so it deregisters on drop.
```

Rank by severity: **blocking** → **should-fix** → **nit**.

State clearly if something looks wrong but you could not confirm it — do not assert a bug you have
not verified.

## What not to do

- Do not rewrite the change. Report it.
- Do not flag style preferences that `rustfmt` and `clippy` already enforce.
- Do not demand a test for trivial code, but do demand one for protocol behaviour.
- Do not approve based on "it looks right". Run the gates.
