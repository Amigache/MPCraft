#!/usr/bin/env node
/**
 * The controls of the parity check: each thing it claims to catch is put in front of it, and the
 * unmutated matrix is shown passing. Both directions, or neither.
 *
 * What these prove, and what they do not: they prove each rule can fire and that the matrix as it
 * stands satisfies them. They do not prove a row is right about what upstream does — the check
 * reads the table, not the code, so a row that claims a feature is done when it is not is outside
 * of what these controls buy.
 */

import { readFileSync } from "node:fs";

import { check } from "./check-parity.js";

const text = readFileSync(
  new URL("../docs/PARITY.md", import.meta.url),
  "utf8",
);

/**
 * Replaces one line with another, and says so when the line to change is not there.
 *
 * The lines are read as the file writes them: git hands this machine text files with the carriage return
 * before the line feed on checkout (`core.autocrlf=true`), so a rule anchored at the end of a line would
 * find nothing in a file written that way.
 */
function mutate(source, pattern, replacement) {
  const line = source.split(/\r?\n/).find((row) => pattern.test(row));

  if (line === undefined) {
    return { error: `nothing in the matrix looks like ${pattern}` };
  }

  return source.replace(line, replacement);
}

const checks = [];

// The known-good case, before anything is disturbed.
const good = check(text);

checks.push({
  claim: "the matrix as it stands agrees with its own summary",
  ok: good.faults.length === 0 && good.rows > 100,
  saw: { rows: good.rows, counted: good.counted, faults: good.faults },
});

const cases = [
  {
    claim: "a summary that disagrees with the rows is caught",
    mutated: mutate(text, /^\| ✅ Done \| \d+ \|$/, "| ✅ Done | 142 |"),
    expect: /summary says 142 for Done/,
  },
  {
    claim: "a status that is not in the legend is caught",
    mutated: mutate(
      text,
      /^\| 1\.1 \|.*\|$/,
      "| 1.1 | Serial port connection | `ExtLibs/Comms/CommsSerialPort.cs` | 🚧 | 0 |",
    ),
    expect: /not in the legend/,
  },
  {
    claim: "a planned row without a phase is caught",
    mutated: mutate(
      text,
      /^\| 1\.18 \|.*🔜.*\|$/,
      "| 1.18 | Bluetooth LE transport | `ExtLibs/Comms/CommsBLE.cs` | 🔜 |  |",
    ),
    expect: /planned without a phase/,
  },
  {
    claim: "a row that names no place in upstream is caught",
    mutated: mutate(
      text,
      /^\| 3\.1 \|.*\|$/,
      "| 3.1 | Arm / disarm |  | ✅ | 1 |",
    ),
    expect: /names no place in upstream/,
  },
  {
    claim: "a dropped row that does not say why is caught",
    mutated: mutate(
      text,
      /^\| 6\.14 \|.*⬜.*\|$/,
      "| 6.14 | Python LogAnalyzer integration | `LogAnalyzer/` | ⬜ | 7 |",
    ),
    expect: /dropped without saying why/,
  },
];

for (const entry of cases) {
  if (entry.mutated.error !== undefined) {
    checks.push({ claim: entry.claim, ok: false, saw: entry.mutated.error });
    continue;
  }

  const result = check(entry.mutated);
  const fault = result.faults.find((line) => entry.expect.test(line));

  /* The mutation has to be caught by the rule it is meant to test. A second fault is allowed only
     when it is the summary disagreeing with the rows, which is what changing a row's status does to
     a hand-written count; anything else means the check is reporting something it was not asked
     about. */
  const extra = result.faults.filter((line) => line !== fault);

  checks.push({
    claim: entry.claim,
    ok:
      fault !== undefined &&
      extra.every((line) => /summary says/.test(line)) &&
      extra.length <= 1,
    saw: result.faults,
  });
}

/* The same rule, against a copy of the matrix whose lines end with the carriage return before the line feed,
   which is what git hands this machine on checkout and what made these controls find nothing before the lines
   were read that way. */
const crlf = text.replace(/\r?\n/g, "\r\n");
const knownBad = crlf
  .split(/\r?\n/)
  .find((row) => /^\| 6\.14 \|.*⬜.*\|$/.test(row));
const crlfResult = check(
  crlf.replace(
    knownBad,
    "| 6.14 | Python LogAnalyzer integration | `LogAnalyzer/` | ⬜ | 7 |",
  ),
);

checks.push({
  claim:
    "the rules fire on a file that ends its lines with the pair, not only on one that ends them with a single byte",
  ok: crlfResult.faults.some((line) => /dropped without saying why/.test(line)),
  saw: crlfResult.faults,
});

const fails = checks.filter((entry) => !entry.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove each rule can fire and that the matrix satisfies them. They do not prove a row is right about what upstream does.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
