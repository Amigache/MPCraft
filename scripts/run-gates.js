#!/usr/bin/env node
/**
 * The gates, run together, with what each one says about itself.
 *
 * A checker that did not run is not a checker that passed. In a log that records only green the
 * two look alike, so "did not run" is split into the things it can be:
 *
 *     NO-APLICA           the layer this gate looks at was not touched, so there is nothing for
 *                         it to say. Honest, but not evidence.
 *     HERRAMIENTA-AUSENTE the tool is not there, so nothing was measured. On a layer that was
 *                         touched that is a failure of evidence, not a pass.
 *     SUSTITUIDA          something else stands in for this gate; what stands in is named,
 *                         because a substitute covers less than the thing it replaces.
 *     BLOQUEADO           the gate could run and did not, because the thing it measures was busy —
 *                         an artifact held by another process, which is not a fault in the code.
 *
 * Two more things are said rather than assumed: what each gate has as its negative control — the
 * known-bad case that proves it can fail — and what its green actually rests on, which is a count
 * or a line from its own output, not the exit code alone.
 *
 * Run it from the repository:
 *
 *     node scripts/run-gates.js
 *     node scripts/run-gates.js --change docs/PLAN.md       # what the labels say for a change
 *     GATE_PATH="" node scripts/run-gates.js --change crates/mpcraft-link/lib.rs
 *
 * The two forms above are what `scripts/run-gates-controls.js` drives, so the labels are shown
 * working before their green is trusted.
 */

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";

import { lineMatching, plain, resultForSection } from "./gate-proof.js";

const ROOT = new URL("../", import.meta.url);
const UI = new URL("./apps/ui/", ROOT);

const TOOLCHAIN = "+stable-x86_64-pc-windows-gnu";
const MINGW = "C:/Users/Byte.DESKTOP-JN9IU2Q/winlibs/mingw64/bin";

/** Which layer a path belongs to, and therefore which gate would have something to say about it. */
function layerOf(path) {
  if (path.startsWith("apps/ui/")) {
    return "frontend";
  }

  if (path.startsWith("crates/") || path.startsWith("apps/desktop/")) {
    return "rust";
  }

  if (path.startsWith("scripts/")) {
    return "tooling";
  }

  if (path.startsWith("docs/")) {
    return "documentation";
  }

  return "other";
}

