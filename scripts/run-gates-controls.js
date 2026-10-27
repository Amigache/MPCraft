#!/usr/bin/env node
/**
 * The controls of the gate runner: what it claims about itself, each shown doing what it claims.
 *
 *     it can fail          a word through the catalogue with no entry, and a known-bad scratch
 *                          file that writes that word, is put through it
 *     it can pass          the same run without the scratch file
 *     it names what did not run, and does not read it as green   a layer that was not touched, a
 *                          tool that is not there, and a gate something else stands in for
 *     it can be scoped by what it reads   one of the documents a gate names is put in front of it, and a
 *                          change that touches none of them; the same reading, with a folder among the
 *                          documents, is shown answering for a file inside it
 *     which count it reads   a report where the unit tests passed and the live areas failed is read as
 *                          the live areas, because those are the ones a vehicle answered
 *     a busy artifact      one file is held by a separate process with no sharing at all, so the gate could
 *                          run and did not; the same file is measured again once the handle is let go
 *     what the run cannot see   the runner names its reach inside what it reports, and the reading is shown
 *                          firing on two copies of itself: one that names no limit, and one whose limit says
 *                          only how the states are read
 *
 * What these prove, and what they do not: they prove the states are reachable and that the exit
 * code follows them. They do not prove that a gate that runs is strong — a gate whose green rests
 * on a count it cannot fail on is outside of what these cases buy. The lock is real, but the process
 * holding it is the control's own rather than the running application, because Node cannot choose the
 * share mode of a handle and the application is not running here; so what this exercises is the rule
 * that reads a busy artifact as a gap rather than as green, not a file locked by MPCraft itself.
 */

