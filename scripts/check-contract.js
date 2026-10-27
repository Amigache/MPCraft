#!/usr/bin/env node
/**
 * The IPC contract, read from both sides.
 *
 * The boundary is one document spoken two ways: the Rust side writes JSON with the serde rule of each
 * struct, and the TypeScript side declares what it reads. Nothing in the repository checked that the two
 * said the same thing, and the drift that this gate found was not a style disagreement — it was a field
 * that arrives spelled one way and is read another, which is a value the interface never gets.
 *
 * What it reads, and the six things it asserts:
 *
 *   - Every event the frontend listens to is one the backend emits. A listener for a name nothing
 *     emits is a subscription to a channel that will never answer.
 *   - Each event name is read as one document, declared by the contract (`apps/ui/src/lib/ipc/types.ts`)
 *     rather than shaped by whoever listens to it. Two readers of the same event drawing two shapes is
 *     how one of them ends up reading a field the sender never wrote.
 *   - A document named on both sides must be spelled one way: each field the contract declares is
 *     compared with the struct that carries it, through the words it says rather than through its
 *     casing, so `system_id` and `systemId` are read as the same field and then required to agree.
 *   - The boundary speaks camelCase. Every serialized struct reachable from the boundary — from the
 *     commands the desktop answers and the events it emits, through the types their fields name — writes
 *     its fields in the casing the contract reads, and the contract declares no field with an underscore
 *     in it.
 *   - A document that reaches the frontend under another name is paired by what the same field says on
 *     both sides — `StateEvent.telemetry` is read as `Telemetry` and written as `TelemetryEvent` — and the
 *     fields of that pair are compared like the fields of any other, which is what the first version of
 *     this gate could not do. A field the contract declares and the struct does not write is a value the
 *     interface cannot get, and that is a finding whether the pair shares a name or not; a field the struct
 *     writes and the contract does not declare is said as a fact, because JSON carries what the interface
 *     does not ask for.
 *
 * What this cannot see, and is said rather than pretended otherwise. It reads sources as text, so a
 * rename applied through a helper macro or a field written by a manual `Serialize` implementation is not
 * seen; a pairing only exists where a field names a document on both sides, so a document the contract
 * declares and no field names is not compared, and a shape written inside a document has no name at all, so
 * there is no struct it can be read against and the places where the contract writes one are listed rather
 * than compared; and whether an event actually reaches a window is outside of what a reading of two source
 * files can say.
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

/** The documents whose rules are read directly; the structs are read from every Rust source. */
const FILES = {
  boundary: "apps/desktop/src/main.rs",
  contract: "apps/ui/src/lib/ipc/types.ts",
  bridge: "apps/ui/src/lib/ipc/bridge.ts",
  store: "apps/ui/src/lib/link/linkStore.ts",
};

/** How a pair is named: one name when both sides spell it the same way, and both when they do not. */
function pair(document, struct) {
  return document === struct ? document : `${document}↔${struct}`;
}

/** Splits a field into the words it says, whichever way it is spelled. */
function words(name) {
  return name
    .split("_")
    .flatMap((part) => part.split(/(?=[A-Z])/))
    .filter(Boolean)
    .map((word) => word.toLowerCase());
}

function camelCase(name) {
  return words(name)
    .map((word, index) =>
      index === 0 ? word : `${word[0].toUpperCase()}${word.slice(1)}`,
    )
    .join("");
}