const GATES = [
  {
    name: "typescript",
    layer: "frontend",
    command: "npm",
    probe: ["--version"],
    args: ["run", "typecheck"],
    cwd: UI,
    control:
      "scripts/check-tool-controls.js: a value that is not the type its declaration says, shown firing and the sources passing after it is put back",
  },
  {
    name: "frontend lint",
    layer: "frontend",
    /**
     * The gate reads the sources this project can change and names what it leaves out: the vendored
     * MapLibre worker under public/, and the React JSX rules, because the plugin for those cannot run on
     * the version of eslint declared in apps/ui/package.json. A rule that is not carried cannot be
     * switched off, so a source file that disables one is a finding rather than a preference.
     */
    command: "npm",
    probe: ["--version"],
    args: ["run", "lint"],
    cwd: UI,
    control:
      "scripts/check-lint-controls.js: a name written and never read, a dependency list that does not match what the effect reads, and a disable comment for a rule the configuration does not register",
    /** eslint says nothing when it is happy, so its green rests on the exit code alone. */
    proof: (text) =>
      /(\d+) problems/.test(text)
        ? text.match(/(\d+) problems/)[0]
        : "no findings; its green rests on the exit code alone",
  },
  {
    name: "prettier",
    layer: "any",
    /** The house rule: only the files the change touched. */
    scopedToChange: true,
    command: "npx",
    probe: ["prettier", "--version"],
    cwd: ROOT,
    control:
      "scripts/check-tool-controls.js: a word written with the quotes prettier is asked not to use — its green rests on the exit code, so this case shows the exit code turning. scripts/run-gates-controls.js: a file held by another process, which turns the state to BLOQUEADO and is measured again once the handle is let go",
  },
  {
    name: "frontend tests",
    layer: "frontend",
    command: "npm",
    probe: ["--version"],
    args: ["test"],
    cwd: UI,
    control:
      "scripts/run-gates-controls.js, scripts/check-overlaps-controls.js, and the known-bad word inside the catalogue gate",
    proof: (text) => lineMatching(text, /Tests\s+\d+ passed/),
  },
  {
    name: "playwright flows",
    layer: "frontend",
    command: "npx",
    probe: ["playwright", "--version"],
    args: ["playwright", "test"],
    cwd: UI,
    /** What it covers that the unit tests do not: the flows, and the boxes at the two sizes in the language chosen. */
    control:
      "the known-bad pair inside e2e/layout.spec.ts, and scripts/check-overlaps-controls.js for the checker itself",
    proof: (text) => lineMatching(text, /\d+ passed/),
  },
  {
    name: "integration against SITL",
    layer: "any",
    /**
     * A live simulator answers this gate when MPCRAFT_SITL_LIVE names it, because what a live
     * autopilot adds is what a script cannot imitate: its own values, its own encoding, its own
     * streaming. Without one on the machine, a simulated vehicle written from the specification
     * stands in; it covers the protocols but not timing, a radio, or a board, so what stands in is
     * narrower than what it replaces.
     */
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [
      TOOLCHAIN,
      "test",
      "-p",
      "mpcraft-link",
      "--features",
      "sitl-tests",
      "--",
      "--test-threads=1",
    ],
    cwd: ROOT,
    liveWhen: (env) => env.MPCRAFT_SITL_LIVE === "1",
    substitute:
      "scripts/simulated-vehicle.py, which answers the parameter, mission and command protocols",
    control:
      "the golden-file stream, which is a second implementation rather than a mirror of the parser, and scripts/run-gates-controls.js for the report line this gate reads",
    /** The report of the live binary, not the first count the run prints. */
    proof: (text) => resultForSection(text, "sitl"),
  },
  {
    name: "the table read against a live board",
    layer: "rust",
    /**
     * Each live area needs an instance of its own: one instance serves one connection and stops when that
     * connection goes away, so two binaries cannot share one vehicle. Without a second peer named, the
     * scripted board stands in — it answers the asks, but it does not decide which rows an airframe can
     * speak, which is what this gate measures.
     */
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [
      TOOLCHAIN,
      "test",
      "-p",
      "mpcraft-app",
      "--features",
      "sitl-tests",
      "--test",
      "streams_live",
      "--",
      "--test-threads=1",
    ],
    cwd: ROOT,
    liveWhen: (env) =>
      env.MPCRAFT_SITL_LIVE === "1" && Boolean(env.MPCRAFT_SITL_TABLE_TARGET),
    substitute:
      "the scripted board in crates/mpcraft-app/tests/streams.rs, which answers each ask but cannot say which rows an airframe can speak",
    control:
      "scripts/run-gates-controls.js for the report line this gate reads, shown against a transcript where one area passed and the other failed",
    proof: (text) => resultForSection(text, "streams_live"),
  },
  {
    name: "the board read as a vehicle",
    layer: "rust",
    /**
     * A board on a port answers this gate when MPCRAFT_HARDWARE_TARGET names it, because the things it says
     * about itself are things a simulator cannot say: the model it is, the address it chose for itself, the card
     * that failed to start, and the answer it gives when asked for logs it does not hold. Without one on the
     * machine a scripted board stands in — it prints a banner and answers as a vehicle, but it has no hardware
     * to name and no address of its own, so what stands in is narrower than what it replaces.
     */
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [
      TOOLCHAIN,
      "test",
      "-p",
      "mpcraft-app",
      "--features",
      "hardware-tests",
      "--test",
      "hardware_live",
      "--",
      "--test-threads=1",
    ],
    cwd: ROOT,
    liveWhen: (env) => Boolean(env.MPCRAFT_HARDWARE_TARGET),
    substitute:
      "the scripted board in crates/mpcraft-app/tests/plaintext.rs, which prints text and answers as a vehicle but cannot name its own model or choose its own address",
    control:
      "scripts/run-gates-controls.js for the report line this gate reads, shown against a transcript where the board area failed while another binary passed",
    proof: (text) => resultForSection(text, "hardware_live"),
  },
  {
    name: "the surface read against a live vehicle",
    layer: "rust",
    /**
     * The numbers a plugin reads are only worth something when an autopilot reports them. Without an
     * instance of its own named by the environment, the scripted peer in tests/plugins_wire.rs stands in:
     * it answers the shapes, but it was written to carry the numbers, so it cannot show that a vehicle
     * decides its own — which is the whole of what this area adds over the scripted half.
     */
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [
      TOOLCHAIN,
      "test",
      "-p",
      "mpcraft-app",
      "--features",
      "sitl-tests",
      "--test",
      "plugins_live",
      "--",
      "--test-threads=1",
    ],
    cwd: ROOT,
    liveWhen: (env) =>
      env.MPCRAFT_SITL_LIVE === "1" && Boolean(env.MPCRAFT_SITL_PLUGINS_TARGET),
    substitute:
      "the scripted peer in crates/mpcraft-app/tests/plugins_wire.rs, which puts numbers on a socket but does not decide them",
    control:
      "scripts/run-gates-controls.js for the report line this gate reads, shown against a transcript where the scripted half passed and the live half failed",
    proof: (text) => resultForSection(text, "plugins_live"),
  },
  {
    name: "the board on the bus, read as it answers",
    layer: "rust",
    /**
     * A board in its bootloader answers this gate when MPCRAFT_DFU_TARGET names the pair it is on, because the
     * things it says about itself are things a scripted peer cannot say: the nine bytes its interface puts on the
     * cable, the memory string its own firmware writes, and which of the two published forms of the mass-erase
     * order it understands. Without a present instance the scripted peer stands in — it answers the requests, but
     * it was written to carry them, so what stands in is narrower than what it replaces. Two things the run
     * cannot do are said rather than assumed: it cannot put the board on the bus, because a Matek F405-CTR gets to
     * its bootloader by holding BOOT at power-on or by the command that holds it there, which is what the operator
     * supplies; and one process holds the device at a time, so the area runs alone.
     */
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [
      TOOLCHAIN,
      "test",
      "-p",
      "mpcraft-app",
      "--features",
      "hardware-tests",
      "--test",
      "dfu_live",
      "--",
      "--test-threads=1",
    ],
    cwd: ROOT,
    liveWhen: (env) => Boolean(env.MPCRAFT_DFU_TARGET),
    substitute:
      "the scripted peer in crates/mpcraft-app/tests/dfu.rs, which answers the requests without being a board that decides them",
    control:
      "scripts/run-gates-controls.js for the report line this gate reads, shown against a transcript where the scripted half passed and the board area failed, and for the stand-in named when no pair is named",
    proof: (text) => resultForSection(text, "dfu_live"),
  },
  {
    name: "the rows this machine writes, read as it writes them",
    layer: "rust",
    /**
     * The rules about which port belongs to Bluetooth are read against published sources, and the literals those
     * rules were written against are carried by the tests in `crates/mpcraft-transport/src/bluetooth.rs`. What only
     * the machine can say is what this area reads: the rows `HKLM\HARDWARE\DEVICEMAP\SERIALCOMM` carries today, the
     * numbers the two registry calls answered for them, and what one instance says about itself — which is a Ports
     * class with the RFCOMM service behind it, so membership comes from the service and not from the class. Without
     * a row named by the environment the scripted peer in `crates/mpcraft-app/tests/bluetooth_peer.rs` stands in: it
     * answers the transport contract without opening a port, so what stands in is narrower than what it replaces —
     * it cannot show what a machine writes in its own registry. Two things are said rather than assumed: nothing is
     * opened on COM40, because that port belongs to a module that may be using it, and no peripheral on this machine
     * advertises the Nordic UART service, so upstream's enumeration filter has nothing to find and its half stays
     * outside the crate, since no runtime is carried here.
     */
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [
      TOOLCHAIN,
      "test",
      "-p",
      "mpcraft-app",
      "--features",
      "hardware-tests",
      "--test",
      "bluetooth_live",
      "--",
      "--test-threads=1",
    ],
    cwd: ROOT,
    liveWhen: (env) => Boolean(env.MPCRAFT_BT_ROW),
    substitute:
      "the scripted peer in crates/mpcraft-app/tests/bluetooth_peer.rs, which answers the transport contract without opening a port, and the literals written from published sources, which cannot show what a machine writes about itself",
    control:
      "scripts/run-gates-controls.js for the report line this gate reads, shown against a transcript where the scripted half passed and the area reading the machine failed, and for the stand-in named when no row is named",
    proof: (text) => resultForSection(text, "bluetooth_live"),
  },
  {
    name: "parity matrix",
    layer: "documentation",
    command: "node",
    probe: ["--version"],
    args: ["scripts/check-parity.js"],
    cwd: ROOT,
    control:
      "scripts/check-parity-controls.js, which shows each rule firing on a known-bad row and the matrix passing untouched",
    proof: (text) => lineMatching(text, /"verdict": "PASS"/),
  },
  {
    name: "the launch, read as the documents say it is",
    layer: "any",
    /**
     * The launch is written across five documents that do not belong to one layer — the entry, the two
     * commands the CLI runs, the port the dev server binds, and where the build writes — so this gate is
     * scoped by what it reads rather than by a layer, and says so when none of them was touched.
     */
    command: "node",
    probe: ["--version"],
    args: ["scripts/check-launch.js"],
    cwd: ROOT,
    reads: [
      "apps/ui/package.json",
      "apps/desktop/package.json",
      "apps/desktop/tauri.conf.json",
      "apps/ui/vite.config.ts",
      "apps/ui/playwright.config.ts",
    ],
    control:
      "scripts/check-launch-controls.js, which shows each rule firing on a known-bad document and the five documents passing untouched",
    proof: (text) => lineMatching(text, /"verdict": "PASS"/),
  },
  {
    name: "the contract, read from both sides",
    layer: "any",
    /**
     * The boundary is one document spoken two ways, so this gate is scoped by what it reads rather than by a
     * layer: the Rust side writes the JSON and the TypeScript side declares what it reads, and the documents
     * that cross it are reached through types the crates declare — which is why one of its entries is a folder.
     */
    command: "node",
    probe: ["--version"],
    args: ["scripts/check-contract.js"],
    cwd: ROOT,
    reads: [
      "apps/desktop/src/main.rs",
      "apps/ui/src/lib/ipc/types.ts",
      "apps/ui/src/lib/ipc/bridge.ts",
      "apps/ui/src/lib/link/linkStore.ts",
      "crates/",
    ],
    control:
      "scripts/check-contract-controls.js, which shows each rule firing on a known-bad source and the sources passing untouched",
    proof: (text) => lineMatching(text, /"verdict": "PASS"/),
  },
  {
    name: "the DFU numbers, worked out again",
    layer: "any",
    /**
     * The golden literals live in one file and the writer that makes the bytes they are worked out over lives in
     * another, so this gate is scoped by what it reads rather than by a layer: the literals are read as text and
     * the numbers are worked out by a table written here from the polynomial, which is what says they did not
     * come from the module that carries them.
     */
    command: "node",
    probe: ["--version"],
    args: ["scripts/check-dfu-crc.js"],
    cwd: ROOT,
    reads: ["crates/mpcraft-app/tests/dfu.rs", "crates/mpcraft-app/src/dfu.rs"],
    control:
      "scripts/check-dfu-crc-controls.js, which shows each rule firing on a known-bad copy — a literal that disagrees with its own table, a pair written as agreeing, a writer whose shapes disagree with the format — and the two files passing untouched",
    proof: (text) => lineMatching(text, /"verdict": "PASS"/),
  },
  {
    name: "rust formatting",
    layer: "rust",
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    args: [TOOLCHAIN, "fmt", "--check"],
    cwd: ROOT,
    control:
      "scripts/check-tool-controls.js: a line with one space too many, which rustfmt reads as a difference and clippy does not read at all — the file has to be one the crate already holds, because cargo fmt walks the module tree rather than a folder",
  },
  {
    name: "rust lints",
    layer: "rust",
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [TOOLCHAIN, "clippy", "--all-targets", "--", "-D", "warnings"],
    cwd: ROOT,
    control:
      "scripts/check-tool-controls.js: a boolean expression that says itself twice, which -D warnings turns into a failure, on a file formatted as the formatting gate expects so that only the lint can speak",
  },
  {
    name: "rust tests",
    layer: "rust",
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [TOOLCHAIN, "test"],
    cwd: ROOT,
    control:
      "scripts/check-tool-controls.js: a test that fails; the report is written behind the first line the runner keeps, so this case buys the flip rather than the finding",
    /** The count of test binaries that said ok, which is what its green means. */
    proof: (text) => (text.match(/test result: ok\./g) ?? []).length,
  },
];