import { execFileSync, spawn } from "node:child_process";
import {
  closeSync,
  existsSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { fileURLToPath } from "node:url";

import { lineMatching, resultForSection } from "./gate-proof.js";

const ROOT = new URL("../", import.meta.url);
const runner = fileURLToPath(new URL("./run-gates.js", import.meta.url));
const scratch = fileURLToPath(
  new URL("./apps/ui/src/lib/i18n/__control_word__.ts", ROOT),
);

/** One run of the runner, with the change it is told to read and the gates it is asked about. */
function run(change, gates, env) {
  try {
    const out = execFileSync(
      "node",
      [runner, "--change", ...change, "--gates", ...gates],
      {
        encoding: "utf8",
        env: { ...process.env, ...(env ?? {}) },
      },
    );

    return JSON.parse(out);
  } catch (cause) {
    const out = String(cause.stdout ?? "");

    try {
      return JSON.parse(out);
    } catch {
      return { error: out || String(cause.message) };
    }
  }
}

const stateOf = (report, gate) =>
  (report.gates ?? []).find((entry) => entry.gate === gate)?.state;

const checks = [];

// The known-bad case: a word written through the catalogue that has no entry.
const word = "A word the control adds";

writeFileSync(
  scratch,
  `export const say = (t: (source: string) => string) => t("${word}");\n`,
  "utf8",
);

try {
  const bad = run(
    ["apps/ui/src/lib/i18n/__control_word__.ts"],
    ["frontend tests"],
  );

  checks.push({
    claim: "the runner fails on a word with no entry",
    ok: bad.verdict === "FAIL" && stateOf(bad, "frontend tests") === "FAIL",
    saw: { verdict: bad.verdict, gate: stateOf(bad, "frontend tests") },
  });
} finally {
  rmSync(scratch, { force: true });
}

checks.push({
  claim: "the scratch file is left behind nothing",
  ok: !existsSync(scratch),
  saw: existsSync(scratch) ? "still there" : "gone",
});

// The known-good case: the same run, with the word not written by anything.
const good = run(
  ["apps/ui/src/lib/i18n/written-words.test.ts"],
  ["frontend tests"],
);

checks.push({
  claim: "the runner passes when the catalogue covers what is written",
  ok: good.verdict === "PASS" && stateOf(good, "frontend tests") === "PASS",
  saw: {
    verdict: good.verdict,
    gate: stateOf(good, "frontend tests"),
    proof: good.gates?.[0]?.proof,
  },
});

// What did not run, in its three kinds: not applicable, tool missing, and stood in for.
const notApplicable = run(["docs/PLAN.md"], ["frontend tests"]);

checks.push({
  claim: "a layer that was not touched says NO-APLICA rather than green",
  ok: stateOf(notApplicable, "frontend tests") === "NO-APLICA",
  saw: {
    verdict: notApplicable.verdict,
    gate: stateOf(notApplicable, "frontend tests"),
  },
});

/* A gate scoped by what it reads, shown both ways: one of its documents touched, and none. */
const launchGate = "the launch, read as the documents say it is";

const launchTouched = run(["apps/desktop/tauri.conf.json"], [launchGate]);

checks.push({
  claim:
    "a gate scoped by the documents it reads runs when one is touched, and names which",
  ok:
    stateOf(launchTouched, launchGate) === "PASS" &&
    (launchTouched.gates?.[0]?.scope ?? []).includes(
      "apps/desktop/tauri.conf.json",
    ),
  saw: {
    verdict: launchTouched.verdict,
    gate: stateOf(launchTouched, launchGate),
    scope: launchTouched.gates?.[0]?.scope,
  },
});

const launchUntouched = run(["docs/PLAN.md"], [launchGate]);

checks.push({
  claim:
    "a gate scoped by the documents it reads says NO-APLICA when none was touched",
  ok: stateOf(launchUntouched, launchGate) === "NO-APLICA",
  saw: {
    verdict: launchUntouched.verdict,
    gate: stateOf(launchUntouched, launchGate),
    why: launchUntouched.gates?.[0]?.why,
  },
});

/* The folder form of the same scoping, which is what a gate that reads a whole layer uses: a file inside a
 * listed folder is read as touched, so a change to one crate does not make the gate say it had nothing to say. */
const contractGate = "the contract, read from both sides";

const contractTouched = run(
  ["crates/mpcraft-vehicle/src/lib.rs"],
  [contractGate],
);

checks.push({
  claim:
    "a gate scoped with a folder runs for a file inside it, and names which file it read the change through",
  ok:
    stateOf(contractTouched, contractGate) === "PASS" &&
    (contractTouched.gates?.[0]?.scope ?? []).includes(
      "crates/mpcraft-vehicle/src/lib.rs",
    ),
  saw: {
    verdict: contractTouched.verdict,
    gate: stateOf(contractTouched, contractGate),
    scope: contractTouched.gates?.[0]?.scope,
  },
});

const contractUntouched = run(["docs/PLAN.md"], [contractGate]);

checks.push({
  claim:
    "the same gate says NO-APLICA when neither its documents nor its folder were touched",
  ok: stateOf(contractUntouched, contractGate) === "NO-APLICA",
  saw: {
    verdict: contractUntouched.verdict,
    gate: stateOf(contractUntouched, contractGate),
    why: contractUntouched.gates?.[0]?.why,
  },
});

const substituted = run(["docs/PLAN.md"], ["integration against SITL"]);
checks.push({
  claim: "what stands in for a gate is named, and is not read as a pass",
  ok:
    stateOf(substituted, "integration against SITL") === "SUSTITUIDA" &&
    substituted.gates?.[0]?.why !== undefined &&
    substituted.verdict === "PASS",
  saw: {
    verdict: substituted.verdict,
    gate: stateOf(substituted, "integration against SITL"),
    why: substituted.gates?.[0]?.why,
  },
});

const missingTool = run(["crates/mpcraft-link/lib.rs"], ["rust lints"], {
  GATE_PATH: "",
});

checks.push({
  claim:
    "a tool that is not there says HERRAMIENTA-AUSENTE, and that is a gap on a layer that was touched",
  ok:
    stateOf(missingTool, "rust lints") === "HERRAMIENTA-AUSENTE" &&
    missingTool.verdict === "FAIL",
  saw: {
    verdict: missingTool.verdict,
    gate: stateOf(missingTool, "rust lints"),
  },
});

/*
 * BLOQUEADO, shown with a genuine lock rather than with a string that looks like one. Node cannot choose the
 * share mode of a handle, so the artifact is held by a separate process that opens it with no sharing at all;
 * the control waits until its own attempt to open the same file answers with the sharing violation, which is
 * what proves the lock is real before anything is asserted. The file is prettier-clean on purpose, so the only
 * thing the gate can say about it is that it could not be read.
 */
const lockedFile = "apps/ui/src/lib/i18n/__control_lock__.ts";
const lockedPath = fileURLToPath(new URL(lockedFile, ROOT));

writeFileSync(
  lockedPath,
  'export const say = "a word that is clean until something else holds the file";\n',
  "utf8",
);

let blocked = null;
let after = null;
let held = null;
let releasedTheLock = false;
const holder = spawn(
  "powershell",
  [
    "-NoProfile",
    "-Command",
    `[IO.File]::Open('${lockedPath}', 'Append', 'Write', 'None'); Start-Sleep -Seconds 30`,
  ],
  { stdio: "ignore" },
);

try {
  /* The lock is only a fact once this process cannot open the file either. */
  for (let attempt = 0; attempt < 40 && held === null; attempt += 1) {
    try {
      closeSync(openSync(lockedPath, "r"));
    } catch (cause) {
      if (cause.code === "EBUSY") {
        held = cause.code;
        break;
      }
    }

    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
  }

  blocked = run([lockedFile], ["prettier"]);

  /* A file cannot be removed while something else holds it, so the handle is let go first and the control
     waits until its own open answers normally again — the same reading that proved the lock, from the other
     side. The gate runs on the file that is still there, because a file that is gone is not a file that was
     measured again. */
  holder.kill();

  for (let attempt = 0; attempt < 40 && !releasedTheLock; attempt += 1) {
    try {
      closeSync(openSync(lockedPath, "r"));
      releasedTheLock = true;
    } catch {
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
    }
  }

  after = run([lockedFile], ["prettier"]);
} finally {
  rmSync(lockedPath, { force: true });
}

checks.push({
  claim:
    "an artifact genuinely held by another process says BLOQUEADO, and that is a gap rather than a pass",
  ok:
    held === "EBUSY" &&
    stateOf(blocked, "prettier") === "BLOQUEADO" &&
    blocked.verdict === "FAIL",
  saw: {
    lock: held,
    verdict: blocked?.verdict,
    gate: stateOf(blocked, "prettier"),
    detail: blocked?.gates?.[0]?.detail,
  },
});

checks.push({
  claim: "the same gate measures the file again once nothing holds it",
  ok:
    releasedTheLock &&
    stateOf(after, "prettier") === "PASS" &&
    !existsSync(lockedPath),
  saw: {
    lock: releasedTheLock ? "let go" : "still held",
    verdict: after.verdict,
    gate: stateOf(after, "prettier"),
    leftBehind: existsSync(lockedPath) ? "still there" : "gone",
  },
});

/*
 * Which count a gate's green rests on, shown on transcripts that disagree. A live vehicle cannot be
 * driven from here, so the reading itself is what is exercised: a report where the unit tests passed and
 * the live areas failed must read as the live areas, because those are the ones a vehicle answered.
 */
const disagreeing =
  "     Running unittests src/lib.rs (target/debug/deps/mpcraft_link-1)\n" +
  "test result: ok. 108 passed; 0 failed; finished in 0.02s\n" +
  "     Running tests/sitl.rs (target/debug/deps/sitl-2)\n" +
  "test result: FAILED. 0 passed; 1 failed; finished in 25.15s\n";

const bothOk =
  "     Running unittests src/lib.rs (target/debug/deps/mpcraft_link-1)\n" +
  "test result: ok. 108 passed; 0 failed; finished in 0.02s\n" +
  "     Running tests/sitl.rs (target/debug/deps/sitl-2)\n" +
  "test result: ok. 1 passed; 0 failed; finished in 25.15s\n";

checks.push({
  claim:
    "the integration gate reads the report of the binary that the vehicle answered, not the first count the run prints",
  ok: resultForSection(disagreeing, "sitl").includes("FAILED"),
  saw: {
    read: resultForSection(disagreeing, "sitl"),
    /** What reading the first count would have said for the same output — the bug this control holds. */
    instead: lineMatching(disagreeing, /\d+ passed/),
  },
});

checks.push({
  claim: "the same reading names the live report when both halves are green",
  ok: resultForSection(bothOk, "sitl").includes("finished in 25.15s"),
  saw: { read: resultForSection(bothOk, "sitl") },
});

checks.push({
  claim:
    "a live area that never started is said as not running rather than counted as passing",
  ok:
    resultForSection(
      bothOk
        .split("\n")
        .filter((row) => !row.includes("tests/sitl.rs"))
        .join("\n"),
      "sitl",
    ) === "the sitl test binary did not run",
  saw: {
    read: resultForSection(
      bothOk
        .split("\n")
        .filter((row) => !row.includes("tests/sitl.rs"))
        .join("\n"),
      "sitl",
    ),
  },
});

/* What caught this reading: the heading as this machine's runner prints it, with the separator a path on
 * disk uses rather than the one the source tree does. A reading that accepts only one of the two calls a
 * run that ran a run that did not. */
const windowsHeading =
  "     Running unittests src/lib.rs (target/debug/deps/mpcraft_link-1)\n" +
  "test result: ok. 108 passed; 0 failed; finished in 0.02s\n" +
  "     Running tests\\sitl.rs (target/debug/deps/sitl-2)\n" +
  "test result: ok. 1 passed; 0 failed; finished in 25.15s\n";

checks.push({
  claim:
    "the heading is read with either separator, because the separator is not part of what the report says",
  ok: resultForSection(windowsHeading, "sitl").includes("finished in 25.15s"),
  saw: { read: resultForSection(windowsHeading, "sitl") },
});

/* The shape this machine's runner actually produces when both streams are joined: every report before
 * every heading, because cargo writes the headings on one stream and the reports on the other. A reading
 * that looks after the heading finds nothing here, so the pairing is by order rather than by position. */
const joinedStreams =
  "test result: ok. 108 passed; 0 failed; finished in 0.02s\n" +
  "test result: ok. 1 passed; 0 failed; finished in 25.15s\n" +
  "     Running unittests src/lib.rs (target/debug/deps/mpcraft_link-1)\n" +
  "     Running tests\\sitl.rs (target/debug/deps/sitl-2)\n";

const joinedRows = joinedStreams
  .split("\n")
  .map((row) => row.replaceAll("\\", "/"));

checks.push({
  claim:
    "the report is paired with its heading by order, not by position, because joining the streams puts the reports first",
  ok: resultForSection(joinedStreams, "sitl").includes("finished in 25.15s"),
  saw: {
    read: resultForSection(joinedStreams, "sitl"),
    /** What looking after the heading would have said for the same output — the shape this control caught. */
    instead:
      joinedRows
        .slice(joinedRows.findIndex((row) => row.includes("tests/sitl.rs")) + 1)
        .find((row) => /test result:/.test(row)) ?? "nothing after the heading",
  },
});

/* Two live areas answer this gate now, so each is named by its own report. One passing and one failing
 * cannot be read as a pass, and a count that covers only the first of them says nothing about the other. */
const twoAreas =
  "test result: ok. 108 passed; 0 failed; finished in 0.02s\n" +
  "test result: FAILED. 0 passed; 1 failed; finished in 16.22s\n" +
  "test result: ok. 1 passed; 0 failed; finished in 45.10s\n" +
  "     Running unittests src/lib.rs (target/debug/deps/mpcraft_link-1)\n" +
  "     Running tests\\sitl.rs (target/debug/deps/sitl-2)\n" +
  "     Running tests\\streams_live.rs (target/debug/deps/streams_live-3)\n";

checks.push({
  claim:
    "each live area is named by its own report, so one that failed and one that passed is not read as a pass",
  ok:
    resultForSection(twoAreas, "sitl").includes("FAILED") &&
    resultForSection(twoAreas, "streams_live").includes("45.10s"),
  saw: {
    link: resultForSection(twoAreas, "sitl"),
    app: resultForSection(twoAreas, "streams_live"),
  },
});

/* The second live area cannot share the first one's vehicle: an instance serves one connection and stops when
 * that connection goes away. So when no second peer is named, the scripted board stands in and says so. */
const tableGate = run(
  ["crates/mpcraft-app/tests/streams_live.rs"],
  ["the table read against a live board"],
);

checks.push({
  claim:
    "the second live area names what stands in for it when no second peer is named, rather than reading the first area as if it covered both",
  ok:
    stateOf(tableGate, "the table read against a live board") === "SUSTITUIDA",
  saw: {
    gate: stateOf(tableGate, "the table read against a live board"),
    why: tableGate.gates?.[0]?.why,
  },
});

/* The board gate names the peer that answered it. With no port named, the scripted board stands in, and the
 * simulator's green is not read as covering hardware: a script has no model to name and no address of its own. */
const boardGate = run(
  ["crates/mpcraft-app/tests/hardware_live.rs"],
  ["the board read as a vehicle"],
  { MPCRAFT_HARDWARE_TARGET: "" },
);

checks.push({
  claim:
    "the board gate names what stands in for it when no port is named, rather than reading the simulator's green as covering hardware",
  ok: stateOf(boardGate, "the board read as a vehicle") === "SUSTITUIDA",
  saw: {
    gate: stateOf(boardGate, "the board read as a vehicle"),
    why: boardGate.gates?.[0]?.why,
  },
});

/* A transcript where the area that speaks to the board failed and another binary passed. The reports come
 * before the headings because cargo prints them on different streams, which is what the pairing reads. */
const boardTranscript =
  "test result: ok. 3 passed; 0 failed; finished in 0.01s\n" +
  "test result: FAILED. 0 passed; 1 failed; finished in 33.52s\n" +
  "     Running tests\\board_boot_text.rs (target/debug/deps/board_boot_text-1)\n" +
  "     Running tests\\hardware_live.rs (target/debug/deps/hardware_live-2)\n";

checks.push({
  claim:
    "the board gate reads the report of its own binary, so a golden test passing does not stand for an area that failed against the board",
  ok:
    resultForSection(boardTranscript, "hardware_live").includes("FAILED") &&
    resultForSection(boardTranscript, "board_boot_text").includes("3 passed"),
  saw: {
    hardware: resultForSection(boardTranscript, "hardware_live"),
    golden: resultForSection(boardTranscript, "board_boot_text"),
  },
});

/* The live half of the plugin surface is fed by a vehicle that decides its own numbers. With no instance of
 * its own named, the scripted peer stands in — it answers the shapes but was written to carry the numbers, so
 * it cannot show a vehicle deciding them. */
const pluginGate = run(
  ["crates/mpcraft-app/tests/plugins_live.rs"],
  ["the surface read against a live vehicle"],
  { MPCRAFT_SITL_LIVE: "", MPCRAFT_SITL_PLUGINS_TARGET: "" },
);

checks.push({
  claim:
    "the plugin area names what stands in for it when no instance of its own is named, rather than reading the scripted peer as if a vehicle had answered",
  ok:
    stateOf(pluginGate, "the surface read against a live vehicle") ===
    "SUSTITUIDA",
  saw: {
    gate: stateOf(pluginGate, "the surface read against a live vehicle"),
    why: pluginGate.gates?.[0]?.why,
  },
});

/* A transcript where the scripted half passed and the live half failed. The two are one crate and one run, so
 * a count taken from the whole run would read the pass as covering both. */
const pluginTranscript =
  "test result: ok. 1 passed; 0 failed; finished in 0.02s\n" +
  "test result: FAILED. 0 passed; 1 failed; finished in 31.40s\n" +
  "     Running tests\\plugins_wire.rs (target/debug/deps/plugins_wire-1)\n" +
  "     Running tests\\plugins_live.rs (target/debug/deps/plugins_live-2)\n";

checks.push({
  claim:
    "the plugin area reads the report of its own binary, so a scripted peer passing does not stand for an area no vehicle answered",
  ok:
    resultForSection(pluginTranscript, "plugins_live").includes("FAILED") &&
    resultForSection(pluginTranscript, "plugins_wire").includes("1 passed"),
  saw: {
    live: resultForSection(pluginTranscript, "plugins_live"),
    scripted: resultForSection(pluginTranscript, "plugins_wire"),
  },
});

/*
 * The runner names its own reach inside what it reports. A limit that is not there, and a limit that names only
 * how the states are read without saying what the run cannot see, are both findings; each is put in front of the
 * reading as a copy of the runner, so the runner as it stands is never disturbed.
 */
function readsItsOwnLimit(source) {
  const said = source.match(/const LIMIT =\s*\n?\s*"([^"]*)"/);

  if (said === null) {
    return "names no limit";
  }

  return /does not see/.test(said[1])
    ? null
    : `names a limit that does not say what the run cannot see: ${said[1]}`;
}

