#!/usr/bin/env node
/**
 * The launch, read as the documents say it is.
 *
 * Nothing in the repository named a way to open the application, and what the CLI does with the folder
 * it is pointed at is not something a script can guess. The two facts that made this gate necessary
 * were measured rather than assumed:
 *
 *   - The CLI takes the project from the folder it runs in, or a subfolder of it. Run from `apps/ui` it
 *     prints an empty App section and `tauri dev` panics at
 *     `crates/tauri-cli/src/helpers/app_paths.rs:140` — "Couldn't recognize the current folder as a
 *     Tauri project". Run from `apps/desktop`, where `tauri.conf.json` is, the same project resolves
 *     and the CLI prints `frontendDist` and `devUrl`.
 *   - The command the CLI runs before the build is run by npm from wherever npm reads its package.json.
 *     With no package.json between the app folder and the repo root npm had nothing to read and the
 *     command failed with ENOENT on `apps/package.json`; with one next to `tauri.conf.json` the CLI ran
 *     its command from that folder, which is what a probe printing its own cwd showed. A script run
 *     through `--prefix` sees the prefix folder as its cwd, so a delegation reaches the config of the
 *     folder it names.
 *
 * So this reads five documents and asserts the chain that has to hold for the launch to work: the entry
 * names the CLI as something npm can find, the folder it moves to is the folder holding the
 * configuration, every command the CLI runs names a script that exists in the package.json npm reads
 * from where it runs, the port the CLI expects is the port the dev server binds and the port the browser
 * run connects to, and the built frontend is written where the configuration looks for it.
 *
 * What this buys, and what it does not. It cannot open a window, cannot run cargo, and cannot tell
 * whether an install is present in either folder; it reads the two TypeScript configurations as text, so
 * a port set through a variable or an environment override is not seen. The rule the CLI uses to decide
 * where npm reads from is recorded above as one measurement, not as something this gate can re-check.
 */

