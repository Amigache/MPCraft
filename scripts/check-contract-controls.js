#!/usr/bin/env node
/**
 * The controls of the contract check: each thing it claims to catch is put in front of it, and the sources
 * as they stand are shown passing. Both directions, or neither.
 *
 * What these prove, and what they do not. They prove each rule can fire and that the sources satisfy them as
 * written. They do not prove that an event reaches a window: the known-bad cases are sources, so a listener
 * that would never be called looks the same here as one that is, and a rename applied through a macro rather
 * than an attribute is outside of what a reading of text can see. Each mutation is caught by the rule it is
 * meant to test, and nothing else may speak.
 *
 * The rule that pairs documents by what a field says on both sides has two controls: a field the contract
 * declares and the struct behind the pair does not write is a finding, and a shape written inside a document
 * is named as a shape rather than compared, because nothing names it. The second cannot fail, so what its
 * control shows is that it names the shapes that exist today and does not turn a field inside one into a
 * field of the document — which is the false finding this rule used to make before the body of an interface
 * was read to its own closing brace.
 */

import { check, readDocuments } from "./check-contract.js";

const CONTRACT = "apps/ui/src/lib/ipc/types.ts";
const BRIDGE = "apps/ui/src/lib/ipc/bridge.ts";
const STORE = "apps/ui/src/lib/link/linkStore.ts";
const APP = "crates/mpcraft-app/src/lib.rs";
const VEHICLE = "crates/mpcraft-vehicle/src/lib.rs";
const MAIN = "apps/desktop/src/main.rs";

const docs = readDocuments();

/** Replaces one line with another, and says so when the line to change is not there. */
function mutate(source, pattern, replacement) {
  const line = source.split("\n").find((row) => pattern.test(row));

  if (line === undefined) {
    return { error: `nothing in ${source.slice(0, 40)} looks like ${pattern}` };
  }

  return source.replace(line, replacement);
}

/** Takes the casing attribute off one struct, which is what it looked like before the rule existed. */
function takeRenameOff(source, name) {
  const pattern = new RegExp(
    `#\\[serde\\(rename_all = "camelCase"\\)\\]\\n(?:pub )?struct ${name}`,
  );
  const found = pattern.exec(source);

  if (found === null) {
    return {
      error: `nothing in ${source.slice(0, 40)} is a camelCase rename on ${name}`,
    };
  }

  return source.replace(
    found[0],
    found[0].replace('#[serde(rename_all = "camelCase")]\n', ""),
  );
}

const checks = [];

// The known-good case, before anything is disturbed.
const good = check(docs);

checks.push({
  claim:
    "the sources as they stand speak one casing, read each event as the document the contract declares, and listen only to what the backend emits",
  ok: good.faults.length === 0 && good.facts.listeners.length > 0,
  saw: {
    events: good.facts.events,
    listeners: good.facts.listeners,
    documents: good.facts.pairedDocuments.length,
    fields: good.facts.comparedFields,
    pairings: good.facts.pairings,
    inlineShapes: good.facts.inlineShapes,
    faults: good.faults,
  },
});

