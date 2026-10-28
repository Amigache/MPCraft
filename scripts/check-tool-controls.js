#!/usr/bin/env node
/**
 * The known-bad case of the five gates that read a tool rather than a document: typescript, prettier, rust
 * formatting, rust lints, and rust tests. Before this each of them said only what its exit code said, and a
 * gate that cannot fail is not trusted — the runner's own note says so, and its control column was empty for
 * these five.
 *
 * Each case writes one file, runs the runner with that file as the change, and puts it back. A tool gate can
 * only be shown failing on something, and what it reads is what the tool reaches: a Rust file that no module
 * declares is never compiled and never formatted, so the two Rust cases disturb a file the crate already
 * holds rather than adding one. The others are files the tool reaches on its own — `tsc` takes everything
 * under `src`, `cargo test` takes every file in a `tests` folder as its own target, and prettier takes
 * whatever it is named, which is why the pass side of a scratch case names a file that is still there.
 *
 * What these prove, and what they do not. They prove each gate can fail on what it claims to read, and that
 * the sources pass untouched. Two of them now show the finding itself: rustfmt names the file it reads as
 * different, and the tests gate keeps the line that names the failing test. The other three buy the flip and
 * nothing more, because npm prints its banner before the compiler speaks, prettier opens with its heading, and
 * cargo may open with what it compiled. BLOQUEADO is not exercised here: the lock case belongs to
 * the runner's own controls, where an artifact is genuinely held by another process rather than described by a
 * string that looks like one.
 *
 * The last three checks read the control column itself, which is the column the runner's note points at: every
 * gate names its known-bad case, and the checker under `scripts/` that it names is on the disk. A gate that
 * names nothing, and a control that points at a file which is not there, are both findings, and each is shown
 * firing on a copy of the runner so the runner as it stands is never disturbed.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const ROOT = new URL("../", import.meta.url);
const runner = fileURLToPath(new URL("./run-gates.js", import.meta.url));

/** One run of the runner, with the change it is told to read and the gate it is asked about. */
function run(change, gates) {
  try {
    return JSON.parse(
      execFileSync(
        "node",
        [runner, "--change", ...change, "--gates", ...gates],
        {
          encoding: "utf8",
        },
      ),
    );
  } catch (cause) {
    const out = String(cause.stdout ?? "");

    try {
      return JSON.parse(out);
    } catch {
      return { error: out || String(cause.message) };
    }
  }
}

const entryOf = (report, gate) =>
  (report.gates ?? []).find((entry) => entry.gate === gate);

const checks = [];

/**
 * Writes one file, runs one gate against it as the change, puts it back, and runs the same gate again on what
 * is left. The case is a control only when the gate fails on what was written, passes afterwards, and leaves
 * nothing behind — which is why the change names one file: a tool that fails on a change of one file can only
 * have failed on that file.
 */
function show({ gate, path, goodPath, wrote, expect, note }) {
  const full = fileURLToPath(new URL(path, ROOT));
  const was = existsSync(full) ? readFileSync(full, "utf8") : null;
  let failing = null;

  try {
    writeFileSync(full, wrote(was), "utf8");
    failing = run([path], [gate]);
  } finally {
    if (was === null) {
      rmSync(full, { force: true });
    } else {
      writeFileSync(full, was, "utf8");
    }
  }

  /* A file the tool reaches on its own cannot be shown passing after it is gone, so the pass side names a
     file that is still there; a file that was disturbed is shown passing with its own bytes back. */
  const passing = run([goodPath ?? path], [gate]);
  const bad = entryOf(failing, gate);
  const good = entryOf(passing, gate);
  const leftBehind =
    was === null ? existsSync(full) : readFileSync(full, "utf8") !== was;

  checks.push({
    claim: `the ${gate} gate fails on the case it is shown and passes afterwards${note}`,
    ok:
      bad?.state === "FAIL" &&
      good?.state === "PASS" &&
      !leftBehind &&
      (failing.change ?? []).includes(path) &&
      (expect === null || expect.test(String(bad.detail ?? ""))),
    saw: {
      bad: { state: bad?.state, detail: bad?.detail },
      good: { state: good?.state, proof: good?.proof },
      leftBehind,
    },
  });
}

/* A word written with the quotes prettier is asked not to use. Its report opens with a heading rather than
   with the file, so this case shows the exit code turning — which is what its green rests on anyway. */
show({
  gate: "prettier",
  path: "apps/ui/src/lib/i18n/__control_quotes__.ts",
  goodPath: "apps/ui/src/lib/i18n/written-words.test.ts",
  wrote: () => "export const say = 'a word written with the wrong quotes'\n",
  expect: null,
  note: ", and shows only the flip, because prettier opens with its heading",
});

/* A value that is not the type its declaration says, and which is otherwise prettier-clean, so only the rule
   it tests can speak. npm prints its banner before the compiler, so the finding is not what the runner keeps. */
show({
  gate: "typescript",
  path: "apps/ui/src/lib/i18n/__control_type__.ts",
  goodPath: "apps/ui/src/lib/ipc/types.ts",
  wrote: () => 'export const controlNumber: number = "not a number";\n',
  expect: null,
  note: ", and shows only the flip, because npm answers before tsc",
});

