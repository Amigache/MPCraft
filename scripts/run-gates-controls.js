#!/usr/bin/env node
/**
 * The controls of the gate runner: the four things it claims, each shown doing what it claims.
 *
 *     it can fail          a word through the catalogue with no entry, and a known-bad scratch
 *                          file that writes that word, is put through it
 *     it can pass          the same run without the scratch file
 *     it names what did not run, and does not read it as green   a layer that was not touched,
 *                          and a tool that is not there
 *
 * What these prove, and what they do not: they prove the states are reachable and that the exit
 * code follows them. They do not prove that a gate that runs is strong — a gate whose green rests
 * on a count it cannot fail on is outside of what these two cases buy, and BLOQUEADO is not
 * exercised here because it needs an artifact genuinely locked by the running application.
 */

import { execFileSync } from "node:child_process";
import { existsSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

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

// What did not run, in its two kinds.
const notApplicable = run(["docs/PLAN.md"], ["frontend tests"]);

checks.push({
  claim: "a layer that was not touched says NO-APLICA rather than green",
  ok: stateOf(notApplicable, "frontend tests") === "NO-APLICA",
  saw: {
    verdict: notApplicable.verdict,
    gate: stateOf(notApplicable, "frontend tests"),
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

const fails = checks.filter((check) => !check.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove the states are reachable and the exit code follows them. They do not prove any gate is strong, and BLOQUEADO is not exercised.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
