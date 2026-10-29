# MPCraft

A modern, Rust-based Ground Control Station for ArduPilot & PX4, functionally equivalent to
[Mission Planner](https://github.com/ardupilot/MissionPlanner) but rebuilt with a contemporary
architecture and stack.

> **Status: feature parity under way.** 145 of the 155 rows in [`docs/PARITY.md`](docs/PARITY.md)
> are done, seven are partial and one is planned; see [`docs/PLAN.md`](docs/PLAN.md).

## Why a rewrite

Mission Planner is a ~500k-line C#/WinForms codebase that has accumulated three decades of
incremental change. It works, but the architecture (global mutable state on `MainV2`, a
`giveComport` cooperative mutex serialising all request/response traffic, a 4 000-line
`CurrentState` class whose properties recompute on every access, 90 vendored sub-projects) makes
it hard to extend, hard to test, and hard to run anywhere but Windows.

MPCraft keeps the **functionality** and discards the **incumbency**.

| Legacy Mission Planner | MPCraft |
| --- | --- |
| C# / .NET Framework 4.7.2 / WinForms | Rust 2021 (async, `tokio`) |
| Global mutable statics on `MainV2` | Explicit `Session` / `Vehicle` state, owned types |
| `giveComport` semaphore + blocking reads | One reader task, `broadcast` fan-out, per-request `oneshot` |
| `CurrentState` with ~400 lazily-computed properties | Push-update snapshot (`ArcSwap`), computed once per packet |
| WinForms + GMap.NET + ZedGraph | Tauri v2 + React/TS + MapLibre GL JS + uPlot |
| `#if LIB` conditional compilation | Native cross-platform, no preprocessor gymnastics |
| C# plugin loading via Roslyn | WASM plugin sandbox (planned) |

## Architecture at a glance

```
┌──────────────────────────────────────────────────────────────┐
│  apps/ui            React 19 + TS + Vite                      │
│                     MapLibre GL JS · uPlot · TanStack Table    │
└───────────────────────────┬──────────────────────────────────┘
                            │ Tauri IPC (commands + events)
┌───────────────────────────┴──────────────────────────────────┐
│  crates/mpcraft-app        session orchestration, app state   │
├──────────────────────────────────────────────────────────────┤
│  crates/mpcraft-link       transactions, params, missions     │
├──────────────────────────────────────────────────────────────┤
│  crates/mpcraft-vehicle    telemetry snapshot & derived state │
├──────────────────────────────────────────────────────────────┤
│  crates/mpcraft-mavlink    framing · CRC · signing · dialect │
├──────────────────────────────────────────────────────────────┤
│  crates/mpcraft-transport  serial · UDP · TCP · file          │
└──────────────────────────────────────────────────────────────┘
```

Full detail in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Repository layout

```
MPCraft/
├── Cargo.toml              # cargo workspace
├── crates/                 # pure-Rust libraries, no Tauri dependency
│   ├── mpcraft-mavlink/    #   MAVLink wire protocol
│   ├── mpcraft-transport/  #   byte transports
│   ├── mpcraft-vehicle/    #   vehicle state model
│   ├── mpcraft-link/       #   session / transactions
│   ├── mpcraft-mission/    #   missions, fences, rallies
│   ├── mpcraft-geo/        #   geodesy & terrain
│   ├── mpcraft-params/     #   parameter metadata
│   ├── mpcraft-log/        #   tlog / DataFlash / ULog parsers
│   ├── mpcraft-settings/   #   persisted configuration
│   └── mpcraft-app/        #   application orchestration
├── apps/
│   ├── desktop/            #   Tauri v2 binary
│   └── ui/                 #   React + TypeScript frontend
├── docs/                   #   plan, architecture, parity matrix
└── .opencode/              #   agents & skills for AI-assisted development
```

## Build

```bash
# Prerequisites: Rust stable, Node 20+, and the Tauri v2 system dependencies
cargo install tauri-cli --version "^2"

# The ffmpeg the installer carries: the desktop crate copies it beside the compiled binary, so a
# build of the workspace needs the file, and a fresh clone does not have it.
node scripts/fetch-ffmpeg.js

# Rust workspace only (no UI) — fastest feedback loop
cargo build --workspace
cargo test --workspace

# Full desktop app (from apps/ui, where the CLI is declared)
cd apps/ui
npm install
npm run launch

# Installers for this platform, written under target/release/bundle
npm run bundle
```

`npm run launch` and `npm run bundle` fetch the sidecar themselves, so the line above is only for a
`cargo` command run from the root. `--triple <target triple>` writes the build for another machine,
which is what a cross-compiled bundle needs.

### Toolchain notes

The workspace targets **Rust 1.88 or newer** and builds with either the MSVC or the GNU
toolchain.

- **MSVC** requires the Visual Studio Build Tools *including the Windows SDK*. A Visual Studio
  installation without the SDK cannot link, and the failure appears as a confusing
  "the C compiler is not able to compile a simple test program" error from a native
  dependency's build script.
- **GNU** ships its own linker and needs no SDK. If the machine has both, make sure a working
  `make` is on the PATH — the MinGW toolchain provides one, and the `make.exe` that ships with
  some Arduino toolchains cannot build native code.

On Windows, `scripts/env.ps1` selects a working toolchain and prepares the environment:

```powershell
. .\scripts\env.ps1
cargo test --workspace
```

### Icons

The application icons are generated, not hand-drawn:

```powershell
.\scripts\make-icons.ps1
```

## Documentation

| Document | Contents |
| --- | --- |
| [`docs/PLAN.md`](docs/PLAN.md) | Phased roadmap, milestones, exit criteria |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | Crate boundaries, data flow, protocol details |
| [`docs/PARITY.md`](docs/PARITY.md) | Feature-by-feature parity matrix vs Mission Planner |
| [`docs/DECISIONS.md`](docs/DECISIONS.md) | Architecture decision records |
| [`docs/RELEASE.md`](docs/RELEASE.md) | Releases, update signing keys, and the publishing workflow |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | The branch flow, the checks, and what a change carries |
| [`SECURITY.md`](SECURITY.md) | How to report a vulnerability, and what is in scope |

## AI-assisted development

The `.opencode/` directory defines the agents and skills used to develop MPCraft.

| Agent | Owns |
| --- | --- |
| `protocol` | `mpcraft-mavlink`, `mpcraft-transport` |
| `link` | `mpcraft-link`, `mpcraft-vehicle`, `mpcraft-params` |
| `mission` | `mpcraft-mission`, `mpcraft-geo` |
| `logs` | `mpcraft-log` |
| `frontend` | `apps/ui`, `apps/desktop` |
| `architect` | `mpcraft-app`, `mpcraft-settings`, CI, docs, code review |

The skills in `.opencode/skills/` are the operational playbooks: adding a MAVLink message,
adding a transport, adding a MAV_CMD, adding a Tauri command, adding a UI screen, adding
parameter metadata, running SITL tests, porting an upstream feature, and reviewing changes.

## Licence

GPL-3.0-or-later, matching upstream Mission Planner's licensing of ArduPilot ground control
software.

[`docs/PLAN.md`]: docs/PLAN.md
[`docs/ARCHITECTURE.md`]: docs/ARCHITECTURE.md
[`docs/PARITY.md`]: docs/PARITY.md
[`docs/DECISIONS.md`]: docs/DECISIONS.md
