#!/usr/bin/env node
/**
 * The controls of the DFU number gate: each thing it claims to catch is put in front of it, and the two files
 * are shown passing untouched. Both directions, or neither.
 *
 * What these prove, and what they do not: they prove each rule can fire — a literal that disagrees with the
 * table built from the polynomial, a pair written as agreeing, a writer whose shape numbers disagree with the
 * format, a file that names another string than the class this program reads through — and that the files as they
 * stand satisfy them. They do not prove that the Rust implementation's own rule uses the same table as this one:
 * that comparison is what the module's test carries, and nothing here executes the module. The known-bad cases are
 * copies held in memory, so the files on the disk are never disturbed, and nothing here shows that an elevated
 * install writes the value the operator puts or that a row answers after it is written.
 */

import { readFileSync } from "node:fs";

import { check } from "./check-dfu-crc.js";

const testsPath = new URL(
  "../crates/mpcraft-app/tests/dfu.rs",
  import.meta.url,
);
const sourcePath = new URL("../crates/mpcraft-app/src/dfu.rs", import.meta.url);
const infPath = new URL("./dfu-winusb.inf", import.meta.url);
const devicePath = new URL(
  "../crates/mpcraft-winusb/src/lib.rs",
  import.meta.url,
);

const tests = readFileSync(testsPath, "utf8");
const source = readFileSync(sourcePath, "utf8");
const inf = readFileSync(infPath, "utf8");
const device = readFileSync(devicePath, "utf8");

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
const good = check({ tests, source, inf, device });

checks.push({
  claim: "the two files agree with a table written from the polynomial",
  ok: good.faults.length === 0 && good.samples.length === 5,
  saw: { fixture: good.fixture, shapes: good.shapes, faults: good.faults },
});

/* The file the operator runs, read as text: the value decides under which name a row answers, and this binding
   reads through the class the element reports, so the file has to name the same string as the crate carries. The
   sample guid stays a separate fact rather than becoming the name the file writes, and the pair stays the pair the
   published filter takes rather than the rule this program reads. */
checks.push({
  claim:
    "the file names the same string as the class this program reads through",
  ok:
    good.faults.length === 0 &&
    good.names.infValue === good.names.sourceClass &&
    good.names.infClassGuid === good.names.sourceClass &&
    good.names.sampleGuid !== good.names.sourceClass,
  saw: good.names,
});