import { existsSync, readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

/** Which document is which, by the path the repository gives it. */
const FILES = {
  uiPackage: "apps/ui/package.json",
  desktopPackage: "apps/desktop/package.json",
  desktopConfig: "apps/desktop/tauri.conf.json",
  viteConfig: "apps/ui/vite.config.ts",
  playwrightConfig: "apps/ui/playwright.config.ts",
};

const ENTRY = "launch";
const CLI = "@tauri-apps/cli";

function parseJson(text) {
  try {
    return JSON.parse(text);
  } catch {
    return null;
  }
}

/** An npm invocation inside a script or a command: `npm run <script>`, with an optional prefix. */
function npmRun(command) {
  const match = /npm\s+(?:--prefix\s+(\S+)\s+)?run\s+([A-Za-z0-9:._-]+)/.exec(
    command ?? "",
  );

  if (match === null) {
    return null;
  }

  return { prefix: match[1] ?? null, script: match[2] };
}

/**
 * Follows one command through the package.json files npm reads from, in the order they are read. A chain
 * that ends at a script nobody wrote, or at a folder npm has nothing to read from, is the failure the
 * launch had on this machine.
 */
function trace(command, start, packages, faults, seen) {
  const hops = [];
  let where = start;

  for (let depth = 0; depth < 8; depth += 1) {
    const call = npmRun(command);

    if (call === null) {
      faults.push(
        `${command} is not an npm run, so there is no script to name`,
      );
      break;
    }

    const path = resolve(where, call.prefix ?? ".");
    const doc = packages[path];

    if (doc === undefined || doc === null) {
      faults.push(
        `npm would read no package.json from ${relative(path)}, which is where \`${command}\` runs`,
      );
      break;
    }

    const scripts = doc.scripts ?? {};

    if (!(call.script in scripts)) {
      faults.push(
        `\`${command}\` names ${call.script}, which is not a script of ${relative(path)}/package.json`,
      );
      break;
    }

    hops.push({ readFrom: relative(path), script: call.script });

    const key = `${call.script}@${relative(path)}`;

    if (seen.has(key)) {
      faults.push(
        `the chain through ${call.script} does not end: it comes back to itself`,
      );
      break;
    }

    seen.add(key);

    // A script run through --prefix sees the prefix folder as its cwd, so the next read happens there.
    where = path;
    command = scripts[call.script];

    if (npmRun(command) === null) {
      break;
    }
  }

  return hops;
}

function relative(path) {
  return path.slice(ROOT.length).replaceAll("\\", "/").replace(/^\//, "");
}

export function check(texts) {
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

  const packages = {};

  for (const key of ["uiPackage", "desktopPackage"]) {
    const path = FILES[key];
    const doc = texts[path] === null ? null : parseJson(texts[path]);

    // npm reads from a folder, so the package is kept under the folder it is read from.
    packages[dirname(resolve(ROOT, path))] = doc;

    if (doc === null && texts[path] !== null) {
      faults.push(`${path} is not a document that can be read`);
    }
  }

  const configText = texts[FILES.desktopConfig];
  const config =
    configText === null ? null : parseJson(configText, FILES.desktopConfig);

  if (config === null && configText !== null) {
    faults.push(`${FILES.desktopConfig} is not a document that can be read`);
  }

  if (faults.length > 0) {
    return { faults, facts };
  }

  const uiFolder = resolve(ROOT, "apps/ui");
  const appFolder = resolve(ROOT, "apps/desktop");

  /* The entry, and the folder it has to move to. */
  const entry = (packages[uiFolder]?.scripts ?? {})[ENTRY];

  if (entry === undefined) {
    faults.push(`${FILES.uiPackage} has no ${ENTRY} script`);
  } else {
    /* An entry may move to another folder; where it does not, it runs from the folder whose package.json
       holds it, and that is the folder the CLI takes the project from. */
    const call = /^(?:cd\s+(\S+)\s+&&\s+)?([A-Za-z0-9._-]+)\s+(\S+)$/.exec(
      entry.trim(),
    );

    facts.entry = { writtenIn: relative(uiFolder), command: entry };

    if (call === null) {
      faults.push(`the launch entry (${entry}) does not name a command`);
    } else {
      const [, target, binary, subcommand] = call;
      const runsFrom =
        target === undefined ? uiFolder : resolve(uiFolder, target);

      facts.entry.runsFrom = relative(runsFrom);

      // The CLI is found through the package.json the entry is written in, so it has to be declared there.
      const declared = {
        ...(packages[uiFolder].dependencies ?? {}),
        ...(packages[uiFolder].devDependencies ?? {}),
      };

      if (!(CLI in declared)) {
        faults.push(
          `the launch entry runs ${binary}, but ${CLI} is not declared in ${FILES.uiPackage}`,
        );
      }

      if (binary !== "tauri" || subcommand !== "dev") {
        faults.push(
          `the launch entry runs \`${binary} ${subcommand}\`, which is not the CLI's dev`,
        );
      }

      // The CLI takes the project from the folder it runs in or a subfolder of it.
      if (runsFrom !== appFolder) {
        faults.push(
          `the launch entry runs the CLI from ${relative(runsFrom)}, which does not hold ${FILES.desktopConfig}`,
        );
      }
    }
  }

  /* What npm reads from where the CLI runs its commands. */
  const build = config.build ?? {};

  for (const key of ["beforeDevCommand", "beforeBuildCommand"]) {
    const command = build[key];

    if (command === undefined) {
      faults.push(`the configuration has no ${key}`);
      continue;
    }

    facts[key] = {
      command,
      chain: trace(command, appFolder, packages, faults, new Set()),
    };
  }

  /* The port, on the three places that have to say the same one. */
  const devUrl = new URL(build.devUrl ?? "http://localhost:0");
  const vitePort = /server:\s*\{[^}]*?port:\s*(\d+)/s.exec(
    texts[FILES.viteConfig],
  )?.[1];
  const basePort = /baseURL:\s*"http:\/\/localhost:(\d+)"/.exec(
    texts[FILES.playwrightConfig],
  )?.[1];

  facts.ports = {
    config: devUrl.port,
    vite: vitePort ?? null,
    playwright: basePort ?? null,
  };

  if (vitePort === undefined) {
    faults.push(`${FILES.viteConfig} does not name a server port`);
  } else if (devUrl.port !== vitePort) {
    faults.push(
      `the configuration expects port ${devUrl.port} and the dev server binds ${vitePort}`,
    );
  }

  if (!/strictPort:\s*true/.test(texts[FILES.viteConfig])) {
    faults.push(`${FILES.viteConfig} does not insist on the port it names`);
  }

  if (basePort === undefined) {
    faults.push(`${FILES.playwrightConfig} does not name a port to connect to`);
  } else if (basePort !== vitePort) {
    faults.push(
      `the browser run connects to port ${basePort}, which is not the port the dev server binds (${vitePort})`,
    );
  }

  /* The dev server the browser run starts has to be one a package.json knows. */
  const webServer =
    /webServer:\s*\{([^}]*)\}/s.exec(texts[FILES.playwrightConfig])?.[1] ?? "";
  const webCall = npmRun(/command:\s*"([^"]+)"/.exec(webServer)?.[1] ?? "");

  if (webCall === null) {
    faults.push(
      `${FILES.playwrightConfig} starts no dev server through an npm script`,
    );
  } else if (!(webCall.script in (packages[uiFolder]?.scripts ?? {}))) {
    faults.push(
      `the browser run starts ${webCall.script}, which is not a script of ${FILES.uiPackage}`,
    );
  } else {
    facts.webServer = { script: webCall.script };
  }

  /* Where the build writes, against where the configuration looks. */
  const outDir =
    /build:\s*\{[^}]*?outDir:\s*"([^"]+)"/s.exec(
      texts[FILES.viteConfig],
    )?.[1] ?? "dist";
  const written = resolve(uiFolder, outDir);
  const looked = resolve(appFolder, build.frontendDist ?? "");

  facts.dist = { written: relative(written), looked: relative(looked) };

  if (written !== looked) {
    faults.push(
      `the configuration looks for the frontend at ${relative(looked)} and the build writes to ${relative(written)}`,
    );
  }

  return { faults, facts };
}

if (
  process.argv[1] !== undefined &&
  process.argv[1].includes("check-launch.js")
) {
  const texts = {};

  for (const path of Object.values(FILES)) {
    const file = resolve(ROOT, path);

    texts[path] = existsSync(file) ? readFileSync(file, "utf8") : null;
  }

  const result = check(texts);

  console.log(
    JSON.stringify(
      {
        facts: result.facts,
        verdict: result.faults.length === 0 ? "PASS" : "FAIL",
        faults: result.faults,
        limit:
          "this reads the documents as they are written. It cannot open a window, cannot run cargo, cannot tell whether an install is present in either folder, and cannot see a port set through a variable or an environment override.",
      },
      null,
      2,
    ),
  );

  process.exit(result.faults.length === 0 ? 0 : 1);
}
