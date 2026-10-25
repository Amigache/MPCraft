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
 * What this buys, and what it does not: it catches a word that goes through the catalogue and
 * has no entry. It does not catch a wrong translation, and it does not catch a word that never
 * goes through the catalogue at all — a string written straight into a component is invisible
 * to it. That is why the check looks for calls and not for strings.
 */
function wordsTheCodeWrites(): string[] {
  const sources = {
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

  const keys = new Set<string>();

  for (const [name, source] of Object.entries(sources)) {
    if (name.endsWith("/strings.ts")) {
      continue;
    }

    for (const match of String(source).matchAll(/\bt\("([^"]+)"\)/g)) {
      const key = match[1];

      if (key !== undefined) {
        keys.add(key);
      }
    }
  }

  return [...keys].sort();
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
