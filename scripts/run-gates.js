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
 *     BLOQUEADO           the gate could run and did not, because the thing it measures was busy
 *                         (an artifact locked by the running application). Not a fault in the code.
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
    control: null,
  },
  {
    name: "prettier",
    layer: "any",
    /** The house rule: only the files the change touched. */
    scopedToChange: true,
    command: "npx",
    probe: ["prettier", "--version"],
    cwd: ROOT,
    control: null,
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
     * A simulated vehicle written from the specification, independently of the Rust code, stands in
     * for a live simulator on this machine, where the simulator image cannot be fetched. It covers
     * the protocols; it does not cover timing, a radio, or a board, so what stands in is narrower
     * than what it replaces.
     */
    substitute:
      "scripts/simulated-vehicle.py, which answers the parameter, mission and command protocols",
    control:
      "the golden-file stream, which is a second implementation rather than a mirror of the parser",
  },
  {
    name: "rust formatting",
    layer: "rust",
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    args: [TOOLCHAIN, "fmt", "--check"],
    cwd: ROOT,
    control: null,
  },
  {
    name: "rust lints",
    layer: "rust",
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [TOOLCHAIN, "clippy", "--all-targets", "--", "-D", "warnings"],
    cwd: ROOT,
    control: null,
  },
  {
    name: "rust tests",
    layer: "rust",
    command: "cargo",
    probe: [TOOLCHAIN, "--version"],
    needs: ["gcc"],
    args: [TOOLCHAIN, "test"],
    cwd: ROOT,
    control: null,
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

console.log(
  JSON.stringify(
    {
      change: files ?? ["git did not answer"],
      layers: [...layers],
      gates: report,
      verdict: gaps.length === 0 ? "PASS" : "FAIL",
      gaps,
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

  if (gate.layer !== "any" && !layers.has(gate.layer)) {
    entry.state = "NO-APLICA";
    entry.why = `nothing in the ${gate.layer} layer was touched`;
    return entry;
  }

  if (gate.substitute) {
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

  /** An artifact locked by the running application says nothing about the code. */
  if (
    /failed to run custom build command|being used by another process/i.test(
      answer.failed,
    )
  ) {
    entry.state = "BLOQUEADO";
    entry.why = "the build output was locked, so nothing was measured";
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

function lineMatching(text, pattern) {
  const line = plain(text)
    .split("\n")
    .find((row) => pattern.test(row));

  return line === undefined ? "no line said what was expected" : line.trim();
}

/** The colours a runner puts in its report are not part of what it says. */
function plain(text) {
  return String(text ?? "").replace(/\u001b\[[0-9;]*m/g, "");
}
