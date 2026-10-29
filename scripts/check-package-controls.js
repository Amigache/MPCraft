#!/usr/bin/env node
/**
 * The controls of the packaging check: each thing it claims to catch is put in front of it, and the
 * documents as they stand are shown passing. Both directions, or neither.
 *
 * What these prove, and what they do not. They prove each rule can fire and that the documents
 * satisfy them as written. They do not build an installer, run npm or cargo, or show that a runner
 * accepts the workflow; signing and notarisation are outside of what any of them reads.
 */

import { readFileSync, readdirSync } from "node:fs";

import { check } from "./check-package.js";

const FILES = {
  uiPackage: "apps/ui/package.json",
  desktopPackage: "apps/desktop/package.json",
  config: "apps/desktop/tauri.conf.json",
  windowsConfig: "apps/desktop/tauri.windows.conf.json",
  linuxConfig: "apps/desktop/tauri.linux.conf.json",
  macosConfig: "apps/desktop/tauri.macos.conf.json",
  updaterConfig: "apps/desktop/tauri.updater.conf.json",
  workflow: ".github/workflows/ci.yml",
  fetchScript: "scripts/fetch-ffmpeg.js",
};

const root = new URL("../", import.meta.url);

const docs = {};

for (const path of Object.values(FILES)) {
  docs[path] = readFileSync(new URL(path, root), "utf8");
}

const icons = new Set(
  readdirSync(new URL("apps/desktop/icons/", root)).map(
    (name) => `icons/${name}`,
  ),
);

/** Edits one JSON document and hands it back as text, as the file would be written. */
function edit(path, change) {
  const doc = JSON.parse(docs[path]);
  change(doc);
  return JSON.stringify(doc, null, 2);
}

/** Replaces one piece of text, and says so when the text to change is not there. */
function replace(source, pattern, replacement) {
  if (!source.includes(pattern)) {
    return { error: `nothing in the document looks like ${pattern}` };
  }

  return source.replace(pattern, replacement);
}

/** Replaces every occurrence, and says so when the text to change is not there. */
function replaceEverywhere(source, pattern, replacement) {
  if (!source.includes(pattern)) {
    return { error: `nothing in the document looks like ${pattern}` };
  }

  return source.split(pattern).join(replacement);
}

/**
 * Changes one job only. The workflow holds two matrices that name the same three systems and two
 * jobs that install the same webview libraries, so a change applied to the first occurrence would
 * disturb the job before the one being tested and leave the one under test as it was.
 */
function inJob(source, id, change) {
  /* A checkout on Windows writes the file with CRLF; the job is found by its line, so the ending
     is normalized before the lines are compared. */
  const lines = source.replaceAll("\r\n", "\n").split("\n");
  const at = lines.findIndex((line) => line === `  ${id}:`);

  if (at === -1) {
    return { error: `the workflow has no ${id} job` };
  }

  let end = lines.length;

  for (let i = at + 1; i < lines.length; i += 1) {
    if (/^ {2}[A-Za-z0-9_-]+:/.test(lines[i])) {
      end = i;
      break;
    }
  }

  const changed = change(lines.slice(at, end).join("\n"));

  if (typeof changed === "object") {
    return changed;
  }

  return [
    ...lines.slice(0, at),
    ...changed.split("\n"),
    ...lines.slice(end),
  ].join("\n");
}

/** The bundle job. */
function inBundleJob(source, change) {
  return inJob(source, "bundle", change);
}

const checks = [];

// The known-good case, before anything is disturbed.
const good = check(docs, icons);

checks.push({
  claim:
    "the documents as they stand carry a bundle that reaches the three platforms and keeps them",
  ok:
    good.faults.length === 0 && good.facts.bundle?.runsFrom === "apps/desktop",
  saw: {
    bundle: good.facts.bundle,
    package: good.facts.package,
    workflow: good.facts.workflow,
    faults: good.faults,
  },
});

