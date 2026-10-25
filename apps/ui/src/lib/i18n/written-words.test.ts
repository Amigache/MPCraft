import { describe, expect, it } from "vitest";

import { DEFAULT_LOCALE, entry, LOCALES } from "./strings";

/**
 * What the interface asks the catalogue for, taken from the code rather than from a list
 * somebody keeps by hand.
 *
 * A list kept by hand says what it was written to say, not what the interface says, and the
 * two drift apart the first time a word is added. So this reads the source.
 *
 * The files are read through the build tool rather than through the filesystem, so that the
 * same check runs the same way in the browser and in the test runner: nothing here knows that
 * a disk exists.
 *
 * What this buys, and what it does not. It catches a word that goes through the catalogue and has
 * no entry, whether or not the parts of the sentence are named with it; and it catches a word that
 * never goes through the catalogue when that word is put on the screen by one of the two channels
 * where what is written is what is drawn: the note of the bar, the report of an action, and the
 * label or hint of the bar itself. It does not catch a wrong translation, and it does not catch a
 * word written as text inside a component by hand — a pattern that only reads quotes cannot see
 * those. Those are said rather than pretended otherwise.
 */
function wordsTheCodeWrites(): string[] {
  const sources = readSources();

  const keys = new Set<string>();

  for (const [name, source] of Object.entries(sources)) {
    if (name.endsWith("/strings.ts")) {
      continue;
    }

    for (const match of String(source).matchAll(/\bt\("([^"]+)"/g)) {
      const key = match[1];

      if (key !== undefined) {
        keys.add(key);
      }
    }
  }

  return [...keys].sort();
}

/**
 * The words that are put on the screen without being looked up.
 *
 * Two channels, and they are the two where what is written is what is drawn:
 *
 * • the note of the bar and the report of an action, which the status bar and the panels show as
 *   they were given;
 * • the label and the hint of the bar itself, because the bar draws its items as it is given
 *   them — a word in its list is not translated on the way to the screen.
 *
 * A `label` written in another file is not the same thing: the quick view, the actions panel and
 * the plan editor keep names that belong to the vehicle or to the file (a mode name, a port name,
 * a language said in its own words), and those are data, not words. So this rule is only for the
 * bar, which is the file that draws its own labels.
 */
function wordsWrittenStraightInto(source: string, isTheBar: boolean): string[] {
  const found = new Set<string>();

  const patterns = [/\b(?:setNote|report)\(\s*(?:null,\s*)?"([^"]+)"/g];

  if (isTheBar) {
    patterns.push(/\b(?:label|hint):\s*"([^"]+)"/g);
  }

  for (const pattern of patterns) {
    for (const match of source.matchAll(pattern)) {
      const word = match[1];

      if (word !== undefined) {
        found.add(word);
      }
    }
  }

  return [...found];
}

/** The sources of the interface, read as text. */
function readSources(): Record<string, unknown> {
  return {
    ...import.meta.glob("../../**/*.ts", {
      query: "?raw",
      import: "default",
      eager: true,
    }),
    ...import.meta.glob("../../**/*.tsx", {
      query: "?raw",
      import: "default",
      eager: true,
    }),
  };
}

describe("the words the code writes", () => {
  const words = wordsTheCodeWrites();

  it("are looked up in the catalogue rather than written into a component", () => {
    // The gate has to be able to fail on a word that is not in the catalogue. A known-bad word
    // is put through it here, so that a pass means the check can find something.
    expect(entry("es", "A word that is not in the catalogue")).toBeUndefined();
    expect(words.length, "the code writes no words").toBeGreaterThan(0);
  });

  it("have an entry in every language the window can be read in", () => {
    const missing = words.filter((word) =>
      LOCALES.some(
        (locale) =>
          locale.id !== DEFAULT_LOCALE && entry(locale.id, word) === undefined,
      ),
    );

    expect(missing, `words with no entry: ${missing.join(", ")}`).toEqual([]);
  });
});

describe("the words written straight into the interface", () => {
  it("can still be found, on a word that is known to be bad", () => {
    // Without this the green would mean nothing: a check that cannot find anything and a check
    // that finds nothing look the same from the outside.
    expect(
      wordsWrittenStraightInto(
        'setNote("A word written straight into the bar");',
        false,
      ),
    ).toEqual(["A word written straight into the bar"]);

    // The bar's own rule is checked the same way, on its own shape.
    expect(
      wordsWrittenStraightInto(
        '{ label: "A word written straight into the bar" }',
        true,
      ),
    ).toEqual(["A word written straight into the bar"]);
  });

  it("are none: what the bar says is looked up rather than written down", () => {
    const offenders: string[] = [];

    for (const [name, source] of Object.entries(readSources())) {
      // A test file writes words for its own assertions; those are not words the operator reads.
      if (name.endsWith(".test.ts")) {
        continue;
      }

      for (const word of wordsWrittenStraightInto(
        String(source),
        name.endsWith("/MenuBar.tsx"),
      )) {
        offenders.push(`${name}: ${word}`);
      }
    }

    expect(offenders, `words not looked up: ${offenders.join(", ")}`).toEqual(
      [],
    );
  });
});
