#!/usr/bin/env node
/**
 * The controls of the frontend lint gate: what it claims about itself, each shown doing what it claims.
 *
 *     it can fail              a scratch file with a name that is written and never read, one with a
 *                              dependency list that does not match what the effect reads, and one that
 *                              switches off a rule the configuration does not register
 *     it can pass              the same run with nothing left behind
 *     what it does not read is named                   the vendored MapLibre worker is outside the gate,
 *                              and the tool run on that file alone finds more than a reader could act on,
 *                              so its exclusion is a fact about an artifact rather than a clean file
 *
 * What these prove, and what they do not: they prove the gate fails in both directions and that the exit
 * code follows them. They do not prove the gate reads the React JSX rules — the plugin for those cannot
 * run on the version of eslint this project uses — nor the minified artifact it names, nor formatting,
 * which is prettier's job and a different gate.
 */

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const ROOT = new URL("../", import.meta.url);
const UI = new URL("./apps/ui/", ROOT);
const runner = fileURLToPath(new URL("./run-gates.js", import.meta.url));
const config = fileURLToPath(new URL("./apps/ui/eslint.config.js", ROOT));
const worker = fileURLToPath(
  new URL("./apps/ui/public/maplibre-gl-csp-worker.js", ROOT),
);
const sources = fileURLToPath(new URL("./apps/ui/src", ROOT));

const scratch = [
  fileURLToPath(new URL("./apps/ui/src/__control_lint__.ts", ROOT)),
  fileURLToPath(new URL("./apps/ui/src/__control_lint_hook__.tsx", ROOT)),
  fileURLToPath(new URL("./apps/ui/src/__control_lint_rule__.ts", ROOT)),
];

/** One run of the runner, with the change it is told to read and the gate it is asked about. */
function run(change) {
  try {
    return JSON.parse(
      execFileSync(
        "node",
        [runner, "--change", ...change, "--gates", "frontend lint"],
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

const stateOf = (report) =>
  (report.gates ?? []).find((entry) => entry.gate === "frontend lint")?.state;

/** The tool, run on files of its own so an exclusion can be seen as an exclusion. */
function lint(paths, options = []) {
  try {
    return execFileSync(
      "node",
      ["node_modules/eslint/bin/eslint.js", ...options, ...paths],
      {
        encoding: "utf8",
        cwd: fileURLToPath(UI),
      },
    );
  } catch (cause) {
    /** A finding is a non-zero exit, and the report is on the output stream before it. */
    return String(cause.stdout ?? "") || String(cause.message);
  }
}

const checks = [];

/* The known-bad cases. Each is put through the runner and taken back before the next one, so the run at
 * the end is the same run that was green before them. */
const badCases = [
  {
    claim: "the gate fails on a name that is written and never read",
    path: scratch[0],
    change: "apps/ui/src/__control_lint__.ts",
    body: "export const read = () => 0;\n\nconst written = 1;\n",
  },
  {
    claim:
      "the gate fails on a dependency list that does not match what the effect reads",
    path: scratch[1],
    change: "apps/ui/src/__control_lint_hook__.tsx",
    body: 'import { useCallback } from "react";\n\nexport const useThing = (value: number) => useCallback(() => value, []);\n',
  },
  {
    claim: "a rule the gate does not carry cannot be switched off by a comment",
    path: scratch[2],
    change: "apps/ui/src/__control_lint_rule__.ts",
    body: "// eslint-disable-next-line react/no-array-index-key\nexport const row = (index: number) => index;\n",
  },
];

for (const bad of badCases) {
  writeFileSync(bad.path, bad.body, "utf8");

  const report = run([bad.change]);

  checks.push({
    claim: bad.claim,
    ok: report.verdict === "FAIL" && stateOf(report) === "FAIL",
    saw: {
      verdict: report.verdict,
      gate: stateOf(report),
      /** The tool run on this file alone, which is what the gate's failure rests on. */
      finding: lint([bad.change.replace("apps/ui/", "")])
        .trim()
        .split("\n")[1],
    },
  });

  rmSync(bad.path, { force: true });
}

const good = run(["apps/ui/eslint.config.js"]);

checks.push({
  claim: "the same run is green when nothing is left behind",
  ok: good.verdict === "PASS" && stateOf(good) === "PASS",
  saw: {
    verdict: good.verdict,
    gate: stateOf(good),
    proof: good.gates?.[0]?.proof,
  },
});

/* What the gate leaves out, shown as something it really does not read: the configuration names the file,
 * the tool says it is ignored, and with the ignores switched off the same tool finds more on that one file
 * than a reader could act on. A green that rested on an artifact being clean would say something the
 * artifact is not. */
const configText = readFileSync(config, "utf8");
const workerText = readFileSync(worker, "utf8");
const readByGate = lint(["public/maplibre-gl-csp-worker.js"]);
const readWithIgnoresOff = lint(
  ["public/maplibre-gl-csp-worker.js"],
  ["--no-ignore"],
);
const findingsWhenRead = readWithIgnoresOff.match(/(\d+) problems/)?.[1];

checks.push({
  claim:
    "the file the gate excludes is named in its configuration, and is an artifact rather than source",
  ok:
    configText.includes("public/maplibre-gl-csp-worker.js") &&
    workerText.split("\n").length <= 8 &&
    Number(findingsWhenRead ?? 0) > 500,
  saw: {
    lines: workerText.split("\n").length,
    findingsWhenRead,
  },
});

checks.push({
  claim:
    "the exclusion is doing work: the tool says the file is ignored, and reads it when the ignores are switched off",
  ok:
    readByGate.includes("matching ignore pattern") &&
    Number(findingsWhenRead ?? 0) > 500,
  saw: {
    asTheGateReadsIt: readByGate.trim().split("\n")[0],
    findingsWhenRead,
  },
});

/* A comment that switches off a rule the configuration does not register is a finding, which is what the
 * sources were caught carrying. So every rule a source file switches off must be one this gate carries. */
function walk(dir) {
  const files = [];

  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = `${dir}/${entry.name}`;

    if (entry.isDirectory()) {
      files.push(...walk(path));
    } else if (/\.(ts|tsx)$/.test(entry.name)) {
      files.push(path);
    }
  }

  return files;
}

/** The plugin namespaces the configuration registers; a rule with no namespace is a core rule. */
const carried = new Set(
  [...configText.matchAll(/plugins: \{([^}]*)\}/g)].flatMap((match) =>
    [...match[1].matchAll(/"([^"]+)":/g)].map((name) => name[1]),
  ),
);

const directives = walk(sources).flatMap((path) =>
  [
    ...readFileSync(path, "utf8").matchAll(
      /eslint-disable-next-line\s+([\w/-]+)/g,
    ),
  ].map((match) => ({
    file: path
      .replaceAll("\\", "/")
      .replace(String(sources).replaceAll("\\", "/"), "apps/ui/src"),
    rule: match[1],
  })),
);

checks.push({
  claim:
    "the sources carry disable comments, and every rule they name is one this gate carries",
  ok:
    directives.length > 0 &&
    directives.every(
      (d) => !d.rule.includes("/") || carried.has(d.rule.split("/")[0]),
    ),
  saw: {
    directives,
    /** What a directive naming a namespace outside this set would have said: the tool reports the rule as not defined. */
    caughtBy: "Definition for rule 'react/no-array-index-key' was not found",
    carried: [...carried],
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
        "these prove the gate fails in both directions and that the exit code follows them. They do not prove that the gate reads the React JSX rules, which the plugin cannot run on this version of eslint, nor the minified artifact it names, nor formatting, which is prettier's job.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