/** Every serialized struct in a Rust source, with the casing its attributes give it. */
function rustStructs(text, path) {
  const out = new Map();

  for (const match of text.matchAll(
    /((?:\s*#[^\n]*\n)*)\s*(?:pub )?struct (\w+)[^\{]*\{([\s\S]*?)\n\}/g,
  )) {
    const attrs = match[1];

    /* A document crosses the boundary in both directions: what the backend writes and what it reads. */
    if (!/Serialize|Deserialize/.test(attrs)) {
      continue;
    }

    const fields = [
      ...match[3].matchAll(
        /^[\t ]*(?:pub )?([a-z_][A-Za-z0-9_]*)\s*:\s*([^\n,]+)/gm,
      ),
    ].map((field) => ({ name: field[1], type: field[2].trim() }));

    out.set(match[2], {
      path,
      camelCase: /rename_all\s*=\s*"camelCase"/.test(attrs),
      fields,
    });
  }

  return out;
}

/** Every document the TypeScript side declares, with the type each field is read as. */
function tsInterfaces(text, path) {
  const out = new Map();

  for (const match of text.matchAll(/(?:export )?interface (\w+) \{/g)) {
    /* The body is read to its own closing brace, so a field of a nested shape is not taken as a field of
       the document — and a shape written inside the document is named by nothing, which is said below. */
    const start = match.index + match[0].length;
    let depth = 1;
    let end = -1;

    for (let i = start; i < text.length; i += 1) {
      if (text[i] === "{") {
        depth += 1;
      } else if (text[i] === "}") {
        depth -= 1;

        if (depth === 0) {
          end = i;
          break;
        }
      }
    }

    const fields = [];
    depth = 0;

    for (const line of text.slice(start, end).split("\n")) {
      if (depth === 0) {
        const field = /^\s*readonly ([A-Za-z0-9_]+)\??\s*:\s*(.+?)\s*$/.exec(
          line,
        );

        if (field !== null) {
          fields.push({
            name: field[1],
            type: field[2],
            inline: field[2].includes("{"),
          });
        }
      }

      depth +=
        (line.match(/\{/g) ?? []).length - (line.match(/\}/g) ?? []).length;
    }

    out.set(match[1], { path, fields });
  }

  return out;
}

/** The document a type names, or nothing when the type is not a document. */
function firstDocument(type, documents) {
  for (const token of type.match(/[A-Za-z0-9_]+/g) ?? []) {
    if (documents.has(token)) {
      return token;
    }
  }

  return null;
}

/** The events the backend names, and the document each one carries. */
function emittedEvents(main, structs) {
  const constants = new Map();

  for (const match of main.matchAll(/const ([A-Z_]+): &str = "([^"]+)"/g)) {
    constants.set(match[1], match[2]);
  }

  const events = new Map();

  for (const match of main.matchAll(
    /emit\(\s*([A-Z_]+|"[a-z-]+")\s*,\s*([A-Za-z0-9_:]+)/g,
  )) {
    const named = match[1].startsWith('"')
      ? match[1].slice(1, -1)
      : constants.get(match[1]);
    const value = match[2];
    // `PlaybackState { .. }` and `StateEvent::default()` name their own document; a variable is
    // resolved through the assignment that gave it its type, which is what the emitter holds.
    let document = value.split("::")[0];

    if (!structs.has(document)) {
      const assignment = new RegExp(
        `let\\s+(?:mut\\s+)?${value}\\s*=\\s*(?:&?)(\\w+)(?:::|\s*\{)`,
      ).exec(main);
      document = assignment?.[1] ?? null;
    }

    if (named !== undefined) {
      events.set(named, { document, path: FILES.boundary });
    }
  }

  return { events, constants };
}

/** The events the frontend listens to, and the shape each listener declares for them. */
function listenedEvents(docs) {
  const listeners = [];

  for (const [path, text] of Object.entries(docs)) {
    if (!path.startsWith("apps/ui/")) {
      continue;
    }

    for (const match of text.matchAll(
      /listen<([^>]*)>\(\s*\n?\s*"([a-z-]+)"/g,
    )) {
      listeners.push({ path, event: match[2], payload: match[1].trim() });
    }
  }

  return listeners;
}

export function check(docs) {
  const faults = [];
  const facts = {};

  for (const path of Object.values(FILES)) {
    if (docs[path] === undefined) {
      faults.push(`${path} is not there to be read`);
    }
  }

  if (faults.length > 0) {
    return { faults, facts };
  }

  /* The structs, from every Rust source the boundary can reach a type from. */
  const structs = new Map();

  for (const [path, text] of Object.entries(docs)) {
    if (!path.endsWith(".rs")) {
      continue;
    }

    for (const [name, value] of rustStructs(text, path)) {
      structs.set(name, value);
    }
  }

  const contract = tsInterfaces(docs[FILES.contract], FILES.contract);

  /* Rule 1 and 2: what the frontend listens to, and as what. */
  const emitted = emittedEvents(docs[FILES.boundary], structs);
  const listeners = listenedEvents(docs);

  facts.events = [...emitted.events.keys()].sort();
  facts.listeners = listeners.map((entry) => ({
    event: entry.event,
    through: entry.payload,
    file: entry.path,
  }));

  if (listeners.length === 0) {
    faults.push(
      "the frontend listens to no event, so there is nothing to compare",
    );
  }

  for (const listener of listeners) {
    const emitter = emitted.events.get(listener.event);

    if (emitter === undefined) {
      /* A subscription to a channel that will never answer cannot be compared as a document. */
      faults.push(
        `${listener.path} listens to "${listener.event}", which nothing emits`,
      );
      continue;
    }

    // A listener reads a document, not a shape it decided for itself.
    if (!contract.has(listener.payload)) {
      faults.push(
        `${listener.path} reads "${listener.event}" as ${listener.payload}, which the contract does not declare`,
      );
    } else if (emitter.document !== listener.payload) {
      faults.push(
        `"${listener.event}" is read as ${listener.payload} and written as ${emitter.document}`,
      );
    }
  }

  /* Rule 3: documents named on both sides must be spelled one way. */
  const roots = new Set();

  for (const [name, value] of structs) {
    if (value.path === FILES.boundary) {
      roots.add(name);
    }
  }

  /* A command answers with a document and reads one: both cross the boundary, so both are roots. */
  for (const match of docs[FILES.boundary].matchAll(
    /#\[tauri::command\]\n(?:pub )?async fn \w+\([^)]*\) -> [^\n{]+/g,
  )) {
    for (const token of match[0].match(/[A-Za-z0-9_]+/g) ?? []) {
      if (structs.has(token)) {
        roots.add(token);
      }
    }
  }

  const reachable = new Set();
  const queue = [...roots];

  while (queue.length > 0) {
    const name = queue.shift();

    if (reachable.has(name) || !structs.has(name)) {
      continue;
    }

    reachable.add(name);

    for (const field of structs.get(name).fields) {
      for (const token of field.type.match(/[A-Za-z0-9_]+/g) ?? []) {
        if (structs.has(token)) {
          queue.push(token);
        }
      }
    }
  }

  facts.reachable = reachable.size;

  /* The boundary speaks one casing, so no document it declares carries an underscore in a field. */
  let compared = 0;
  const unpaired = [];
  const reported = new Set();

  for (const name of reachable) {
    const value = structs.get(name);
    const spelling = new Map();

    for (const field of value.fields) {
      const key = words(field.name).join(".");
      const written = value.camelCase ? camelCase(field.name) : field.name;

      spelling.set(key, written);

      /* A field whose name has an underscore and no rename reaches the frontend with it still in the key. */
      if (field.name.includes("_") && !value.camelCase) {
        reported.add(`${name}.${key}`);
        faults.push(
          `${name} (${value.path}) writes ${field.name}, which is not the casing the contract reads`,
        );
      }
    }

    value.spelling = spelling;
  }

  /* Rule 6: a document that reaches the frontend under another name is paired by what its field says.
     `StateEvent.telemetry` is read as `Telemetry` and written as `TelemetryEvent`, so the pair is not a
     guess — it is what the same field says on both sides — and the fields of that pair are compared like
     the fields of any other. */
  const pairs = new Map();
  const inlineShapes = [];

  for (const name of contract.keys()) {
    if (reachable.has(name)) {
      pairs.set(name, name);
    }
  }

  const walk = [...pairs.entries()];

  while (walk.length > 0) {
    const [document, struct] = walk.shift();
    const declared = contract.get(document);

    for (const field of declared.fields) {
      const key = words(field.name).join(".");
      const written = structs
        .get(struct)
        .fields.find((other) => words(other.name).join(".") === key);

      if (written === undefined) {
        continue;
      }

      /* A shape written inside a document has no name, so there is no struct it can be read against: the
         fields of that shape are not compared, and that is said rather than left as a green. */
      if (field.inline) {
        inlineShapes.push(`${document}.${field.name}`);
        continue;
      }

      const tsDocument = firstDocument(field.type, contract);
      const rustDocument = firstDocument(written.type, structs);

      if (tsDocument === null || rustDocument === null) {
        continue;
      }

      if (!pairs.has(tsDocument)) {
        pairs.set(tsDocument, rustDocument);
        walk.push([tsDocument, rustDocument]);
      }
    }
  }

  const gaps = [];

  for (const [document, struct] of pairs) {
    const declared = contract.get(document);
    const value = structs.get(struct);

    for (const field of declared.fields) {
      if (field.name.includes("_")) {
        faults.push(
          `the contract declares ${document}.${field.name}, which is not the casing the boundary speaks`,
        );
        /* The same field is not said twice: the casing is the finding, and the comparison against the struct
           would say it again with the other spelling. */
        continue;
      }

      const key = words(field.name).join(".");

      if (!value.spelling.has(key)) {
        /* The reader asks for something the writer never sends: that is a value the interface cannot get,
           whether the pair is named the same on both sides or read through another name. */
        faults.push(
          `${pair(document, struct)}.${field.name} is read by the contract and never written by ${struct} (${value.path})`,
        );
        continue;
      }

      compared += 1;
      const written = value.spelling.get(key);

      /* The same field is not said twice: a struct that does not speak camelCase is one finding. */
      if (written !== field.name && !reported.has(`${struct}.${key}`)) {
        faults.push(
          `${document}.${field.name} is read as ${field.name} and written as ${written} (${value.path})`,
        );
      }
    }

    for (const field of value.fields) {
      const key = words(field.name).join(".");

      /* A field the interface does not read is not a fault: JSON carries what the interface does not ask for.
         It is said as a fact, so a document read in part is not read as a document read whole. */
      if (
        ![...declared.fields].some(
          (other) => words(other.name).join(".") === key,
        )
      ) {
        gaps.push({
          pair: pair(document, struct),
          side: "written but not declared",
          field: field.name,
        });
      }
    }
  }

  for (const name of contract.keys()) {
    if (pairs.has(name)) {
      continue;
    }

    /* A document only the frontend declares still has to be spelled the way the boundary speaks. */
    for (const field of contract.get(name).fields) {
      if (field.name.includes("_")) {
        faults.push(
          `the contract declares ${name}.${field.name}, which is not the casing the boundary speaks`,
        );
      }
    }

    unpaired.push(name);
  }

  facts.comparedFields = compared;
  facts.pairings = [...pairs.entries()]
    .filter(([document, struct]) => document !== struct)
    .map(([document, struct]) => pair(document, struct))
    .sort();
  facts.pairedDocuments = [...pairs.keys()].sort();
  facts.unpairedDocuments = unpaired.sort();
  facts.inlineShapes = inlineShapes.sort();
  facts.gaps = gaps.sort((a, b) =>
    `${a.pair}.${a.field}`.localeCompare(`${b.pair}.${b.field}`),
  );

  /* Rule 5: a document is declared once, by the contract. */
  for (const [path, text] of Object.entries(docs)) {
    if (!path.startsWith("apps/ui/") || path === FILES.contract) {
      continue;
    }

    for (const [name, declared] of tsInterfaces(text, path)) {
      const keys = new Set(
        declared.fields.map((field) => words(field.name).join(".")),
      );

      for (const other of contract.values()) {
        if (
          other.fields.length === keys.size &&
          other.fields.every((f) => keys.has(words(f.name).join(".")))
        ) {
          faults.push(
            `${path} declares its own ${name} for what the contract already declares as ${[...contract.keys()].find((key) => contract.get(key) === other)}`,
          );
          break;
        }
      }
    }
  }

  return { faults, facts };
}

/**
 * The sources this reads: the four documents whose rules are read directly, and every Rust source the
 * boundary can reach a type from. A control hands in the same map with one document disturbed, so each rule
 * is shown firing on its known-bad case.
 */
export function readDocuments() {
  const docs = {};

  for (const path of Object.values(FILES)) {
    const file = resolve(path);

    docs[path] = existsSync(file) ? readFileSync(file, "utf8") : null;
  }

  for (const dir of ["crates", "apps/desktop/src"]) {
    for (const entry of scan(dir)) {
      docs[entry] = readFileSync(resolve(entry), "utf8");
    }
  }

  return docs;
}

if (
  process.argv[1] !== undefined &&
  process.argv[1].includes("check-contract.js")
) {
  const result = check(readDocuments());

  console.log(
    JSON.stringify(
      {
        facts: result.facts,
        verdict: result.faults.length === 0 ? "PASS" : "FAIL",
        faults: result.faults,
        limit:
          "this reads sources as text. A rename applied through a macro or a manual Serialize implementation is not seen; a pairing exists only where a field names a document on both sides, so a document the contract declares and no field names is not compared, and a shape written inside a document has no name to be read against; a field the writer carries and the interface does not read is said as a fact rather than a fault; what the boundary does not reach — the settings file and the log formats — keeps its own spelling and is not read here; and whether an event reaches a window is outside of what two source files can say.",
      },
      null,
      2,
    ),
  );

  process.exit(result.faults.length === 0 ? 0 : 1);
}

function resolve(path) {
  return ROOT + path;
}

/** The Rust sources the structs are read from — the crate sources, not the tests. */
function scan(dir) {
  const found = [];

  for (const entry of readdirSync(ROOT + dir, { recursive: true })) {
    const path = `${dir}/${String(entry).replaceAll("\\", "/")}`;

    if (path.endsWith(".rs") && !path.includes("/tests/")) {
      found.push(path);
    }
  }

  return found;
}
