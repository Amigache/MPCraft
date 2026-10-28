#!/usr/bin/env node
/**
 * The packaging, read as the documents say it is.
 *
 * An installer is written by a chain that no gate can run cheaply on three platforms: an entry
 * that moves to the folder holding `tauri.conf.json`, the metadata the bundler writes into the
 * package, an icon for each platform the package is built on, a target list for each of the three,
 * and a workflow that builds them where each platform builds them. This reads those documents and
 * asserts the chain: the entry names the CLI as something npm can find and runs it from the folder
 * that holds the configuration, the bundle is active and carries the metadata a package shows, every
 * icon it lists is on the disk, each platform packages the installers that platform has, and the
 * workflow runs the entry on all three and keeps what it wrote.
 *
 * What this buys, and what it does not. It cannot build an installer, so it cannot say that one
 * exists; it reads the workflow as text, so it cannot say that a runner accepts it; and it says
 * nothing about signing or notarisation, which is row 10.9 and needs a certificate only the operator
 * holds.
 */

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

/** Which document is which, by the path the repository gives it. */
const FILES = {
  uiPackage: "apps/ui/package.json",
  config: "apps/desktop/tauri.conf.json",
  windowsConfig: "apps/desktop/tauri.windows.conf.json",
  linuxConfig: "apps/desktop/tauri.linux.conf.json",
  macosConfig: "apps/desktop/tauri.macos.conf.json",
  updaterConfig: "apps/desktop/tauri.updater.conf.json",
  workflow: ".github/workflows/ci.yml",
};

const CLI = "@tauri-apps/cli";

/** The categories the bundler accepts, read from its own configuration type. */
const CATEGORIES = [
  "Business",
  "DeveloperTool",
  "Education",
  "Entertainment",
  "Finance",
  "Game",
  "ActionGame",
  "AdventureGame",
  "ArcadeGame",
  "BoardGame",
  "CardGame",
  "CasinoGame",
  "DiceGame",
  "EducationalGame",
  "FamilyGame",
  "KidsGame",
  "MusicGame",
  "PuzzleGame",
  "RacingGame",
  "RolePlayingGame",
  "SimulationGame",
  "SportsGame",
  "StrategyGame",
  "TriviaGame",
  "WordGame",
  "GraphicsAndDesign",
  "HealthcareAndFitness",
  "Lifestyle",
  "Medical",
  "Music",
  "News",
  "Photography",
  "Productivity",
  "Reference",
  "SocialNetworking",
  "Sports",
  "Travel",
  "Utility",
  "Video",
  "Weather",
];

/** The icons the bundler needs for the three platforms it packages. */
const REQUIRED_ICONS = [
  "icons/32x32.png",
  "icons/128x128.png",
  "icons/128x128@2x.png",
  "icons/icon.icns",
  "icons/icon.ico",
];

/** What each platform packages, and nothing another platform packages. */
const PLATFORM_TARGETS = {
  windowsConfig: { platform: "Windows", targets: ["msi", "nsis"] },
  linuxConfig: { platform: "Linux", targets: ["appimage", "deb"] },
  macosConfig: { platform: "macOS", targets: ["app", "dmg"] },
};

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

