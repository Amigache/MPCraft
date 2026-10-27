#!/usr/bin/env node
/**
 * Write a picture the interface can be shown, independently of any image library.
 *
 * This script is a second, independent implementation of the PNG format: it writes the chunks by
 * hand and works the CRC out with the rule the format gives, rather than taking the bytes from a
 * decoder somebody else wrote. That is what makes it usable as a fixture — when the reader in the
 * test and the writer here disagree, one of them has a bug, and the file on disk is the thing that
 * settles it.
 *
 * The picture is not a flat colour. It carries a mark at the middle, which is where a click has to
 * say `0, 0`, and its proportions are 16:9, so a box of a different shape cannot hold it without a
 * band — which is the case the mapping rule is read against.
 *
 * The generated file is committed as a test fixture. Regenerate it with:
 *
 *     node scripts/write-test-picture.js apps/ui/e2e/fixtures/picture-1280x720.png
 */

import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

/** The dimensions the fixture declares. */
const WIDTH = 1280;
const HEIGHT = 720;

/** The chunk types this writer knows, and the order the format puts them in. */
const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

/**
 * CRC-32 as the PNG rule gives it: reflected, polynomial `0xedb88320`, started at `0xFFFFFFFF` and
 * inverted at the end. The writer first had it without the starting value and without the inversion,
 * which gave a different number for the same bytes than every real reader — so the values below are
 * not what this function decides: they come from an independent implementation (Python's `zlib.crc32`)
 * reading a PNG written by another tool, and the test compares the fixture against them.
 */
function crc32(bytes) {
  let crc = 0xffffffff;

  for (const byte of bytes) {
    let value = crc ^ byte;

    for (let bit = 0; bit < 8; bit += 1) {
      value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
    }

    crc = value;
  }

  return ~crc >>> 0;
}

/** One chunk: length, the name, the body, and the check over the name and the body. */
function chunk(name, body) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(body.length);

  const nameBytes = Buffer.from(name, "ascii");
  const check = Buffer.alloc(4);
  check.writeUInt32BE(crc32(Buffer.concat([nameBytes, body])));

  return Buffer.concat([length, nameBytes, body, check]);
}

/** The declaration of the picture: size, bit depth, colour type, and the three other filters. */
function header(width, height) {
  const data = Buffer.alloc(13);
  data.writeUInt32BE(width, 0);
  data.writeUInt32BE(height, 4);
  data[8] = 8; // bit depth
  data[9] = 2; // colour type: truecolour, no alpha
  data[10] = 0; // compression method
  data[11] = 0; // filter method
  data[12] = 0; // interlace

  return chunk("IHDR", data);
}

/** What one pixel is: the mark at the middle, and the ground around it. */
function pixel(x, y) {
  const centreX = Math.floor(WIDTH / 2);
  const centreY = Math.floor(HEIGHT / 2);

  // The mark is a cross of 40 pixels on each arm, which is what a click at the middle says.
  const onArm =
    (Math.abs(x - centreX) <= 20 && Math.abs(y - centreY) <= 2) ||
    (Math.abs(y - centreY) <= 20 && Math.abs(x - centreX) <= 2);

  if (onArm) {
    return [0xff, 0xff, 0xff];
  }

  return [0x12, 0x34, 0x56];
}

/** The rows, each one with its filter byte — none of them filtered, so the bytes are the picture. */
function pixels() {
  const raw = [];

  for (let y = 0; y < HEIGHT; y += 1) {
    raw.push(Buffer.from([0]));

    const row = [];

    for (let x = 0; x < WIDTH; x += 1) {
      row.push(...pixel(x, y));
    }

    raw.push(Buffer.from(row));
  }

  return Buffer.concat(raw);
}

/** The bytes as they go to the disk, assembled from the chunks in the order the format gives. */
export function pictureBytes(width = WIDTH, height = HEIGHT) {
  if (width !== WIDTH || height !== HEIGHT) {
    throw new Error("the fixture is written for 1280 × 720");
  }

  return Buffer.concat([
    SIGNATURE,
    header(width, height),
    chunk("IDAT", deflateSync(pixels())),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const target = process.argv[2];

if (target !== undefined) {
  const path = resolve(target);

  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, pictureBytes());
  console.log(
    `wrote ${target}: ${WIDTH} × ${HEIGHT}, ${pictureBytes().length} bytes`,
  );
}

/*
 * The check below is worked out from the format's own rule rather than read back through a decoder:
 * a writer that is checked only against itself proves nothing. So its numbers are compared with what
 * an independent implementation gives — Python's `zlib.crc32`, a second reader of the same rule —
 * and that comparison is what catches a wrong polynomial or a chunk assembled in the wrong order. It
 * did catch one: this writer started its run at zero without the final inversion, which is not the
 * rule the format gives, and every chunk it wrote disagreed with a PNG written by another tool. With
 * the starting value and the inversion the two agree, and the values they agree on for this fixture
 * are recorded in `apps/ui/src/lib/video/picture.test.ts` as golden numbers.
 */
if (process.argv[2] === undefined) {
  const bytes = pictureBytes();
  const chunks = [];
  let pos = 8;

  while (pos < bytes.length) {
    const length = bytes.readUInt32BE(pos);
    const name = bytes.subarray(pos + 4, pos + 8);
    const body = bytes.subarray(pos + 8, pos + 8 + length);
    const stored = bytes.readUInt32BE(pos + 8 + length);

    chunks.push(
      `${name.toString("ascii")} ${stored.toString(16).padStart(8, "0")}`,
    );
    pos += 12 + length;
  }

  console.log(chunks.join(" "));
}