/** The change the gates are read against: what git says, or what the control hands in. */
function changedFiles() {
  const handed = option("--change");

  if (handed.length > 0) {
    return handed.map((path) => path.replaceAll("\\", "/"));
  }

  const output = call("git", ["status", "--porcelain"], { cwd: ROOT });

  if (typeof output !== "string") {
    return null;
  }

  return output
    .split("\n")
    .filter((line) => line.length > 3)
    .map((line) => line.slice(3).replaceAll("\\", "/"));
}

const argv = process.argv.slice(2);

/** A flag and the words that belong to it, up to the next flag. */
function option(name) {
  const at = argv.indexOf(name);

  if (at === -1) {
    return [];
  }

  const words = [];

  for (let i = at + 1; i < argv.length && !argv[i].startsWith("--"); i += 1) {
    words.push(argv[i]);
  }

  return words;
}

const files = changedFiles();
const layers = new Set((files ?? []).map(layerOf));
const only = option("--gates");

if (process.env.GATE_PATH !== undefined) {
  process.env.PATH = process.env.GATE_PATH;
} else if (existsSync(MINGW)) {
  process.env.PATH = `${MINGW};${process.env.PATH}`;
}

const report = GATES.filter(
  (gate) => only.length === 0 || only.includes(gate.name),
).map(assess);

