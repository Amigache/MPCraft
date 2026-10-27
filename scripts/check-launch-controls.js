#!/usr/bin/env node
/**
 * The controls of the launch check: each thing it claims to catch is put in front of it, and the
 * documents as they stand are shown passing. Both directions, or neither.
 *
 * What these prove, and what they do not. They prove each rule can fire and that the documents satisfy
 * them as written. They do not prove the application opens: no control here runs cargo, connects to a
 * dev server, or shows a window, and the rule the CLI uses to decide where npm reads from was measured
 * once (a probe printing its own cwd) rather than re-checked by these controls.
 */

import { readFileSync } from "node:fs";

import { check } from "./check-launch.js";

const FILES = {
  uiPackage: "apps/ui/package.json",
  desktopPackage: "apps/desktop/package.json",
  desktopConfig: "apps/desktop/tauri.conf.json",
  viteConfig: "apps/ui/vite.config.ts",
  playwrightConfig: "apps/ui/playwright.config.ts",
};

const root = new URL("../", import.meta.url);

const docs = {};

for (const path of Object.values(FILES)) {
  docs[path] = readFileSync(new URL(path, root), "utf8");
}

/** Replaces one line with another, and says so when the line to change is not there. */
function mutate(source, pattern, replacement) {
  const line = source.split("\n").find((row) => pattern.test(row));

  if (line === undefined) {
    return { error: `nothing in ${source.slice(0, 40)} looks like ${pattern}` };
  }

  return source.replace(line, replacement);
}

const checks = [];

// The known-good case, before anything is disturbed.
const good = check(docs);

checks.push({
  claim:
    "the documents as they stand carry a launch that reaches the dev server and the build",
  ok: good.faults.length === 0 && good.facts.entry?.runsFrom === "apps/desktop",
  saw: {
    entry: good.facts.entry,
    chains: good.facts.beforeDevCommand?.chain,
    faults: good.faults,
  },
});

const cases = [
  {
    claim:
      "an entry that runs the CLI from the folder whose package.json holds it is caught",
    mutated: {
      ...docs,
      [FILES.uiPackage]: mutate(
        docs[FILES.uiPackage],
        /"launch":/,
        '    "launch": "tauri dev"',
      ),
    },
    expect: /does not hold apps\/desktop\/tauri.conf.json/,
  },
  {
    claim: "an entry that names the CLI without declaring it is caught",
    mutated: {
      ...docs,
      [FILES.uiPackage]: mutate(
        docs[FILES.uiPackage],
        /"@tauri-apps\/cli":/,
        '    "@types/react-dom": "^19.1.0",',
      ),
    },
    expect: /@tauri-apps\/cli is not declared/,
  },
  {
    claim:
      "a folder holding the configuration with no package.json is caught — the ENOENT the launch had",
    mutated: { ...docs, [FILES.desktopPackage]: null },
    expect:
      /npm would read no package.json from apps\/desktop|nothing to read from the folder/,
  },
  {
    claim:
      "a command naming a script that is not in the package.json npm reads from where it runs is caught",
    mutated: {
      ...docs,
      [FILES.desktopConfig]: mutate(
        docs[FILES.desktopConfig],
        /"beforeDevCommand":/,
        '    "beforeDevCommand": "npm run serve",',
      ),
    },
    expect: /names serve, which is not a script of apps\/desktop\/package.json/,
  },
  {
    claim: "a delegation that never ends is caught",
    mutated: {
      ...docs,
      [FILES.desktopPackage]: mutate(
        docs[FILES.desktopPackage],
        /"dev":/,
        '    "dev": "npm --prefix ../desktop run dev",',
      ),
    },
    expect: /does not end: it comes back to itself/,
  },
  {
    claim:
      "a configuration expecting one port while the dev server binds another is caught",
    mutated: {
      ...docs,
      [FILES.desktopConfig]: mutate(
        docs[FILES.desktopConfig],
        /"devUrl":/,
        '    "devUrl": "http://localhost:1421",',
      ),
    },
    expect: /expects port 1421 and the dev server binds 1420/,
  },
  {
    claim:
      "a browser run connecting to a port the dev server does not bind is caught",
    mutated: {
      ...docs,
      [FILES.playwrightConfig]: mutate(
        docs[FILES.playwrightConfig],
        /baseURL:/,
        '  baseURL: "http://localhost:1421",',
      ),
    },
    expect: /connects to port 1421/,
  },
  {
    claim:
      "the configuration looking for the frontend where the build does not write is caught",
    mutated: {
      ...docs,
      [FILES.desktopConfig]: mutate(
        docs[FILES.desktopConfig],
        /"frontendDist":/,
        '    "frontendDist": "../ui/out",',
      ),
    },
    expect:
      /looks for the frontend at apps\/ui\/out and the build writes to apps\/ui\/dist/,
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

const fails = checks.filter((entry) => !entry.ok);

console.log(
  JSON.stringify(
    {
      checks,
      verdict: fails.length === 0 ? "PASS" : "FAIL",
      fails,
      limit:
        "these prove each rule can fire and that the documents satisfy them as written. They do not open a window, run cargo, or connect to a dev server, and the CLI's own rule for where npm reads from was measured once rather than re-checked here.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