function relative(path) {
  return path.slice(ROOT.length).replaceAll("\\", "/").replace(/^\//, "");
}

/** The block of one job, from its line to the line where the next job begins. */
function jobBlock(text, id) {
  /* A checkout on Windows writes the file with CRLF, and a line that still carries the carriage
     return is not the line the job is written on. The reading is of the text, so the ending is
     normalized before the lines are compared. */
  const lines = text.replaceAll("\r\n", "\n").split("\n");
  const at = lines.findIndex((line) => line === `  ${id}:`);

  if (at === -1) {
    return null;
  }

  const block = [];

  for (let i = at + 1; i < lines.length; i += 1) {
    if (/^ {2}[A-Za-z0-9_-]+:/.test(lines[i])) {
      break;
    }

    block.push(lines[i]);
  }

  return block.join("\n");
}

export function check(texts, icons) {
  const faults = [];
  const facts = {};

  for (const path of Object.values(FILES)) {
    if (texts[path] === undefined) {
      faults.push(`${path} is not there to be read`);
    }
  }

  if (faults.length > 0) {
    return { faults, facts };
  }

  const documents = {};

  for (const [key, path] of Object.entries(FILES)) {
    if (key === "workflow") {
      continue;
    }

    const text = texts[path];

    if (text === null) {
      documents[key] = null;
      faults.push(`${path} is not there to be read`);
      continue;
    }

    documents[key] = parseJson(text);

    if (documents[key] === null) {
      faults.push(`${path} is not a document that can be read`);
    }
  }

  if (faults.length > 0) {
    return { faults, facts };
  }

  const uiFolder = resolve(ROOT, "apps/ui");
  const appFolder = resolve(ROOT, "apps/desktop");
  const ui = documents.uiPackage;
  const config = documents.config;

  /* The entry, and the folder it has to move to. */
  const entry = (ui.scripts ?? {}).bundle;

  if (entry === undefined) {
    faults.push(`${FILES.uiPackage} has no bundle script`);
  } else {
    const call = /^(?:cd\s+(\S+)\s+&&\s+)?([A-Za-z0-9._-]+)\s+(\S+)$/.exec(
      entry.trim(),
    );

    facts.bundle = { command: entry };

    if (call === null) {
      faults.push(`the bundle entry (${entry}) does not name a command`);
    } else {
      const [, target, binary, subcommand] = call;
      const runsFrom =
        target === undefined ? uiFolder : resolve(uiFolder, target);

      facts.bundle.runsFrom = relative(runsFrom);

      const declared = {
        ...(ui.dependencies ?? {}),
        ...(ui.devDependencies ?? {}),
      };

      if (!(CLI in declared)) {
        faults.push(
          `the bundle entry runs ${binary}, but ${CLI} is not declared in ${FILES.uiPackage}`,
        );
      }

      if (binary !== "tauri" || subcommand !== "build") {
        faults.push(
          `the bundle entry runs \`${binary} ${subcommand}\`, which is not the CLI's build`,
        );
      }

      if (runsFrom !== appFolder) {
        faults.push(
          `the bundle entry runs the CLI from ${relative(runsFrom)}, which does not hold ${FILES.config}`,
        );
      }
    }
  }

  /* The bundle itself: on, with the metadata a package shows. */
  const bundle = config.bundle ?? {};

  facts.package = {
    publisher: bundle.publisher ?? null,
    category: bundle.category ?? null,
    targets: bundle.targets ?? null,
  };

  if (bundle.active !== true) {
    faults.push(`the configuration does not bundle the application`);
  }

  if (bundle.targets === undefined) {
    faults.push(`the bundle names no target`);
  }

  for (const [key, label] of [
    ["publisher", "publisher"],
    ["copyright", "copyright"],
    ["shortDescription", "short description"],
    ["longDescription", "long description"],
    ["license", "license"],
  ]) {
    const said = bundle[key];

    if (typeof said !== "string" || said.trim() === "") {
      faults.push(`the bundle names no ${label}`);
    }
  }

  if (!/^https:\/\//.test(bundle.homepage ?? "")) {
    faults.push(`the bundle names no https home page`);
  }

  if (!CATEGORIES.includes(bundle.category)) {
    faults.push(
      `the bundle names category ${bundle.category ?? "nothing"}, which the bundler does not accept`,
    );
  }

  if (!/^\d+\.\d+$/.test(bundle.macOS?.minimumSystemVersion ?? "")) {
    faults.push(`the bundle names no macOS system version`);
  }

  /* Every icon the bundle lists has to be on the disk, and the ones a platform installs have to be
     listed at all — an .icns that nobody lists is a macOS bundle with no icon. */
  const listed = Array.isArray(bundle.icon) ? bundle.icon : [];

  facts.icons = listed;

  for (const wanted of REQUIRED_ICONS) {
    if (!listed.includes(wanted)) {
      faults.push(`the bundle does not carry ${wanted}`);
    }
  }

  for (const icon of listed) {
    if (!icons.has(icon)) {
      faults.push(`the bundle lists ${icon}, which is not on the disk`);
    }
  }

  /* Each platform packages its own installers, and says so in its own document. */
  for (const [key, { platform, targets }] of Object.entries(PLATFORM_TARGETS)) {
    const doc = documents[key];
    const said = doc?.bundle?.targets;

    if (!Array.isArray(said)) {
      faults.push(`${FILES[key]} does not name the ${platform} targets`);
      continue;
    }

    const shown = [...said].sort().join(", ");
    const wanted = [...targets].sort().join(", ");

    if (shown !== wanted) {
      faults.push(
        `the ${platform} bundle packages [${shown}], and ${platform} packages [${wanted}]`,
      );
    }
  }

  /* The update: the public key the application checks an archive with, and the two documents that
     turn the update artifacts on for a tag and leave them off for a change. */
  const updater = config.plugins?.updater;

  if (updater !== undefined) {
    if (typeof updater.pubkey !== "string" || updater.pubkey.trim() === "") {
      faults.push(`the updater names no public key`);
    }

    const endpoints = Array.isArray(updater.endpoints) ? updater.endpoints : [];

    if (endpoints.length === 0) {
      faults.push(`the updater names no endpoint`);
    }

    for (const endpoint of endpoints) {
      if (!/^https:\/\//.test(endpoint)) {
        faults.push(`the updater names ${endpoint}, which is not https`);
      }
    }
  }

  if (bundle.createUpdaterArtifacts === true) {
    faults.push(
      `the configuration signs every build, and a change that is not a tag has no key`,
    );
  }

  if (documents.updaterConfig?.bundle?.createUpdaterArtifacts !== true) {
    faults.push(`${FILES.updaterConfig} does not turn the update artifacts on`);
  }

  /* The workflow: the entry run on all three, and what it wrote kept. */
  const job = jobBlock(texts[FILES.workflow], "bundle");

  if (job === null) {
    faults.push(`${FILES.workflow} has no bundle job`);
  } else {
    const os = /os:\s*\[([^\]]*)\]/.exec(job)?.[1] ?? "";

    for (const wanted of ["ubuntu-latest", "macos-latest", "windows-latest"]) {
      if (!os.includes(wanted)) {
        faults.push(`the bundle job does not run on ${wanted}`);
      }
    }

    if (!/npm run bundle/.test(job)) {
      faults.push(`the bundle job does not run the bundle script`);
    }

    if (!/npm ci/.test(job)) {
      faults.push(`the bundle job does not install the frontend dependencies`);
    }

    if (!/libwebkit2gtk-4\.1-dev/.test(job)) {
      faults.push(
        `the bundle job does not install the Linux webview dependencies`,
      );
    }

    if (!/if-no-files-found:\s*error/.test(job)) {
      faults.push(
        `the bundle job does not say that an empty upload is a failure`,
      );
    }

    if (!/target\/release\/bundle/.test(job)) {
      faults.push(
        `the bundle job does not keep the installers from target/release/bundle`,
      );
    }

    facts.workflow = { os, runs: /npm run bundle/.test(job) };
  }

  return { faults, facts };
}

if (
  process.argv[1] !== undefined &&
  process.argv[1].includes("check-package.js")
) {
  const texts = {};

  for (const path of Object.values(FILES)) {
    const file = resolve(ROOT, path);

    texts[path] = existsSync(file) ? readFileSync(file, "utf8") : null;
  }

  const iconFolder = resolve(ROOT, "apps/desktop/icons");
  const icons = new Set(
    existsSync(iconFolder)
      ? readdirSync(iconFolder).map((name) => `icons/${name}`)
      : [],
  );

  const result = check(texts, icons);

  console.log(
    JSON.stringify(
      {
        facts: result.facts,
        verdict: result.faults.length === 0 ? "PASS" : "FAIL",
        faults: result.faults,
        limit:
          "this reads the documents as they are written. It does not run cargo or npm, does not build an installer, and cannot say that a runner accepts the workflow; it says nothing about signing or notarisation, which needs a certificate only the operator holds.",
      },
      null,
      2,
    ),
  );

  process.exit(result.faults.length === 0 ? 0 : 1);
}
