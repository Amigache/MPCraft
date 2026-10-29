#!/usr/bin/env node
/**
 * The `ffmpeg` the installer carries, brought to the disk before the application is built.
 *
 * The relay that draws a camera stream reads `ffmpeg`, and a release cannot ask the machine it is
 * installed on to have one: some machines do, and a ground station that draws video only where
 * somebody installed a program is a ground station that does not draw video. So the binary travels
 * beside the executable as a Tauri sidecar, and this script is what puts it in the folder the
 * bundler reads.
 *
 * The build fetches it rather than the repository carrying it: three platforms of a static build are
 * some two hundred megabytes in the history of every clone, and a binary that is replaced by a newer
 * one is not something a diff can review. What the repository carries is the platform each build
 * comes from and the name it is written as, which is what can be read.
 *
 * Tauri expects a sidecar named `<name>-<target triple>` — `ffmpeg-x86_64-pc-windows-msvc.exe` on
 * Windows, `ffmpeg-aarch64-apple-darwin` on an Apple machine — in the folder `bundle > externalBin`
 * names, and it copies that file beside the compiled binary under the plain name `ffmpeg`, which is
 * where `crates/mpcraft-app/src/video.rs` looks for it. The triple is the one being built rather than
 * the one this machine would build by default, so `--triple` and `CARGO_BUILD_TARGET` are read before
 * anything is guessed.
 *
 * What this does not do: it does not say that the build it fetched reads every protocol the relay
 * accepts — it prints what the binary says about itself, and a scheme the build cannot read fails at
 * the moment a stream is drawn rather than here. It also does not verify a signature: the download is
 * over https from the release the platform is named in, and the version is recorded beside the file.
 */

import {
  createWriteStream,
  existsSync,
  mkdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { chmod } from "node:fs/promises";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";

const ROOT = fileURLToPath(new URL("../", import.meta.url));

/** Where the bundler reads the sidecars: `bundle > externalBin` names `binaries/ffmpeg`. */
const FOLDER = resolve(ROOT, "apps/desktop/binaries");

/** The release the static builds come from, and the version its own tag names. */
const RELEASE = "b6.0";
const SOURCE = `https://github.com/eugeneware/ffmpeg-static/releases/download/${RELEASE}`;

/**
 * What each build is given.
 *
 * The triple is Tauri's, because that is the name the sidecar has to carry; the asset is the
 * upstream name, and they are not the same word for the same machine. The GNU and the MSVC Windows
 * triples are two rows of the same build: a static `ffmpeg.exe` does not care which linker produced
 * the program that spawns it, and the machine this repository is built on has both toolchains.
 */
const TARGETS = [
  {
    triple: "x86_64-pc-windows-msvc",
    asset: "ffmpeg-win32-x64",
    extension: ".exe",
  },
  {
    triple: "x86_64-pc-windows-gnu",
    asset: "ffmpeg-win32-x64",
    extension: ".exe",
  },
  {
    triple: "x86_64-unknown-linux-gnu",
    asset: "ffmpeg-linux-x64",
    extension: "",
  },
  {
    triple: "aarch64-unknown-linux-gnu",
    asset: "ffmpeg-linux-arm64",
    extension: "",
  },
  {
    triple: "aarch64-apple-darwin",
    asset: "ffmpeg-darwin-arm64",
    extension: "",
  },
  {
    triple: "x86_64-apple-darwin",
    asset: "ffmpeg-darwin-x64",
    extension: "",
  },
];

/** The triple this machine builds without being told, which is what a plain `cargo build` uses. */
const DEFAULTS = {
  "win32 x64": "x86_64-pc-windows-msvc",
  "linux x64": "x86_64-unknown-linux-gnu",
  "linux arm64": "aarch64-unknown-linux-gnu",
  "darwin arm64": "aarch64-apple-darwin",
  "darwin x64": "x86_64-apple-darwin",
};

function say(message) {
  process.stdout.write(`${message}\n`);
}

/**
 * The triple being built.
 *
 * A build that names its target — `cargo build --target`, or a Tauri build for another machine — is
 * the one the sidecar has to match, because the bundler copies `ffmpeg-<that triple>`; only when
 * nothing names one is the machine's own default used.
 */
function wantedTriple() {
  const named = process.argv.slice(2);
  const at = named.indexOf("--triple");

  if (at !== -1 && named[at + 1] !== undefined) {
    return named[at + 1];
  }

  const fromEnvironment =
    process.env.CARGO_BUILD_TARGET ?? process.env.TAURI_ENV_TARGET_TRIPLE;

  if (fromEnvironment !== undefined && fromEnvironment !== "") {
    return fromEnvironment;
  }

  return DEFAULTS[`${process.platform} ${process.arch}`] ?? null;
}

/** The build that answers for one triple, when this script carries one. */
function machine(triple) {
  return TARGETS.find((target) => target.triple === triple) ?? null;
}

/** The file the bundler reads for this build. */
export function sidecarPath(triple = wantedTriple()) {
  const target = machine(triple);

  return target === null
    ? null
    : resolve(FOLDER, `ffmpeg-${target.triple}${target.extension}`);
}

/** What the binary says about itself, or nothing when it does not answer. */
function run(file, arguments_) {
  try {
    return execFileSync(file, ["-hide_banner", ...arguments_], {
      encoding: "utf8",
      timeout: 30_000,
    });
  } catch {
    return null;
  }
}

/** The first line of `-version`, which names the build. */
function version(file) {
  const said = run(file, ["-version"]);

  return said === null ? null : (said.split("\n")[0] ?? "").trim();
}

/** The protocols the binary says it reads. */
function protocols(file) {
  const said = run(file, ["-protocols"]);

  if (said === null) {
    return [];
  }

  return said
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "" && !line.endsWith(":"));
}

