#!/usr/bin/env node
/**
 * The controls of the DFU number gate: each thing it claims to catch is put in front of it, and the two files
 * are shown passing untouched. Both directions, or neither.
 *
 * What these prove, and what they do not: they prove each rule can fire — a literal that disagrees with the
 * table built from the polynomial, a pair written as agreeing, a writer whose shape numbers disagree with the
 * format — and that the files as they stand satisfy them. They do not prove that the Rust implementation's own
 * rule uses the same table as this one: that comparison is what the module's test carries, and nothing here
 * executes the module. The known-bad cases are copies held in memory, so the files on the disk are never
 * disturbed.
 */

import { readFileSync } from "node:fs";

import { check } from "./check-dfu-crc.js";

const testsPath = new URL(
  "../crates/mpcraft-app/tests/dfu.rs",
  import.meta.url,
);
const sourcePath = new URL("../crates/mpcraft-app/src/dfu.rs", import.meta.url);

const tests = readFileSync(testsPath, "utf8");
const source = readFileSync(sourcePath, "utf8");

/**
 * Replaces one line with another, and says so when the line to change is not there. The indentation the file
 * writes is kept, because these lines sit inside a module rather than at the top of the file.
 */
function mutate(text, pattern, replacement) {
  const line = text.split(/\r?\n/).find((row) => pattern.test(row));

  if (line === undefined) {
    return { error: `nothing in the file looks like ${pattern}` };
  }

  const indent = line.match(/^(\s*)/)?.[1] ?? "";

  return text.replace(line, `${indent}${replacement}`);
}

const checks = [];

/* The known-good case, before anything is disturbed. */
const good = check({ tests, source });

checks.push({
  claim: "the two files agree with a table written from the polynomial",
  ok: good.faults.length === 0 && good.samples.length === 5,
  saw: { fixture: good.fixture, shapes: good.shapes, faults: good.faults },
});

const cases = [
  {
    side: "tests",
    claim: "a golden literal that disagrees with its own table is caught",
    mutated: mutate(
      tests,
      /const FIXTURE_CRC: u32 = 0x[0-9a-f_]+;/,
      "const FIXTURE_CRC: u32 = 0x8cdc_967b;",
    ),
    expect: /the fixture's number is 8cdc967b/,
    allowExtra:
      /read with the wording of the specification|writes the two readings as agreeing/,
  },
  {
    side: "tests",
    claim: "a sample whose number comes from the other reading is caught",
    mutated: mutate(
      tests,
      /\(b"abc", 0x[0-9a-f_]+, 0x[0-9a-f_]+\)/,
      '(b"abc", 0x9e62_63ff, 0xcadb_be3d)',
    ),
    expect: /the sample "abc" says 9e6263ff/,
    allowExtra:
      /read with the wording of the specification|writes the two readings as agreeing/,
  },
  {
    side: "tests",
    claim:
      "a pair written as agreeing closes the disagreement, and is a finding",
    mutated: mutate(
      tests,
      /\(b"golden", 0x[0-9a-f_]+, 0x[0-9a-f_]+\)/,
      '(b"golden", 0xcc00_79d8, 0xcc00_79d8)',
    ),
    expect: /writes the two readings as agreeing/,
    allowExtra: /read with the wording of the specification/,
  },
  {
    side: "source",
    claim:
      "a writer that counts the target size field without its prefix is caught",
    mutated: mutate(
      source,
      /bytes\.extend_from_slice\(&\(\(274 \+ data_len \+ 11\)/,
      "bytes.extend_from_slice(&(data_len + 11) as u32).to_le_bytes());",
    ),
    expect: /says 0 for the targetPrefix/,
    /* The same line carries both numbers the format gives for this shape, so the second one is said as well:
       a size field written without its prefix is one mutation rather than two findings. */
    allowExtra: /the format gives/,
  },
  {
    side: "source",
    claim: "a suffix that does not carry the marker the format gives is caught",
    mutated: mutate(
      source,
      /bytes\.extend_from_slice\(b"UFD"\);/,
      'bytes.extend_from_slice(b"DFU");',
    ),
    expect: /says "DFU" for the suffixMarker/,
    allowExtra: /the format gives/,
  },
];

for (const entry of cases) {
  if (entry.mutated.error !== undefined) {
    checks.push({ claim: entry.claim, ok: false, saw: entry.mutated.error });
    continue;
  }

  const changed = {
    tests: entry.side === "tests" ? entry.mutated : tests,
    source: entry.side === "source" ? entry.mutated : source,
  };
  const result = check(changed);
  const fault = result.faults.find((line) => entry.expect.test(line));

  /* The mutation has to be caught by the rule it is meant to test. A second fault is allowed only when it
     belongs to the same reading — the same bytes seen from the other table, or another shape on the same line —
     and anything else means the reading is reporting something it was not asked about. */
  const extra = result.faults.filter((line) => line !== fault);

  checks.push({
    claim: entry.claim,
    ok:
      fault !== undefined &&
      extra.every((line) => entry.allowExtra.test(line)) &&
      extra.length <= 1,
    saw: result.faults,
  });
}

/*
 * The gate names what it cannot reach inside what it reports, which is the same rule the other readings carry:
 * a gate whose green is read as more than it buys is one that says nothing about its reach. A limit that is not
 * there, and a limit that names only what the numbers are recomputed from, are both findings, and each is put in
 * front of the reading as a copy of the gate so the gate as it stands is never disturbed.
 */
function readsItsOwnLimit(text) {
  const said = text.match(/const LIMIT =\s*\n?\s*"([^"]*)"/);

  if (said === null) {
    return "names no limit";
  }

  return /not by the module that carries them/.test(said[1]) &&
    /does not say which reading a board answers with/.test(said[1])
    ? null
    : `names a limit that does not say what it cannot reach: ${said[1]}`;
}

const gate = readFileSync(
  new URL("./check-dfu-crc.js", import.meta.url),
  "utf8",
);
const sentence = gate.match(/const LIMIT =\s*\n?\s*"[^"]*"/)?.[0];

checks.push({
  claim: "the gate names what it cannot reach in what it reports",
  ok: readsItsOwnLimit(gate) === null && sentence !== undefined,
  saw: { limit: readsItsOwnLimit(gate) },
});

checks.push({
  claim: "the reading fires on a gate that names no limit at all",
  ok:
    readsItsOwnLimit(gate.replace(sentence, "const LIMIT = null;")) ===
    "names no limit",
  saw: readsItsOwnLimit(gate.replace(sentence, "const LIMIT = null;")),
});

checks.push({
  claim: "and on a limit that names only what the numbers are recomputed from",
  ok:
    readsItsOwnLimit(
      gate.replace(
        sentence,
        'const LIMIT =\n  "the numbers are worked out by a second implementation written here.";',
      ),
    ) ===
    "names a limit that does not say what it cannot reach: the numbers are worked out by a second implementation written here.",
  saw: readsItsOwnLimit(
    gate.replace(
      sentence,
      'const LIMIT =\n  "the numbers are worked out by a second implementation written here.";',
    ),
  ),
});

const fails = checks.filter((entry) => !entry.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove each rule can fire and that the two files satisfy them, and that the gate names its own reach in what it reports. They do not show the Rust implementation using the same table as this reading — that comparison is the module's own test, and nothing here executes it. The known-bad cases are copies held in memory, so the files on the disk are never disturbed, and the reading of the writer is a reading of text: a shape it does not name is not seen.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