checks.push({
  claim: "the pair the crate carries is the pair the published filter takes",
  ok:
    good.faults.length === 0 &&
    good.names.pair[0] === good.names.publishedPair[0] &&
    good.names.pair[1] === good.names.publishedPair[1],
  saw: { pair: good.names.pair, publishedPair: good.names.publishedPair },
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
  {
    side: "inf",
    claim:
      "a value naming the guid WinUSB samples is caught, because that pair was read as if the two names were one fact",
    /* This is the state this change found: the file wrote the sample guid, which agrees with the second reading
       and not with the name this program reads through, so the row would be there and the program would refuse. */
    mutated: mutate(
      inf,
      /^HKR,,DeviceInterfaceGUIDs,0x10000,/,
      'HKR,,DeviceInterfaceGUIDs,0x10000,"{a5dcbf13-4437-40bb-9a12-8e18f591d448}"',
    ),
    expect:
      /the file writes "a5dcbf13-4437-40bb-9a12-8e18f591d448" on the device key/,
    allowExtra: /never matched/,
  },
  {
    side: "inf",
    claim: "a class declaration naming another string is caught",
    mutated: mutate(
      inf,
      /^ClassGuid=\{/,
      "ClassGuid={36109140-8b13-11cf-bfc1-08002be10318}",
    ),
    expect: /the class the file declares/,
    allowExtra: /never matched/,
  },
  {
    side: "inf",
    claim:
      "a value written as one word instead of as a list of names is caught",
    mutated: mutate(
      inf,
      /^HKR,,DeviceInterfaceGUIDs,0x10000,/,
      'HKR,,DeviceInterfaceGUIDs,0x10001,"{88bae032-5a81-49f0-bc3d-a4ff138216d6}"',
    ),
    /* The two readings differ on this shape: one stops at the terminator and answers with one word, and the other
       splits at the terminators and answers with entries, so a copy that writes the value as `REG_SZ` says the
       reading would get a list of names, which it would not. */
    expect: /the file writes the value with "0x10001"/,
    allowExtra: /never matched/,
  },
  {
    side: "device",
    claim:
      "the two spellings of the value type written as the same number are caught",
    /* An AddReg line writes 0x10000 for a list of names and the call that reads the value answered 7 for this
       machine's instance. Writing them as one number closes the difference between what a file says and what a
       reading gets. */
    mutated: mutate(
      device,
      /pub const REGISTRY_TYPE_MULTI_SZ: &str = "0x7";/,
      'pub const REGISTRY_TYPE_MULTI_SZ: &str = "0x10000";',
    ),
    expect: /written as the same number/,
    allowExtra: /the published table of value types gives/,
  },
  {
    side: "device",
    claim:
      "a reading that walks entries by another marker than the terminators is caught",
    /* Today's machine showed why the two readings are two rules: the value it holds is one entry with a comma
       inside, so a reader that stops at the first terminator sees one word and the rule that reads names refuses it,
       while a reader that splits at terminators would give one entry per name. A copy that splits by something else
       says the pair is the same fact seen through a different mark. */
    mutated: mutate(
      device,
      /\.split\(\|unit\| \*unit == 0\)/,
      ".split(|unit| *unit == 1)",
    ),
    expect: /does not split at the terminators/,
    allowExtra: /never matched/,
  },
  {
    side: "device",
    claim:
      "a word reading that does not stop at its first terminator is caught",
    mutated: mutate(
      device,
      /text\.split\('\\0'\)\.next\(\)/,
      "text.split('_').first()",
    ),
    expect: /answers with a word but does not stop at the first terminator/,
    allowExtra: /never matched/,
  },
  {
    side: "device",
    claim:
      "the brace pair read as decoration is caught, because a name that opens it without closing it is not a name either reading gives",
    /* This is the state the corrected write left on the key: an opening brace and no closing one. `StringFromCLSID`
       writes names with the pair and these APIs print them without it, so the pair is one fact with two shapes; a copy
       that trims the opening brace on its own turns an incomplete text into a class. */
    mutated: mutate(device, /if opens != closes \{/, "if opens {"),
    expect: /read as decoration rather than as its own fact/,
    allowExtra: /never matched/,
  },
  {
    side: "device",
    claim: "a pair written as if it decided both rules is caught",
    mutated: mutate(
      device,
      /^pub const DFU_CLASS: u8 = 0xFE;/,
      "pub const DFU_CLASS: u8 = 0xFB;",
    ),
    /* pydfu takes 0xFE and 1 from the interface settings; a literal that names another number says the filter and
       this binding read the same fact. */
    expect: /the pair the crate carries is \[251,1\]/,
    allowExtra: /never matched/,
  },
  {
    side: "inf",
    claim: "a file that declares a class and writes nothing is caught",
    mutated: mutate(
      inf,
      /^HKR,,DeviceInterfaceGUIDs,0x10000,/,
      "; nothing written, which is what this machine answers today",
    ),
    expect: /writes no value on the device key/,
    allowExtra: /never matched/,
  },
  {
    side: "inf",
    claim:
      "a value whose pair is opened and not closed is caught, because that text is not a name on either reading",
    /* The gate reads the name as the line writes it rather than through a reader that strips the pair, so a line that
       loses the closing brace says a class where the file wrote an incomplete string — which is what happened on this
       machine when the command was pasted without it. */
    mutated: mutate(
      inf,
      /^HKR,,DeviceInterfaceGUIDs,0x10000,"\{[^"]*\}"$/,
      'HKR,,DeviceInterfaceGUIDs,0x10000,"{88bae032-5a81-49f0-bc3d-a4ff138216d6"',
    ),
    expect: /the pair on one side only/,
    allowExtra: /never matched/,
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
    inf: entry.side === "inf" ? entry.mutated : inf,
    device: entry.side === "device" ? entry.mutated : device,
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

  if (!/not by the module that carries them/.test(said[1])) {
    return `names a limit that does not say what it cannot reach: ${said[1]}`;
  }

  if (!/does not say which reading a board answers with/.test(said[1])) {
    return `names a limit that does not say what it cannot reach: ${said[1]}`;
  }

  /* The file is a third thing this reading touches, and it can only be read as text: nothing here shows that an
     elevated install writes the value, or that a row answers after it is written. A limit that names the numbers
     and the writer but not that is a claim about more than the reading buys. */
  return /read as text too/.test(said[1])
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

/* The file the operator runs is a third thing this reading touches, and it can only be read as text. A copy whose
   limit names the numbers and the writer but not that is put in front of the rule, and the gate as it stands passes. */
const omittedBody =
  "the numbers are worked out by a second implementation of the format written here, not by the module that carries them, so nothing of the module is executed. The writer is read as text, and the disagreement is said as two numbers for the same bytes, which does not say which reading a board answers with.";

checks.push({
  claim:
    "and on a limit that names the numbers and the writer but not the file it reads as text",
  ok:
    readsItsOwnLimit(
      gate.replace(sentence, `const LIMIT =\n  "${omittedBody}";`),
    ) ===
    `names a limit that does not say what it cannot reach: ${omittedBody}`,
  saw: readsItsOwnLimit(
    gate.replace(sentence, `const LIMIT =\n  "${omittedBody}";`),
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