/* A line with one space too many, which rustfmt reads as a difference and clippy does not read at all. The
   file is one the crate already holds, because cargo fmt walks the module tree rather than a folder — and this
   is the one case where the first line the runner keeps is the finding itself. */
show({
  gate: "rust formatting",
  path: "crates/mpcraft-settings/src/profiles.rs",
  wrote: (text) =>
    text.replace("    pub name: String,", "    pub  name: String,"),
  expect: /profiles\.rs/,
  note: ", and the diff names the file it would rewrite",
});

/* A boolean expression that says itself twice, which is a rule clippy carries by default and that
   `-D warnings` turns into a failure. The same file is formatted as the formatting gate expects, so only the
   lint can speak; the report may open with what cargo compiled, so this case shows the flip. */
show({
  gate: "rust lints",
  path: "crates/mpcraft-settings/src/lib.rs",
  wrote: (text) =>
    `${text}\n/// The known-bad case of the lint gate: the tool flags this and nothing else.\npub fn control_lint_case(flag: bool) -> bool {\n    if flag {\n        true\n    } else {\n        false\n    }\n}\n`,
  expect: null,
  note: ", and shows only the flip, because cargo may open with what it compiled",
});

/* A test that fails. Its green normally rests on the count of binaries that said ok, and the report comes behind
   whatever cargo printed first; now the runner keeps the line that names the failing test, so this case buys the
   finding as well as the flip. */
show({
  gate: "rust tests",
  path: "crates/mpcraft-app/tests/__control_test__.rs",
  goodPath: "crates/mpcraft-app/tests/plugins_wire.rs",
  wrote: () => "#[test]\nfn control_case() {\n    assert!(false);\n}\n",
  expect: /control_case/,
  note: ", which is what the runner keeps when a line names the finding",
});

/*
 * The control column, read as a document rather than remembered: every gate names its known-bad case, and
 * the checker it names is on the disk. A claim that points at a file which is not there is a claim nothing
 * can show, so both halves are read — a gate that says nothing about its negative control, and a gate that
 * names a checker that does not exist. The known-bad cases are copies of the runner put in front of the
 * reading, so the runner as it stands is never disturbed.
 */
function readControlColumn(text) {
  const table = text.match(/const GATES = \[([\s\S]*?)\n\];/);
  const rows = [];

  for (const block of (table?.[1] ?? "").split("\n  {")) {
    const name = block.match(/name: "([^"]+)"/)?.[1];

    if (name === undefined) {
      continue;
    }

    const said = block.match(/control:\s*(?:\n\s+)?"([^"]*)"/);
    const control = said === null ? null : said[1];
    const named = (control ?? "").match(/scripts\/[\w.\-]+\.js/);

    rows.push({
      name,
      finding:
        control === null
          ? "names no known-bad case"
          : named === null
            ? "names no checker under scripts/"
            : existsSync(new URL(named[0], ROOT))
              ? null
              : `names ${named[0]}, which is not on the disk`,
    });
  }

  return rows;
}

/** Reads a copy of the runner, shows the reading firing on it, and leaves nothing behind. */
function showReading({ wrote, expect }) {
  const scratch = fileURLToPath(new URL("scripts/__control_column__.js", ROOT));
  let fired = [];

  try {
    writeFileSync(scratch, wrote(readFileSync(runner, "utf8")), "utf8");

    fired = readControlColumn(readFileSync(scratch, "utf8")).filter(
      (row) => row.finding !== null,
    );
  } finally {
    /* Nothing can be said about what is left behind until the copy is gone. */
    rmSync(scratch, { force: true });
  }

  checks.push({
    claim: `the control column fires on ${expect}`,
    ok: fired.length === 1 && !existsSync(scratch),
    saw: { gate: fired[0]?.name, finding: fired[0]?.finding },
  });
}

const untouched = readControlColumn(readFileSync(runner, "utf8"));

checks.push({
  claim:
    "every gate names its known-bad case, and the checker it names is on the disk",
  ok: untouched.every((row) => row.finding === null),
  saw: { gates: untouched.length },
});

showReading({
  wrote: (text) =>
    text.replace(
      /control:\n      "scripts\/check-parity-controls\.js[^"]*"/,
      "control: null,",
    ),
  expect: "a gate that says nothing about its negative control",
});

showReading({
  wrote: (text) =>
    text.replace(
      "scripts/check-parity-controls.js",
      "scripts/__control_missing__.js",
    ),
  expect: "a control that points at a checker which is not there",
});

const fails = checks.filter((check) => !check.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove each tool gate can fail on what it claims to read and that the sources pass afterwards. They do not show the tool reading anything outside the file the control wrote; one of them shows the finding itself, because the runner keeps the first line of what a tool says, and the other four show only the state turning. The two Rust cases must disturb a file the crate already holds, because a file no module declares is neither compiled nor formatted, and a scratch file cannot be shown passing after it is gone, so the pass side names a file that is still there. The control column is read as a document: a gate that names nothing, and a checker under scripts/ that is not on the disk, are findings; a path named outside scripts/ — a spec file, a test file inside a crate — is not read as a path, because the reading does not know which layer a sentence about a case belongs to. BLOQUEADO is exercised by the runner's own controls, where an artifact is genuinely held by another process.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
