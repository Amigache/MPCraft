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
- [ ] A skill that disagrees with the script it describes is reported as a divergence, with the change that
      resolves it (`docs/SKILLS.md`, point 5) — a skill held to the same standard as the code.

### 8. Verifying what is reported (before writing it down)

A finding is a claim about behaviour, and a claim that could have been shown by running something is not a
finding until it has been run.

- [ ] Name what would show it: a test that fails, a byte on the wire, a value the vehicle answered, a box
      measured at 1024×640 and 1920×1080 in the language chosen, an entry the catalogue does not have.
- [ ] Run that thing. If it cannot be run on this machine, say so in the finding rather than presenting the
      reading as the proof.
- [ ] Golden values are worked out from the published dictionary, not copied from the code under test — a
      mirror passes round-trip and corrupts every upload.
- [ ] A gate's green is read with its control: a stand-in is named as a stand-in, and `NO-APLICA` is not
      evidence for the layer it did not read.
- [ ] What the code decides is kept apart from what the vehicle decides: a refusal, a clamped parameter, a
      mode the board reports back, are the board's answers, not failures of this code.
- [ ] Every finding cites a path and a line, and the line cited is read, not inferred from a name.

## Reporting format

Group findings by severity, and write for each one what showed it. The `verified by:` field is what makes a
review a report rather than an opinion. For each:

```
[blocking] crates/mpcraft-link/src/session.rs:142
  The transaction timeout path does not cancel the waiter registration, so
  a timed-out request leaks an entry in the waiter list.

  verified by: two transactions sent over scripts/simulated-vehicle.py, the second matching a packet the
             first had already consumed — the waiter list still holds the registration after the timeout.

  Fix: drop the registration guard on the timeout branch —
  `let _guard = self.register(...).await?;` so it deregisters on drop.
```

Rank by severity: **blocking** → **should-fix** → **nit**.

What could not be shown is written as unconfirmed, with the thing that would settle it, and is not ranked as
blocking:

```
[unconfirmed] crates/mpcraft-log/src/tlog.rs:88
  Looks like a buffer sized from a wire length without a ceiling. Nothing here could send a length that
  large, so this is a suspicion rather than a finding.

  What would settle it: a golden file whose record header names a length past the ring buffer, read against
  the parser.
```

State clearly if something looks wrong but you could not confirm it — do not assert a bug you have
not verified.

## What not to do

- Do not rewrite the change. Report it.
- Do not flag style preferences that `rustfmt` and `clippy` already enforce.
- Do not demand a test for trivial code, but do demand one for protocol behaviour.
- Do not approve based on "it looks right". Run the gates.