const cases = [
  {
    claim:
      "a listener for a name nothing emits is caught — which is what the dead subscription was",
    mutated: {
      ...docs,
      [BRIDGE]: mutate(
        docs[BRIDGE],
        /listen<StateEvent>\("link-state"/,
        'const unlisten = listen<StateEvent>("telemetry", (event) => {',
      ),
    },
    expect: /listens to "telemetry", which nothing emits/,
  },
  {
    claim:
      "a listener reading a shape of its own rather than the document the contract declares is caught",
    mutated: {
      ...docs,
      [STORE]: mutate(
        docs[STORE],
        /listen<StateEvent>\("link-state"/,
        'void listen<{ status: ConnectionStatus }>("link-state", (event) => {',
      ),
    },
    expect:
      /reads "link-state" as \{ status: ConnectionStatus \}, which the contract does not declare/,
  },
  {
    claim:
      "the same event read as two documents is caught: one reader holds the contract's document and another holds its own",
    mutated: {
      ...docs,
      [CONTRACT]: `${docs[CONTRACT]}\nexport interface LinkState {\n  readonly status: string;\n}\n`,
      [BRIDGE]: mutate(
        docs[BRIDGE],
        /listen<StateEvent>\("link-state"/,
        'const unlisten = listen<LinkState>("link-state", (event) => {',
      ),
    },
    expect: /is read as LinkState and written as StateEvent/,
  },
  {
    claim:
      "a document the contract declares with an underscore in a field is caught",
    mutated: {
      ...docs,
      [CONTRACT]: mutate(
        docs[CONTRACT],
        /readonly distanceToHome: number \| null;/,
        "  readonly distance_to_home: number | null;",
      ),
    },
    expect:
      /the contract declares Telemetry.distance_to_home, which is not the casing the boundary speaks/,
  },
  {
    claim:
      "a struct the boundary reaches that writes its fields with underscores is caught — the pair, before the rename was put on it",
    mutated: {
      ...docs,
      [APP]: takeRenameOff(docs[APP], "VehicleKey"),
    },
    expect:
      /VehicleKey \(crates\/mpcraft-app\/src\/lib.rs\) writes (system_id|component_id)/,
  },
  {
    claim:
      "the same rule reads the pair on the side that holds the vehicle state, because the document that carries it is written from there",
    mutated: { ...docs, [VEHICLE]: takeRenameOff(docs[VEHICLE], "VehicleId") },
    expect:
      /VehicleId \(crates\/mpcraft-vehicle\/src\/lib.rs\) writes (system_id|component_id)/,
  },
  {
    claim:
      "a file declaring its own copy of a document the contract already declares is caught",
    mutated: {
      ...docs,
      [STORE]: `${docs[STORE]}\ninterface PlaybackStatePayload {\n  readonly open: boolean;\n  readonly position: number;\n  readonly duration: number;\n  readonly telemetry: Telemetry;\n}\n`,
    },
    expect:
      /linkStore.ts declares its own PlaybackStatePayload for what the contract already declares as PlaybackState/,
  },
  {
    claim:
      "the fields of a pair are compared even when the two sides name the document differently — Telemetry is read and TelemetryEvent is written, which the first version of this gate could not see",
    mutated: {
      ...docs,
      [MAIN]: mutate(docs[MAIN], /^    battery: Option<BatteryEvent>,$/, ""),
    },
    expect:
      /Telemetry↔TelemetryEvent\.battery is read by the contract and never written by TelemetryEvent/,
  },
  {
    claim:
      "a pair is what the same field says on both sides rather than a table somebody writes: a reader that declares another document for the field is read against what the writer sends through it",
    mutated: {
      ...docs,
      [CONTRACT]: mutate(
        docs[CONTRACT],
        /^\s*readonly telemetry: Telemetry;$/,
        "  readonly telemetry: LossStats;",
      ),
    },
    expect: /LossStats↔TelemetryEvent/,
  },
];

for (const entry of cases) {
  if (
    Object.values(entry.mutated).some(
      (text) => typeof text === "object" && text?.error,
    )
  ) {
    const missing = Object.values(entry.mutated).find((text) => text?.error);

    checks.push({ claim: entry.claim, ok: false, saw: missing.error });
    continue;
  }

  const result = check(entry.mutated);
  const fired = result.faults.filter((line) => entry.expect.test(line));

  /* The mutation has to be caught by the rule it is meant to test, and nothing else may speak. */
  checks.push({
    claim: entry.claim,
    ok: fired.length > 0 && fired.length === result.faults.length,
    saw: result.faults,
  });
}

/* The pairing rule reports which side carries a field the other does not, so its control is that a field read
   by the contract and never written is a finding, and that a shape inside a document is named as a shape
   rather than read as fields of the document — an inner field the writer does not send must not become a fault
   the gate cannot support. */
const shape = check({
  ...docs,
  [CONTRACT]: mutate(
    docs[CONTRACT],
    /^    readonly radiusNm: number;$/,
    "    readonly nothingTheWriterSends: boolean;",
  ),
});

checks.push({
  claim:
    "a shape written inside a document is named as a shape, and a field inside it is not read as a field of the document",
  ok:
    shape.faults.length === 0 &&
    shape.facts.inlineShapes.includes("AdsbRelayStatus.settings"),
  saw: { faults: shape.faults, inlineShapes: shape.facts.inlineShapes },
});

const fails = checks.filter((entry) => !entry.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove each rule can fire and that the sources satisfy them as written. They do not show an event reaching a window, and a rename applied through a macro rather than an attribute is outside of what a reading of text can see. The pairing rule cannot see a document that no field names, and a shape written inside a document has no name to be read against, so its control shows what it names rather than what it refuses.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
