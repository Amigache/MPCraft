#!/usr/bin/env node
/**
 * The IPC contract, read from both sides.
 *
 * The boundary is one document spoken two ways: the Rust side writes JSON with the serde rule of each
 * struct, and the TypeScript side declares what it reads. Nothing in the repository checked that the two
 * said the same thing, and the drift that this gate found was not a style disagreement — it was a field
 * that arrives spelled one way and is read another, which is a value the interface never gets.
 *
 * What it reads, and the eight things it asserts:
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
 *   - A command crosses the boundary as well: it answers with a document and reads one. Every call the
 *     frontend makes is to a command the backend answers, and the document the call reads the answer as is
 *     compared with the document that command answers with. A shape written at the call site has no name, so
 *     there is no answer to read it against, and that is refused rather than skipped.
 *   - The way in changes what a field means. On the way back a field the struct carries and the contract
 *     does not declare is only a fact, because JSON carries what the interface does not ask for; on the way
 *     in it is a finding, because the backend expects a value the interface never sends — which is the same
 *     class as a field the reader asks for and the writer never sends, seen from the other side. The
 *     argument's own name is what pairs it with the contract: either the contract declares that name, or a
 *     field says the pair for it, and nothing here writes a table of names.
 *
 * What this cannot see, and is said rather than pretended otherwise. It reads sources as text, so a
 * rename applied through a helper macro or a field written by a manual `Serialize` implementation is not
 * seen; a pairing only exists where a field names a document on both sides or where a call or an argument
 * names one, so a document the contract declares and nothing names is not compared — which is the case for
 * the error shape, read as `CommandError` while the wire carries it inside a result rather than as an answer
 * of its own; a shape written inside a document is refused, because nothing names it, and that is the whole
 * of what can be said about an anonymous shape without a name; an argument typed with a struct the contract
 * has no name for cannot be compared, and which fields the interface actually puts on the wire at a call site
 * is outside of what this reads, because the call sites pass object literals built by a helper; and whether an
 * event actually reaches a window is outside of what a reading of two source files can say.
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

/** The commands the backend answers: what each one answers with, and what it reads as its argument. */
function answeredCommands(main, documents) {
  const out = new Map();

  for (const match of main.matchAll(
    /#\[tauri::command\]\n(?:pub )?async fn (\w+)\(([^)]*)\)\s*-> ([^\n{]+)/g,
  )) {
    const read = [];

    for (const argument of match[2].split(",")) {
      const parts = argument.split(":");

      if (parts.length !== 2) {
        continue;
      }

      /* The state handle is not a document: it is the application, and it never crosses the boundary. */
      const type = parts[1].trim();

      if (!/^[A-Z]/.test(type) || type.startsWith("State")) {
        continue;
      }

      read.push({ name: parts[0].trim(), type, path: FILES.boundary });
    }

    out.set(match[1], {
      answers: firstDocument(match[3], documents),
      reads: read,
    });
  }

  return out;
}

