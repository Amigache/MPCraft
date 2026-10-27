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
 * is refused because nothing names it — which is the whole of what can be said about an anonymous shape, so the
 * sources are shown naming what they declare rather than holding it as a shape. The rule that reads calls has
 * three: a call to a name nothing answers, a shape written at the call site, and the pair the call makes
 * compared field by field against the command that answers it.
 */

import { check, readDocuments } from "./check-contract.js";

const CONTRACT = "apps/ui/src/lib/ipc/types.ts";
const BRIDGE = "apps/ui/src/lib/ipc/bridge.ts";
const STORE = "apps/ui/src/lib/link/linkStore.ts";
const APP = "crates/mpcraft-app/src/lib.rs";
const VEHICLE = "crates/mpcraft-vehicle/src/lib.rs";
const MAIN = "apps/desktop/src/main.rs";
const OPERATIONS = "crates/mpcraft-app/src/operations.rs";

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
    commands: good.facts.commands,
    calls: good.facts.calls,
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
  {
    claim:
      "a shape written inside a document is refused rather than skipped: nothing names it, so there is no struct its fields can be read against",
    mutated: {
      ...docs,
      [CONTRACT]: mutate(
        docs[CONTRACT],
        /^  readonly settings: TransponderSettings;$/,
        "  readonly settings: { readonly icaoAddress: number; };",
      ),
    },
    expect:
      /TransponderStatus\.settings is a shape written inside the document, which nothing names/,
  },
  {
    claim:
      "a call to a name nothing answers is caught — a call that will never be answered",
    mutated: {
      ...docs,
      [BRIDGE]: mutate(
        docs[BRIDGE],
        /^\s*await call<void>\("arm", vehicleRequest\(vehicle\)\);$/,
        '  await call<void>("arm_vehicle", vehicleRequest(vehicle));',
      ),
    },
    expect: /calls "arm_vehicle", which nothing answers/,
  },
  {
    claim:
      "a shape written at a call site is refused rather than skipped: nothing names it, so there is no command answer to read it against",
    mutated: {
      ...docs,
      [BRIDGE]: mutate(
        docs[BRIDGE],
        /^\s*return call<PartialUpdateInfo>\("upload_mission_partial", \{$/,
        '  return call<{ start: number }>("upload_mission_partial", {',
      ),
    },
    expect:
      /reads "upload_mission_partial" as a shape written at the call site, which nothing names/,
  },
  {
    claim:
      "the pair a call makes is compared like the pair an event carries: the document read through a call is read against what that command answers",
    mutated: {
      ...docs,
      [OPERATIONS]: mutate(
        docs[OPERATIONS],
        /^    pub items: Vec<PlanItem>,$/,
        "",
      ),
    },
    expect:
      /Plan↔PlanInfo\.items is read by the contract and never written by PlanInfo/,
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

/* The rule refuses an anonymous shape, so its green rests on the contract having named what it declares: the
   four places that were shapes inside a document are documents now, and each is read against the struct that
   writes it. */
checks.push({
  claim:
    "the contract names every document it declares as its own, so the four shapes that used to sit inside one are compared as documents",
  ok:
    good.facts.inlineShapes.length === 0 &&
    [
      "AdsbRelaySettings",
      "MessageCount",
      "TransponderSettings",
      "UnreadPlace",
    ].every((name) => good.facts.pairedDocuments.includes(name)),
  saw: {
    inlineShapes: good.facts.inlineShapes,
    named: [
      "AdsbRelaySettings",
      "MessageCount",
      "TransponderSettings",
      "UnreadPlace",
    ],
    fields: good.facts.comparedFields,
  },
});

const fails = checks.filter((entry) => !entry.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove each rule can fire and that the sources satisfy them as written. They do not show an event reaching a window, and a rename applied through a macro rather than an attribute is outside of what a reading of text can see. The pairing rule cannot see a document that no field names — the ones read through a command's answer — and an anonymous shape is refused without being compared, because there is nothing to compare it with until it is named.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