const cases = [
  {
    claim: "an entry with no bundle script is caught",
    mutated: {
      ...docs,
      [FILES.uiPackage]: edit(FILES.uiPackage, (doc) => {
        delete doc.scripts.bundle;
      }),
    },
    expect: /has no bundle script/,
  },
  {
    claim:
      "an entry that runs the CLI from the folder whose package.json holds it is caught",
    mutated: {
      ...docs,
      [FILES.uiPackage]: edit(FILES.uiPackage, (doc) => {
        doc.scripts.bundle = "tauri build";
      }),
    },
    expect: /does not hold apps\/desktop\/tauri\.conf\.json/,
  },
  {
    claim: "an entry that names the CLI without declaring it is caught",
    mutated: {
      ...docs,
      [FILES.uiPackage]: edit(FILES.uiPackage, (doc) => {
        delete doc.devDependencies["@tauri-apps/cli"];
      }),
    },
    expect: /@tauri-apps\/cli is not declared/,
  },
  {
    claim: "a configuration that does not bundle the application is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        doc.bundle.active = false;
      }),
    },
    expect: /does not bundle the application/,
  },
  {
    claim: "a package that names no publisher is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        delete doc.bundle.publisher;
      }),
    },
    expect: /names no publisher/,
  },
  {
    claim: "a category the bundler does not accept is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        doc.bundle.category = "Spacecraft";
      }),
    },
    expect: /does not accept/,
  },
  {
    claim: "an icon the bundle lists that is not on the disk is caught",
    mutated: docs,
    icons: new Set([...icons].filter((icon) => icon !== "icons/32x32.png")),
    expect: /lists icons\/32x32\.png, which is not on the disk/,
  },
  {
    claim: "a macOS bundle that lists no .icns is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        doc.bundle.icon = doc.bundle.icon.filter(
          (icon) => icon !== "icons/icon.icns",
        );
      }),
    },
    expect: /does not carry icons\/icon\.icns/,
  },
  {
    claim:
      "a platform packaging an installer another platform packages is caught",
    mutated: {
      ...docs,
      [FILES.linuxConfig]: edit(FILES.linuxConfig, (doc) => {
        doc.bundle.targets = ["deb", "msi"];
      }),
    },
    expect: /the Linux bundle packages \[deb, msi\], and Linux packages/,
  },
  {
    claim: "an updater that names no public key is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        delete doc.plugins.updater.pubkey;
      }),
    },
    expect: /names no public key/,
  },
  {
    claim: "an updater whose endpoint is not https is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        doc.plugins.updater.endpoints = ["http://example.invalid/latest.json"];
      }),
    },
    expect: /which is not https/,
  },
  {
    claim: "a configuration that signs every change is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        doc.bundle.createUpdaterArtifacts = true;
      }),
    },
    expect: /signs every build/,
  },
  {
    claim:
      "the tag document that turns the update artifacts on is caught when it does not",
    mutated: {
      ...docs,
      [FILES.updaterConfig]: edit(FILES.updaterConfig, (doc) => {
        doc.bundle.createUpdaterArtifacts = false;
      }),
    },
    expect: /does not turn the update artifacts on/,
  },
  {
    claim: "a bundle that carries no ffmpeg sidecar is caught",
    mutated: {
      ...docs,
      [FILES.config]: edit(FILES.config, (doc) => {
        delete doc.bundle.externalBin;
      }),
    },
    expect: /carries no binaries\/ffmpeg sidecar/,
  },
  {
    claim: "a fetch script that names no build for one platform is caught",
    mutated: {
      ...docs,
      [FILES.fetchScript]: replace(
        docs[FILES.fetchScript],
        'triple: "aarch64-apple-darwin"',
        'triple: "aarch64-apple-nowhere"',
      ),
    },
    expect: /does not name the aarch64-apple-darwin build/,
  },
  {
    claim:
      "a stage that builds the application without fetching the sidecar is caught",
    mutated: {
      ...docs,
      [FILES.desktopPackage]: edit(FILES.desktopPackage, (doc) => {
        doc.scripts.build = "npm --prefix ../ui run build";
      }),
    },
    expect: /the build stage does not fetch the sidecar/,
  },
  {
    claim:
      "a job that compiles the workspace without fetching the sidecar is caught",
    mutated: {
      ...docs,
      [FILES.workflow]: inJob(docs[FILES.workflow], "rust", (job) =>
        replaceEverywhere(
          job,
          "node scripts/fetch-ffmpeg.js",
          "node scripts/never.js",
        ),
      ),
    },
    expect: /the rust job does not fetch the ffmpeg sidecar/,
  },
  {
    claim:
      "the other job that compiles the workspace is read for the sidecar too",
    mutated: {
      ...docs,
      [FILES.workflow]: inJob(docs[FILES.workflow], "sitl", (job) =>
        replaceEverywhere(
          job,
          "node scripts/fetch-ffmpeg.js",
          "node scripts/never.js",
        ),
      ),
    },
    expect: /the sitl job does not fetch the ffmpeg sidecar/,
  },
  {
    claim: "a workflow that leaves one platform out is caught",
    mutated: {
      ...docs,
      [FILES.workflow]: inBundleJob(docs[FILES.workflow], (job) =>
        replace(
          job,
          "os: [ubuntu-latest, macos-latest, windows-latest]",
          "os: [ubuntu-latest, windows-latest]",
        ),
      ),
    },
    expect: /does not run on macos-latest/,
  },
  {
    claim: "a workflow that does not run the bundle entry is caught",
    mutated: {
      ...docs,
      [FILES.workflow]: inBundleJob(docs[FILES.workflow], (job) =>
        replaceEverywhere(job, "npm run bundle", "npm run launch"),
      ),
    },
    expect: /does not run the bundle script/,
  },
  {
    claim:
      "a workflow that keeps a folder the bundler does not write is caught",
    mutated: {
      ...docs,
      [FILES.workflow]: inBundleJob(docs[FILES.workflow], (job) =>
        replaceEverywhere(job, "target/release/bundle", "target/release/out"),
      ),
    },
    expect: /does not keep the installers from target\/release\/bundle/,
  },
  {
    claim: "a workflow that reads an empty upload as a pass is caught",
    mutated: {
      ...docs,
      [FILES.workflow]: inBundleJob(docs[FILES.workflow], (job) =>
        replaceEverywhere(
          job,
          "if-no-files-found: error",
          "if-no-files-found: warn",
        ),
      ),
    },
    expect: /empty upload is a failure/,
  },
  {
    claim:
      "a workflow that builds on Linux without the webview libraries is caught",
    mutated: {
      ...docs,
      [FILES.workflow]: inBundleJob(docs[FILES.workflow], (job) =>
        replace(job, "libwebkit2gtk-4.1-dev \\\n", ""),
      ),
    },
    expect: /does not install the Linux webview dependencies/,
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

  const result = check(entry.mutated, entry.icons ?? icons);
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
        "these prove each rule can fire and that the documents satisfy them as written. They do not build an installer, run npm or cargo, or show that a runner accepts the workflow; signing and notarisation are outside of what any of them reads.",
    },
    null,
    2,
  ),
);

process.exit(fails.length === 0 ? 0 : 1);