/** The commands the frontend calls, and the document each call reads the answer as. */
function calledCommands(docs, contract) {
  const calls = [];

  for (const [path, text] of Object.entries(docs)) {
    if (!path.startsWith("apps/ui/")) {
      continue;
    }

    for (const match of text.matchAll(
      /(?:call|invoke)<([^>]+)>\(\s*\n?\s*"([a-z_]+)"/g,
    )) {
      calls.push({ path, type: match[1].trim(), command: match[2] });
    }
  }

  return calls;
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
  const pairList = [];
  const seenPairs = new Set();
  const inlineShapes = [];
  const unpairedCalls = [];
  const readPairs = new Set();

  function addPair(document, struct, parent = null) {
    const key = `${document}→${struct}`;

    if (seenPairs.has(key)) {
      return;
    }

    seenPairs.add(key);
    pairList.push([document, struct, parent]);
  }

  for (const name of contract.keys()) {
    if (reachable.has(name)) {
      addPair(name, name);
    }
  }

  /* Rule 7: a command crosses the boundary as well — it answers with a document and reads one — so the pair a
     call site makes is read the same way as the pair an event carries. A call to a name nothing answers is a
     call that will never be answered, and a shape written at the call site has no name, so it is refused rather
     than skipped. */
  const commands = answeredCommands(docs[FILES.boundary], structs);
  const calls = calledCommands(docs, contract);

  facts.commands = commands.size;
  facts.calls = calls.length;
  facts.commandsNotCalled = [...commands.keys()]
    .filter((name) => !calls.some((call) => call.command === name))
    .sort();

  for (const call of calls) {
    const answered = commands.get(call.command);

    if (answered === undefined) {
      faults.push(
        `${call.path} calls "${call.command}", which nothing answers`,
      );
      continue;
    }

    if (call.type.includes("{")) {
      inlineShapes.push(`"${call.command}" in ${call.path}`);
      faults.push(
        `${call.path} reads "${call.command}" as a shape written at the call site, which nothing names, so its fields cannot be read against the command that answers`,
      );
      continue;
    }

    const document = firstDocument(call.type, contract);

    if (
      document === null ||
      answered.answers === null ||
      !structs.has(answered.answers)
    ) {
      /* The answer is not read as a document here: a primitive, or a command answered with one and read as
         something else. That is said as a fact, not as a fault. */
      unpairedCalls.push(`${call.command} (${call.path})`);
      continue;
    }

    addPair(document, answered.answers);
  }

  /* Pass one builds every pair before anything is compared, because a pair grows the pairs its own fields name,
     and a document read through another name can only be read against what the same field says on the other side.
     The parent is kept so that the side a pair is read on travels to the pairs that grow out of it. */
  for (let index = 0; index < pairList.length; index += 1) {
    const [document, struct, parent] = pairList[index];
    const declared = contract.get(document);
    const value = structs.get(struct);

    for (const field of declared.fields) {
      const key = words(field.name).join(".");
      const written = value.fields.find(
        (other) => words(other.name).join(".") === key,
      );

      if (written === undefined) {
        continue;
      }

      /* A shape written inside a document has no name, so there is no struct it can be read against. That is
         refused rather than skipped: an anonymous shape is a document one side holds and the other cannot be
         checked against, which is how a field of it can be read without ever arriving. */
      if (field.inline) {
        inlineShapes.push(`${pair(document, struct)}.${field.name}`);
        faults.push(
          `${pair(document, struct)}.${field.name} is a shape written inside the document, which nothing names, so its fields cannot be read against the struct that writes them (${value.path})`,
        );
        continue;
      }

      const tsDocument = firstDocument(field.type, contract);
      const rustDocument = firstDocument(written.type, structs);

      if (tsDocument === null || rustDocument === null) {
        continue;
      }

      addPair(tsDocument, rustDocument, `${document}→${struct}`);
    }
  }

  /* Rule 8 reads the way in. A command's argument names a document on the Rust side, and the contract's spelling
     of it is either the same name or the pair its own field says — never a table written here. The direction
     changes what a field means: on the way back a field the struct carries and the interface does not ask for is
     only a fact, because JSON carries what the interface does not ask for; on the way in it is a finding, because
     the backend expects a value the interface never sends, which is the same class as a field the reader asks for
     and the writer never sends. */
  const spellingOfStruct = new Map();

  for (const [document, struct] of pairList) {
    if (!spellingOfStruct.has(struct)) {
      spellingOfStruct.set(struct, document);
    }
  }

  let typedArguments = 0;
  const argumentsNotDeclared = [];
  const argumentsThroughDocuments = [];

  for (const [command, answered] of commands) {
    for (const argument of answered.reads) {
      typedArguments += 1;

      const read = firstDocument(argument.type, structs);

      if (read === null) {
        /* The argument is not read as a document here: a primitive, or a name the sources give as an enum or a
           type alias rather than a struct. That is said as a fact, not as a fault. */
        argumentsNotDeclared.push(`${command}(${argument.type})`);
        continue;
      }

      const declared = contract.has(read) ? read : spellingOfStruct.get(read);

      if (declared === undefined) {
        /* The interface has no document named for what this command reads, so there is nothing to compare it
           against: the call site sends a shape it decided for itself, and that is said as a fact. */
        argumentsNotDeclared.push(`${command}(${argument.type})`);
        continue;
      }

      readPairs.add(`${declared}→${read}`);
      argumentsThroughDocuments.push(`${pair(declared, read)} (${command})`);
    }
  }

  /* The side travels down: a field of a document the interface sends is sent as well, so every pair that grows
     out of a read pair is read on the way in too. */
  for (const [document, struct, parent] of pairList) {
    if (parent !== null && readPairs.has(parent)) {
      readPairs.add(`${document}→${struct}`);
    }
  }

  facts.typedArguments = typedArguments;
  facts.argumentsThroughDocuments = argumentsThroughDocuments.sort();
  facts.argumentsNotDeclared = argumentsNotDeclared.sort();

  const gaps = [];

  for (const [document, struct] of pairList) {
    const declared = contract.get(document);
    const value = structs.get(struct);
    const reads = readPairs.has(`${document}→${struct}`);

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
           whether the pair is named the same on both sides or read through another name. On the way in the same
           field is the other half of the same thing — the interface sends it and the backend never reads it. */
        faults.push(
          reads
            ? `${pair(document, struct)}.${field.name} is declared by the contract as what the interface sends and never read by ${struct} (${value.path})`
            : `${pair(document, struct)}.${field.name} is read by the contract and never written by ${struct} (${value.path})`,
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
         It is said as a fact, so a document read in part is not read as a document read whole. On the way in
         the same field is a finding, because the backend expects something the interface never sends. */
      if (
        ![...declared.fields].some(
          (other) => words(other.name).join(".") === key,
        )
      ) {
        if (reads) {
          faults.push(
            `${pair(document, struct)}.${field.name} is expected by ${struct} (${value.path}) and never declared by the contract, so the backend reads a value the interface does not send`,
          );
          continue;
        }

        gaps.push({
          pair: pair(document, struct),
          side: "written but not declared",
          field: field.name,
        });
      }
    }
  }

  const pairedDocuments = new Set(pairList.map(([document]) => document));

  for (const name of contract.keys()) {
    if (pairedDocuments.has(name)) {
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
  facts.pairings = pairList
    .filter(([document, struct]) => document !== struct)
    .map(([document, struct]) => pair(document, struct))
    .sort();
  facts.pairedDocuments = [...pairedDocuments].sort();
  facts.unpairedDocuments = unpaired.sort();
  facts.inlineShapes = inlineShapes.sort();
  facts.gaps = gaps.sort((a, b) =>
    `${a.pair}.${a.field}`.localeCompare(`${b.pair}.${b.field}`),
  );
  facts.unpairedCalls = unpairedCalls.sort();

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
          "this reads sources as text. A rename applied through a macro or a manual Serialize implementation is not seen; a pairing exists only where a field names a document on both sides or where a call or an argument names one, so a document the contract declares and nothing names is not compared — three are today: CommandError, reached as the error half of a result rather than as an answer of its own, and FormationRequest and TileBounds, which no call reads through them; a shape written inside a document or at a call site is refused because nothing names it; a field the writer carries and the interface does not read is said as a fact on the way back and as a finding on the way in, so the side is kept as its own fact rather than assumed; an argument typed with a struct the contract has no name for cannot be compared, and which fields the interface actually puts on the wire at a call site is not read here, because the calls pass object literals built by a helper; a command nothing calls is said as a fact, because the rule reads calls from the frontend towards the backend and not the other way round; what the boundary does not reach — the settings file and the log formats — keeps its own spelling and is not read here; and whether an event reaches a window is outside of what two source files can say.",
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