/** A gate that could not run on a layer this change touched is a gap, not a pass. */
const gaps = report.filter((entry) =>
  ["FAIL", "BLOQUEADO", "HERRAMIENTA-AUSENTE"].includes(entry.state),
);

/**
 * What the whole run cannot see, said in what it reports rather than only in its header: a run sees what each
 * tool answers on the files it reaches, and nothing else — no window, no vehicle, no board. A layer that was not
 * touched is named as such, and a gate something stands in for covers less than what it replaces.
 */
const LIMIT =
  "what the run sees is what each tool answers on the files it reaches. It does not see a window, a vehicle, or a board: a layer that was not touched is said as NO-APLICA rather than counted as evidence, a gate something stands in for covers less than what it replaces, and a tool gate shows the state turning rather than the finding whenever the tool opens with something other than the finding.";

console.log(
  JSON.stringify(
    {
      change: files ?? ["git did not answer"],
      layers: [...layers],
      gates: report,
      verdict: gaps.length === 0 ? "PASS" : "FAIL",
      gaps,
      limit: LIMIT,
      note: "NO-APLICA is honest but is not evidence: the layer it would have read was not touched. A gate that cannot fail is not trusted, and the control column says which gates have a known-bad case.",
    },
    null,
    2,
  ),
);

process.exit(gaps.length === 0 ? 0 : 1);

