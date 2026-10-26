#!/usr/bin/env node
/**
 * How a gate reads its own output, kept apart so the reading can be shown working.
 *
 * A count is not the same as a count of the right thing. When the integration gate answers against a
 * live vehicle the run holds several test binaries, and the first report it prints is the one for the
 * unit tests — which say nothing about the vehicle. The selector below reads the report that belongs to
 * the binary named in the heading, so a green rests on what the vehicle answered rather than on the
 * tests that ran before it.
 */

/** The colours a runner puts in its report are not part of what it says. */
export function plain(text) {
  return String(text ?? "").replace(/\u001b\[[0-9;]*m/g, "");
}

export function lineMatching(text, pattern) {
  const line = plain(text)
    .split("\n")
    .find((row) => pattern.test(row));

  return line === undefined ? "no line said what was expected" : line.trim();
}

/**
 * The report of one test binary, paired with the heading that opens its section.
 *
 * The position in the joined text cannot be trusted: cargo prints the headings on one stream and the
 * reports on the other, so joining them puts every report before every heading, which is what made this
 * reading say nothing over a run that ran. What is stable is the order — the reports come in the same
 * order the headings do, so the kth heading takes the kth report.
 *
 * A heading that never appears means the binary did not run, which is said rather than covered by the
 * nearest count: a live area that never started cannot be read as an area that passed.
 */
export function resultForSection(text, section) {
  /** The separator a runner prints is not part of what it says: on this machine cargo writes `tests\sitl.rs`. */
  const lines = plain(text)
    .split("\n")
    .map((row) => row.replaceAll("\\", "/"));

  const headings = lines.filter((row) => /^\s*(Running|Doc-tests)\s/.test(row));
  const reports = lines.filter((row) => /test result:/.test(row));

  if (headings.length === 0) {
    return `the ${section} test binary did not run`;
  }

  const at = headings.findIndex((row) => row.includes(`tests/${section}.rs`));

  if (at === -1) {
    return `the ${section} test binary did not run`;
  }

  return reports[at] ?? "no report said what was expected";
}
