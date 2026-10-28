#!/usr/bin/env node
/**
 * The document the updater reads, built from what the bundler wrote.
 *
 * Tauri publishes an update as three things: the archive of the new version for one platform, the
 * signature of that archive, and a `latest.json` that names them. The signature is what makes the
 * update safe to install: the application carries the public key and refuses an archive that was
 * not signed by its private half.
 *
 * The document is assembled in two steps because the three platforms are built by three jobs that
 * never see each other:
 *
 *     updater-manifest.js discover --bundle target/release/bundle --platform windows-x86_64 \
 *         --version 0.1.0 --tag v0.1.0 --repository owner/repo --out latest-windows.json
 *
 *     updater-manifest.js merge --out latest.json latest-*.json
 *
 * `discover` refuses to write a fragment when the archive or its signature is not there, because a
 * manifest that names an archive nobody signed is an update that cannot be installed. `merge`
 * refuses fragments that disagree about the version or that name the same platform twice.
 *
 * What this cannot do is said rather than assumed: it does not check the signature, which is the
 * application's job with the public key, and it does not reach the release, so a URL it writes is
 * one the release has to answer.
 */

import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";

/** The platforms the updater names, as the protocol spells them. */
const PLATFORM = /^(windows|darwin|linux)-(x86_64|aarch64|i686|armv7)$/;

/** Which archive the updater of each system installs. */
const ARCHIVE = {
  windows: ["-setup.exe", ".msi"],
  darwin: [".app.tar.gz"],
  linux: [".AppImage"],
};

/** One platform of the document, with the signature and the address of its archive. */
export function fragment({ platform, version, signature, url }) {
  if (!PLATFORM.test(platform)) {
    throw new Error(`${platform} is not a platform the updater names`);
  }

  const clean = version.replace(/^v/, "");

  if (!/^\d+\.\d+\.\d+/.test(clean)) {
    throw new Error(`${version} is not a version`);
  }

  if (String(signature).trim() === "") {
    throw new Error(`${platform} has no signature`);
  }

  if (!/^https:\/\//.test(url)) {
    throw new Error(`${platform} names an address that is not https`);
  }

  return {
    version: clean,
    platforms: {
      [platform]: { signature: String(signature).trim(), url },
    },
  };
}

/** The fragments as one document, or an error when they disagree. */
export function merge(fragments) {
  if (fragments.length === 0) {
    throw new Error("there is no fragment to merge");
  }

  const version = String(fragments[0].version).replace(/^v/, "");
  const platforms = {};

  for (const one of fragments) {
    if (String(one.version).replace(/^v/, "") !== version) {
      throw new Error(
        `the fragments disagree about the version: ${version} and ${one.version}`,
      );
    }

    for (const [key, entry] of Object.entries(one.platforms ?? {})) {
      if (!PLATFORM.test(key)) {
        throw new Error(`${key} is not a platform the updater names`);
      }

      if (platforms[key] !== undefined) {
        throw new Error(`two fragments name ${key}`);
      }

      const signature = String(entry.signature ?? "").trim();
      const url = String(entry.url ?? "");

      if (signature === "" || !/^https:\/\//.test(url)) {
        throw new Error(`${key} is not a complete entry`);
      }

      platforms[key] = { signature, url };
    }
  }

  const sorted = {};

  for (const key of Object.keys(platforms).sort()) {
    sorted[key] = platforms[key];
  }

  return { version, platforms: sorted };
}

/** Finds the archive the updater of a platform installs, and builds its fragment. */
export function discover(bundle, { platform, version, tag, repository }) {
  const system = platform.split("-")[0];
  const suffixes = ARCHIVE[system];

  if (suffixes === undefined) {
    throw new Error(`${platform} is not a platform the updater names`);
  }

  const files = walk(bundle);
  let archive = null;

  for (const suffix of suffixes) {
    archive = files.find((file) => file.endsWith(suffix)) ?? archive;

    if (archive !== null) {
      break;
    }
  }

  if (archive === null) {
    throw new Error(
      `the bundle holds no ${suffixes.join(" or ")} for ${platform}`,
    );
  }

  const signatureFile = `${archive}.sig`;

  if (!existsSync(signatureFile)) {
    throw new Error(`${signatureFile} is not there, so nothing signed it`);
  }

  const name = archive.split(/[\\/]/).pop();
  const url = `https://github.com/${repository}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;

  return fragment({
    platform,
    version,
    signature: readFileSync(signatureFile, "utf8"),
    url,
  });
}

/** Every file under a folder, at any depth. */
function walk(folder) {
  const found = [];

  for (const name of readdirSync(folder)) {
    const full = join(folder, name);

    if (statSync(full).isDirectory()) {
      found.push(...walk(full));
    } else {
      found.push(full);
    }
  }

  return found;
}

/** The options of a command line, and what is left when they are taken out. */
function parse(args) {
  const options = {};
  const positional = [];

  for (let index = 0; index < args.length; index += 1) {
    if (args[index].startsWith("--")) {
      options[args[index].slice(2)] = args[index + 1];
      index += 1;
    } else {
      positional.push(args[index]);
    }
  }

  return { options, positional };
}

if (process.argv[1]?.includes("updater-manifest.js")) {
  const [mode, ...rest] = process.argv.slice(2);
  const { options, positional } = parse(rest);

  try {
    if (mode === "discover") {
      const document = discover(options.bundle, {
        platform: options.platform,
        version: options.version,
        tag: options.tag,
        repository: options.repository,
      });

      writeFileSync(
        options.out,
        `${JSON.stringify(document, null, 2)}\n`,
        "utf8",
      );
    } else if (mode === "merge") {
      const fragments = positional.map((file) =>
        JSON.parse(readFileSync(file, "utf8")),
      );

      writeFileSync(
        options.out,
        `${JSON.stringify(merge(fragments), null, 2)}\n`,
        "utf8",
      );
    } else {
      throw new Error(
        "the mode is discover or merge, as updater-manifest.js <mode>",
      );
    }
  } catch (error) {
    console.error(`updater-manifest: ${error.message}`);
    process.exit(1);
  }
}