/** What one gate says about itself: what it measured, or why it could not. */
function assess(gate) {
  const entry = { gate: gate.name, control: gate.control };

  if (gate.reads) {
    /* A gate scoped by what it reads says so when none of those documents was touched. */
    const touched = (files ?? []).filter((path) =>
      gate.reads.some((entry) =>
        // An entry may be a folder, which is how a gate that reads a whole layer is scoped.
        entry.endsWith("/") ? path.startsWith(entry) : path === entry,
      ),
    );

    if (touched.length === 0) {
      entry.state = "NO-APLICA";
      entry.why = "none of the documents this gate reads was touched";
      return entry;
    }

    entry.scope = touched;
  } else if (gate.layer !== "any" && !layers.has(gate.layer)) {
    entry.state = "NO-APLICA";
    entry.why = `nothing in the ${gate.layer} layer was touched`;
    return entry;
  }

  if (gate.substitute && !(gate.liveWhen?.(process.env) ?? false)) {
    entry.state = "SUSTITUIDA";
    entry.why = gate.substitute;
    return entry;
  }

  if (gate.scopedToChange) {
    /** Prose is not prettier-formatted in this repository, so the gate reads code only. */
    const touched = (files ?? []).filter((path) =>
      /\.(ts|tsx|js|mjs|css)$/.test(path),
    );

    if (touched.length === 0) {
      entry.state = "NO-APLICA";
      entry.why = "no file the change touched";
      return entry;
    }

    entry.scope = touched;
    entry.args = ["prettier", "--check", ...touched];
  } else {
    entry.args = gate.args;
  }

  const tool = toolPresent(gate.command, gate.probe);

  if (!tool.ok) {
    entry.state = "HERRAMIENTA-AUSENTE";
    entry.why = tool.reason;
    return entry;
  }

  entry.tool = tool.version;

  for (const helper of gate.needs ?? []) {
    const answer = toolPresent(helper, ["--version"]);

    if (!answer.ok) {
      entry.state = "HERRAMIENTA-AUSENTE";
      entry.why = `the tool answers, its helper does not: ${answer.reason}`;
      return entry;
    }
  }

  const answer = call(tool.path, [...entry.args], { cwd: gate.cwd });

  if (typeof answer === "string") {
    entry.state = "PASS";

    if (gate.proof) {
      entry.proof = gate.proof(answer);
    } else if (plain(answer).trim() === "") {
      /** A gate that says nothing when it is happy rests on its exit code and nothing else. */
      entry.proof = "said nothing; its green rests on the exit code alone";
    } else {
      entry.proof = firstLine(answer);
    }

    return entry;
  }

  if (answer.missingTool) {
    entry.state = "HERRAMIENTA-AUSENTE";
    entry.why = answer.missingTool;
    return entry;
  }

  /** A browser that is not installed is a missing tool, not a flow that went wrong. */
  if (/browsers are not installed|browserType\.launch/i.test(answer.failed)) {
    entry.state = "HERRAMIENTA-AUSENTE";
    entry.why = "the tool answers but its runtime is not installed";
    entry.detail = firstLine(answer.failed);
    return entry;
  }

  /**
   * An artifact that could not be read or written because something else holds it says nothing about
   * the code. Both spellings of a busy artifact are read: the sharing violation a tool reports when it
   * cannot open a file, and the one cargo reports when it cannot overwrite a build output.
   */
  if (
    /failed to run custom build command|being used by another process|resource busy or locked/i.test(
      answer.failed,
    )
  ) {
    entry.state = "BLOQUEADO";
    entry.why =
      "the artifact was held by another process, so nothing was measured";
    entry.detail = firstLine(answer.failed);
    return entry;
  }

  entry.state = "FAIL";
  entry.detail = firstLine(answer.failed);
  return entry;
}