const runnerSource = readFileSync(runner, "utf8");
const runnerSentence = runnerSource.match(/const LIMIT =\s*\n?\s*"[^"]*"/)?.[0];

checks.push({
  claim:
    "the runner names what the whole run cannot see inside what it reports",
  ok: readsItsOwnLimit(runnerSource) === null && runnerSentence !== undefined,
  saw: { limit: readsItsOwnLimit(runnerSource) },
});

checks.push({
  claim: "the reading fires on a runner that names no limit at all",
  ok:
    readsItsOwnLimit(
      runnerSource.replace(runnerSentence, "const LIMIT = null;"),
    ) === "names no limit",
  saw: readsItsOwnLimit(
    runnerSource.replace(runnerSentence, "const LIMIT = null;"),
  ),
});

checks.push({
  claim:
    "and on a limit that names how the states are read but not what the run cannot see",
  ok:
    readsItsOwnLimit(
      runnerSource.replace(
        runnerSentence,
        'const LIMIT =\n  "NO-APLICA is honest but is not evidence.";',
      ),
    ) ===
    "names a limit that does not say what the run cannot see: NO-APLICA is honest but is not evidence.",
  saw: readsItsOwnLimit(
    runnerSource.replace(
      runnerSentence,
      'const LIMIT =\n  "NO-APLICA is honest but is not evidence.";',
    ),
  ),
});

const fails = checks.filter((check) => !check.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove the states are reachable and the exit code follows them. They do not prove any gate is strong. The busy artifact is a real sharing violation held by a separate process, because Node cannot choose the share mode of a handle, so what it exercises is the rule that reads a busy artifact as a gap rather than as green — not a file locked by the running application, which is not running here. The last three checks read the runner as a document rather than run it, so they show the rule firing on copies of the runner and not on what a run measures.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
