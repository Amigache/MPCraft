#!/usr/bin/env node
/**
 * The parity matrix, read rather than remembered.
 *
 * The matrix is the contract: every upstream feature has to be marked Done, Planned or Dropped,
 * and nothing may be left out quietly. Its summary is written by hand, and a hand-written summary
 * says what somebody typed, not what the table holds — the two drift apart the first time a row is
 * changed without changing the count.
 *
 * So the counts are worked out from the rows and compared with the summary. What it buys, and what
 * it does not: it catches a row whose status is not in the legend, a row that names no place in
 * upstream, a planned row with no phase, a dropped row that does not say why, and a summary that
 * disagrees with the rows. It cannot catch a row that is wrong about what upstream does — that is a
 * judgement about the code, not a fact about the table.
 */

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const LEGEND = {
  "✅": "Done",
  "🔜": "Planned",
  "⬜": "Dropped",
  "🟡": "Partial",
  "🔽": "Partial, the work is under way",
};

/** The two partial marks are one family in the summary, so the family is what is counted. */
function familyOf(status) {
  const name = LEGEND[status];

  return name === undefined ? null : name.split(",")[0];
}

/** The cells of a table line, or nothing when the line is prose rather than a row. */
function cellsOf(line) {
  if (!line.startsWith("|")) {
    return null;
  }

  // The pipes at each end are not cells; an empty cell between them is, because that is what a
  // missing phase or a missing place in upstream looks like.
  const parts = line.split("|");
  parts.shift();
  parts.pop();

  const cells = parts.map((cell) => cell.trim());

  return cells.length === 0 ? null : cells;
}

function check(text) {
  const lines = text.split(/\r?\n/);

  const faults = [];
  const counted = {};
  let rows = 0;

  for (const line of lines) {
    const cells = cellsOf(line);

    if (cells === null || !/^\d+\.\d+$/.test(cells[0] ?? "")) {
      continue;
    }

    rows += 1;

    const [, id, feature, upstream, status, phase] = [line, ...cells];

    const family = familyOf(status);

    if (family === null) {
      faults.push(`row ${id} says "${status}", which is not in the legend`);
      continue;
    }

    counted[family] = (counted[family] ?? 0) + 1;

    if (upstream === "") {
      faults.push(`row ${id} names no place in upstream`);
    }

    if (family === "Planned" && phase === "") {
      faults.push(`row ${id} is planned without a phase`);
    }

    /* A feature may be left out, but not quietly: the reason has to be written beside the mark, in
       the same cell, because a row that says only "Dropped" is a decision nobody can read. */
    if (family === "Dropped" && !/^Dropped\s+—\s+\S/.test(phase)) {
      faults.push(`row ${id} is dropped without saying why`);
    }
  }

  /* The summary is read as it is written: a symbol and a number. */
  const summary = {};

  for (const line of lines) {
    const cells = cellsOf(line);

    if (cells === null || cells.length !== 2) {
      continue;
    }

    const family = Object.entries(LEGEND).find(([symbol]) =>
      cells[0].includes(symbol),
    )?.[1];

    if (family === undefined) {
      continue;
    }

    const count = Number(cells[1]);

    if (Number.isFinite(count)) {
      summary[family.split(",")[0]] = count;
    }
  }

  for (const [name, count] of Object.entries(counted)) {
    if (summary[name] !== count) {
      faults.push(
        `the summary says ${summary[name] ?? "nothing"} for ${name}, the rows make ${count}`,
      );
    }
  }

  if (!/_ref_missionplanner/.test(text)) {
    faults.push("the matrix does not say where upstream was read from");
  }

  return { rows, counted, summary, faults };
}

/**
 * Runs the check on a file when the script is run, so that the controls can call `check` on a
 * copy without writing anything to the disk.
 */
function main() {
  const path = process.argv[2] ?? "docs/PARITY.md";
  const result = check(readFileSync(path, "utf8"));

  console.log(
    JSON.stringify(
      {
        file: path,
        ...result,
        verdict: result.faults.length === 0 ? "PASS" : "FAIL",
      },
      null,
      2,
    ),
  );

  process.exit(result.faults.length === 0 ? 0 : 1);
}

if (pathToFileURL(process.argv[1] ?? "").href === import.meta.url) {
  main();
}

export { check };