/**
 * Runs one gate and hands back what it said.
 *
 * Both streams are read, because a test runner writes its report on the error stream: reading
 * only the output stream would make a green run look like a run that said nothing.
 */
function call(command, args, options) {
  /** A program found on the path can hold a space, and the shell needs it quoted. */
  const program = /\s/.test(command) ? `"${command}"` : command;

  const result = spawnSync(program, args, {
    encoding: "utf8",
    shell: true,
    ...options,
  });

  const text = `${result.stdout ?? ""}${result.stderr ?? ""}`;

  if (result.status === 0) {
    return text;
  }

  if (
    /could not be found|no such file or directory|command not found/i.test(text)
  ) {
    return { missingTool: `${command} ${args.join(" ")}` };
  }

  return { failed: text };
}

/** Whether a program is on the path, which is decided by looking, not by what a shell says. */
function findOnPath(name) {
  const dirs = String(process.env.PATH ?? "").split(";");

  for (const dir of dirs) {
    const base = dir.replace(/[\\/]+$/, "");

    for (const suffix of ["", ".exe", ".cmd"]) {
      const full = `${base}/${name}${suffix}`;

      if (base !== "" && existsSync(full)) {
        return full;
      }
    }
  }

  return null;
}

/** Whether a tool answers at all, which is what HERRAMIENTA-AUSENTE turns on. */
function toolPresent(command, probe) {
  const found = findOnPath(command);

  if (found === null) {
    return { ok: false, reason: `the tool is not on the path: ${command}` };
  }

  const answer = call(found, probe, {});

  if (typeof answer === "string") {
    return { ok: true, path: found, version: firstLine(answer) };
  }

  if (answer.missingTool) {
    return {
      ok: false,
      reason: `the tool is not there: ${found} ${probe.join(" ")}`,
    };
  }

  return { ok: false, reason: firstLine(answer.failed) };
}

function firstLine(text) {
  const line = plain(text)
    .split("\n")
    .find((row) => row.trim() !== "");

  return line === undefined ? "" : line.trim();
}

/** What a gate reads is in `scripts/gate-proof.js`, so the reading can be shown firing on its own. */