/**
 * The formats the binary says it reads and writes.
 *
 * `rtsp` is a demuxer rather than a protocol — it is the session that carries a stream over one of
 * the transports — and `mpjpeg` is the muxer the relay's own output is written in, so both are asked
 * for here rather than in the list of protocols.
 */
function formats(file) {
  const said = run(file, ["-formats"]);

  return said === null ? [] : said.split("\n").map((line) => line.trimEnd());
}

/** Downloads one asset to the path the bundler reads, following the release's redirect. */
async function download(asset, destination) {
  const address = `${SOURCE}/${asset}`;

  say(`fetching ${address}`);

  const answer = await fetch(address, { redirect: "follow" });

  if (!answer.ok || answer.body === null) {
    throw new Error(`${address} answered ${answer.status}`);
  }

  mkdirSync(dirname(destination), { recursive: true });

  const partial = `${destination}.part`;

  await pipeline(Readable.fromWeb(answer.body), createWriteStream(partial));

  const size = statSync(partial).size;

  if (size < 10 * 1024 * 1024) {
    rmSync(partial, { force: true });

    throw new Error(`${asset} answered ${size} bytes, which is not a build`);
  }

  // The rename is what makes a half-written file impossible to read as a binary: the bundler only
  // ever sees a name that was written whole.
  rmSync(destination, { force: true });
  renameSync(partial, destination);

  if (destination.endsWith(".exe")) {
    return size;
  }

  await chmod(destination, 0o755);

  return size;
}

async function main() {
  const arguments_ = process.argv.slice(2);
  const force = arguments_.includes("--force");
  const check = arguments_.includes("--check");
  const target = machine(wantedTriple());
  const destination = sidecarPath();

  if (target === null) {
    say(
      `this build (${wantedTriple() ?? `${process.platform} ${process.arch}`}) has no static ffmpeg named here, so the sidecar is not fetched`,
    );

    process.exit(check ? 1 : 0);
  }

  if (check) {
    say(destination);
    process.exit(existsSync(destination) ? 0 : 1);
  }

  const record = `${destination}.txt`;

  if (!force && existsSync(destination)) {
    const said = version(destination);

    if (said !== null) {
      say(`already here: ${destination}`);
      say(said);

      return;
    }

    // A file that does not answer is not the build the installer would carry, whatever its name
    // says, so it is fetched again rather than shipped.
    say(`${destination} does not answer as ffmpeg, so it is fetched again`);
    rmSync(destination, { force: true });
  }

  const size = await download(target.asset, destination);
  const said = version(destination);

  if (said === null) {
    rmSync(destination, { force: true });

    throw new Error(
      `${target.asset} was written but does not answer as ffmpeg`,
    );
  }

  writeFileSync(
    record,
    `${said}\n${SOURCE}/${target.asset}\n${size} bytes\n`,
    "utf8",
  );

  say(`wrote ${destination} (${(size / (1024 * 1024)).toFixed(1)} MiB)`);
  say(said);

  // The relay accepts more schemes than one build of ffmpeg may carry — srt needs a library that is
  // not in every static build — so what the binary reads is printed rather than assumed. `rtsp` is
  // a demuxer and `mpjpeg` is the muxer the relay's own output is written in, so the three are
  // asked for in the two tables that carry them.
  const read = protocols(destination);
  const wanted = ["rtmp", "udp", "tcp", "http", "https", "srt"];
  const carried = wanted.filter((scheme) => read.includes(scheme));
  const absent = wanted.filter((scheme) => !read.includes(scheme));

  const table = formats(destination).flatMap((line) => {
    const found = /^ ([D ])([E ]) +(\S+)/.exec(line);

    return found === null
      ? []
      : [{ reads: found[1] === "D", writes: found[2] === "E", name: found[3] }];
  });

  const readsRtsp = table.some((entry) => entry.name === "rtsp" && entry.reads);
  const writesMjpeg = table.some(
    (entry) => entry.name === "mpjpeg" && entry.writes,
  );

  if (!readsRtsp) {
    absent.push("rtsp");
  }

  if (!writesMjpeg) {
    absent.push("mpjpeg");
  }

  say(`protocols read: ${carried.join(", ")}`);
  say(
    `rtsp demuxer: ${readsRtsp ? "yes" : "no"}, mpjpeg muxer: ${writesMjpeg ? "yes" : "no"}`,
  );

  if (absent.length > 0) {
    say(
      `not carried by this build: ${absent.join(", ")} — the relay accepts them and the build refuses them when a stream is drawn`,
    );
  }
}

if (
  process.argv[1] !== undefined &&
  process.argv[1].includes("fetch-ffmpeg.js")
) {
  await main();
}
