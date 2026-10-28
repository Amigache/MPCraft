#!/usr/bin/env node
/**
 * The golden numbers of the DFU format, worked out again by a second implementation of the rule.
 *
 * The literals in `crates/mpcraft-app/tests/dfu.rs` say what a reading of the format gives for certain bytes.
 * A literal that is read only against the module that carries it says nothing: the module could have written
 * the number it likes. So this gate builds its own table from the polynomial — not from a decoder — and works
 * the same bytes out again, which is what says the literals did not come from the module being checked.
 *
 * Two readings are carried, and the two are not the same rule:
 *
 *     the published one — the reflected table from `crc32_table` in dfu-util's `src/dfu_file.c`, starting at
 *     0xFFFFFFFF, with no final inversion — is what pydfu, betaflight's `dfuse-pack.py`, tinyusb's fomu `dfu.py`
 *     and dfu-util all write, and it is the side this project carries;
 *     the wording of the specification — a table built with a polynomial that is not reflected (0x04C11DB7),
 *     starting at 0xFFFFFFFF and inverted at the end — gives a different number for the same bytes.
 *
 * The disagreement is kept rather than closed: for every sample the two numbers are different, so a copy that
 * writes them as agreeing is a finding. What the gate also reads is the writer as a document — the shapes the
 * format gives for the five fields — because the number is worked out over bytes the writer puts together, and
 * a writer that counts the target size field without its prefix says one thing and builds another.
 *
 * The third reading is the pair of names the bus half carries, and the file the operator runs with elevation
 * (`scripts/dfu-winusb.inf`), read as text. A row answers under whatever name the install writes on the device key,
 * while this binding reads the interface list through the class the element reports, so the two have to name the
 * same string or the row is there and the program still refuses; that is why the file is compared with the names
 * the crate carries rather than with a name written here. The same reason makes the pair 0xFE and 1 a separate fact:
 * pydfu decides an interface by that pair and this binding decides by the service it read, so a copy that writes the
 * two as one rule closes something that is not closed.
 */

import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const POLYNOMIALS = { published: 0xedb88320, specWorded: 0x04c11db7 };

/** The table built from the polynomial, by the rule each of the two readings shifts with. */
function table(poly) {
  const values = [];

  for (let byte = 0; byte < 256; byte += 1) {
    let value = byte;

    for (let bit = 0; bit < 8; bit += 1) {
      value =
        poly === POLYNOMIALS.published
          ? value & 1
            ? ((value >>> 1) ^ poly) >>> 0
            : (value >>> 1) >>> 0
          : value & 0x80000000
            ? (((value << 1) >>> 0) ^ poly) >>> 0
            : (value << 1) >>> 0;
    }

    values.push(value >>> 0);
  }

  return values;
}

/** Both readings start at 0xFFFFFFFF; only one of them inverts at the end. */
function accumulate(bytes, poly, invertAtTheEnd) {
  const values = table(poly);
  let crc = 0xffffffff;

  for (const byte of bytes) {
    crc =
      poly === POLYNOMIALS.published
        ? (values[(crc ^ byte) & 0xff] ^ (crc >>> 8)) >>> 0
        : (values[((crc >>> 24) ^ byte) & 0xff] ^ ((crc << 8) >>> 0)) >>> 0;
  }

  return (invertAtTheEnd ? (crc ^ 0xffffffff) >>> 0 : crc) >>> 0;
}

/** What a number written in the file says, with the underscore it may carry for readability. */
function numberOf(text) {
  return Number.parseInt(text.replace(/_/g, ""), 16) >>> 0;
}

/** A number the writer counts in the base it writes it in, which for the shapes is ten. */
function decimalOf(text) {
  return Number.parseInt(text, 10);
}

/** The bytes a Rust byte string writes, including the escape the fixture's first element carries. */
function bytesOf(literal) {
  const bytes = [];

  for (let i = 0; i < literal.length; i += 1) {
    if (literal[i] === "\\") {
      if (literal[i + 1] === "x") {
        bytes.push(Number.parseInt(literal.slice(i + 2, i + 4), 16));
        i += 3;
      } else {
        bytes.push(literal.charCodeAt(i + 1));
        i += 1;
      }
    } else {
      bytes.push(literal.charCodeAt(i));
    }
  }

  return bytes;
}

const little32 = (value) => [
  value & 255,
  (value >>> 8) & 255,
  (value >>> 16) & 255,
  (value >>> 24) & 255,
];
const little16 = (value) => [value & 255, (value >>> 8) & 255];
const ascii = (text) => [...text].map((character) => character.charCodeAt(0));

/**
 * The fixture, put together by the rules of the format rather than by the writer that is being read: the file
 * prefix `<5sBIB>` is 11 bytes, the target prefix `<6sBI255s2I>` is 274 with a name field that is always 255,
 * each element carries an 8-byte prefix, and the suffix `<4H3sBI>` is 16 with the marker `UFD` — the number is
 * worked out over everything before the CRC field, which includes the 11-byte prefix.
 */
function fixture() {
  const elements = [[ascii("mpcraft fixture\0")], [ascii("golden")]];
  const dataLength = elements.reduce(
    (total, [bytes]) => total + 8 + bytes.length,
    0,
  );
  const name = new Array(255).fill(0);

  ascii("MPCRAFT F405").forEach((byte, index) => {
    name[index] = byte;
  });

  const target = [
    ...ascii("Target"),
    0,
    ...little32(1),
    ...name,
    ...little32(dataLength),
    ...little32(elements.length),
    ...little32(0x0800_0000),
    ...little32(elements[0][0].length),
    ...elements[0][0],
    ...little32(0x0800_0100),
    ...little32(elements[1][0].length),
    ...elements[1][0],
  ];

  const prefix = [...ascii("DfuSe"), 1, ...little32(target.length + 11), 1];
  const suffix = [
    ...little16(0x0413),
    ...little16(0xdf11),
    ...little16(0x0483),
    ...little16(0x011a),
    ...ascii("UFD"),
    16,
  ];

  return prefix.concat(target, suffix);
}

/** What the writer says about the shapes the format gives, read as a document. */
function shapesOf(source) {
  const block =
    source.match(/fn fixture\(\) -> Vec<u8> \{([\s\S]*?)\n    \}/)?.[1] ?? "";

  return {
    nameField: decimalOf(block.match(/\[0_u8; (\d+)\]/)?.[1] ?? "0"),
    targetPrefix: decimalOf(
      block.match(/\(\((\d+) \+ data_len \+ 11\)/)?.[1] ?? "0",
    ),
    filePrefix: decimalOf(block.match(/\+ data_len \+ (\d+)\)/)?.[1] ?? "0"),
    elementPrefix: block.match(
      /(\d+) \+ element_one\.len\(\) \+ (\d+) \+ element_two\.len\(\)/,
    )
      ? Number.parseInt(
          block.match(
            /(\d+) \+ element_one\.len\(\) \+ (\d+) \+ element_two\.len\(\)/,
          )[1],
          10,
        )
      : 0,
    /* The marker is said as the three letters the writer puts out, not as the word the format gives them, so a
       copy that writes them in another order names what it wrote rather than what it should have written. */
    suffixMarker: block.match(/b"([A-Za-z]{3})"/)?.[1] ?? "",
    /* The length of the suffix is the last number the writer puts out before its CRC field. Reading it as the
       first would catch the version byte instead, which is not the size of the suffix. */
    suffixLength: decimalOf(
      (block.split("FIXTURE_CRC")[0].match(/bytes\.push\((\d+)\)/g) ?? [])
        .pop()
        ?.match(/\d+/)?.[0] ?? "0",
    ),
  };
}

/** The rules, said as numbers, so the writer can be compared with them. */
const RULES = {
  nameField: 255,
  targetPrefix: 274,
  filePrefix: 11,
  elementPrefix: 8,
  suffixMarker: "UFD",
  suffixLength: 16,
};

/** The names and the pair the bus half carries as literals, read from the crate rather than written here. */
function namesOfDevice(device) {
  const number = (pattern) =>
    Number.parseInt(device.match(pattern)?.[1] ?? "", 16);

  return {
    /* The name this program reads the interface list through, because `read_element` goes through the class the
       element reports. A row answers under whatever name the install wrote on the device key, so these two are
       only the same fact when they name the same string — which is what the file has to be checked against. */
    class: (
      device.match(
        /pub const WINDOWS_USBDEVICE_CLASS: &str = "([^"]+)"/,
      )?.[1] ?? ""
    ).toLowerCase(),
    /* A second fact: the guid WinUSB samples write, which is not a class on this machine. */
    sample: (
      device.match(/pub const WINUSB_CLASS: &str = "([^"]+)"/)?.[1] ?? ""
    ).toLowerCase(),
    /* pydfu's filter takes this pair from the interface settings; this binding decides by the service the registry
       bound, so the pair is a published fact kept beside the rule that reads it, not the same rule written twice. */
    dfuClass: number(/pub const DFU_CLASS: u8 = (0x[0-9a-fA-F]+);/),
    dfuSubclass: number(/pub const DFU_SUBCLASS: u8 = (\d+);/),
  };
}

/** What pydfu's published filter takes, said as data so the literals can be compared with it. */
const PYDFU_PAIR = { dfuClass: 0xfe, dfuSubclass: 1 };

/** What the file the operator runs with elevation says: the class it declares, and the value it writes. */
function namesOfInf(text) {
  return {
    classGuid: (
      text.match(/^ClassGuid=\{?([0-9a-fA-F][0-9a-fA-F-]*)\}?/m)?.[1] ?? ""
    ).toLowerCase(),
    /* `HKR` in an AddReg section called from a DDInstall is the device key, which is where
       `interface_guid_value` looks, so this line is the reading the crate will get. The type is read as its own
       part of the line rather than assumed, because the value is a list of names. */
    line: text.match(/DeviceInterfaceGUIDs,(0x[0-9a-fA-F]+),"([^"]*)"/) ?? [],
    type: (
      text.match(/DeviceInterfaceGUIDs,(0x[0-9a-fA-F]+)/)?.[1] ?? ""
    ).toLowerCase(),
    value: (
      text.match(
        /DeviceInterfaceGUIDs,0x[0-9a-fA-F]+,"\{?([0-9a-fA-F][0-9a-fA-F-]*)\}?"/,
      )?.[1] ?? ""
    ).toLowerCase(),
  };
}

/** The type the registry stores this value as, said as the number the call reads it with. */
const REG_MULTI_SZ = "0x10000";

function check({ tests, source, inf, device }) {
  const faults = [];

  const golden = {
    fixture: numberOf(
      tests.match(/const FIXTURE_CRC: u32 = (0x[0-9a-f_]+);/)?.[1] ?? "0",
    ),
    specWorded: numberOf(
      tests.match(/const FIXTURE_SPEC_CRC: u32 = (0x[0-9a-f_]+);/)?.[1] ?? "0",
    ),
    samples: [
      ...tests.matchAll(/\(b"([^"]*)", (0x[0-9a-f_]+), (0x[0-9a-f_]+)\)/g),
    ].map(([, literal, published, specWorded]) => ({
      bytes: literal,
      published: numberOf(published),
      specWorded: numberOf(specWorded),
    })),
  };

  if (golden.samples.length === 0) {
    faults.push("the file carries no samples to read");
  }

  const hex = (value) => value.toString(16).padStart(8, "0");

  for (const sample of golden.samples) {
    const bytes = bytesOf(sample.bytes);
    const recomputedPublished = accumulate(bytes, POLYNOMIALS.published, false);
    const recomputedSpec = accumulate(bytes, POLYNOMIALS.specWorded, true);

    if (recomputedPublished !== sample.published) {
      faults.push(
        `the sample ${JSON.stringify(sample.bytes)} says ${hex(sample.published)} and the table built from ${hex(POLYNOMIALS.published)} gives ${hex(recomputedPublished)}`,
      );
    }

    if (recomputedSpec !== sample.specWorded) {
      faults.push(
        `the same bytes read with the wording of the specification give ${hex(recomputedSpec)}, not ${hex(sample.specWorded)}`,
      );
    }

    /* The two readings disagree on these bytes, and that is what the file says they do. A pair written as one
       number closes the disagreement by hand, which is a finding rather than a tidier table. */
    if (sample.published === sample.specWorded) {
      faults.push(
        `the sample ${JSON.stringify(sample.bytes)} writes the two readings as agreeing`,
      );
    }
  }

  const body = fixture();
  const recomputedFixture = accumulate(body, POLYNOMIALS.published, false);
  const recomputedFixtureSpec = accumulate(body, POLYNOMIALS.specWorded, true);

  if (recomputedFixture !== golden.fixture) {
    faults.push(
      `the fixture's number is ${hex(golden.fixture)} and the same bytes put together by the rules give ${hex(recomputedFixture)}`,
    );
  }

  if (recomputedFixtureSpec !== golden.specWorded) {
    faults.push(
      `the same bytes read with the wording of the specification give ${hex(recomputedFixtureSpec)}, not ${hex(golden.specWorded)}`,
    );
  }

  if (golden.fixture === golden.specWorded) {
    faults.push("the fixture writes the two readings as agreeing");
  }

  const shapes = shapesOf(source);

  for (const [shape, rule] of Object.entries(RULES)) {
    if (shapes[shape] !== rule) {
      faults.push(
        `the writer says ${JSON.stringify(shapes[shape])} for the ${shape}, the format gives ${JSON.stringify(rule)}`,
      );
    }
  }

  const names = namesOfInf(inf);
  const constants = namesOfDevice(device);

  if (constants.class === "") {
    faults.push("the crate carries no class name to compare the file with");
  }

  if (constants.sample === "") {
    faults.push("the crate carries no sample guid to keep as a separate fact");
  }

  /* The pair is read through pydfu's published filter, and the service is read through the call that answers; they
     are two rules, so the literals have to agree with the filter rather than be written as if one decided both. */
  if (Number.isNaN(constants.dfuClass) || Number.isNaN(constants.dfuSubclass)) {
    faults.push(
      "the crate carries no pair to compare with the published filter",
    );
  } else if (
    constants.dfuClass !== PYDFU_PAIR.dfuClass ||
    constants.dfuSubclass !== PYDFU_PAIR.dfuSubclass
  ) {
    faults.push(
      `the pair the crate carries is ${JSON.stringify([constants.dfuClass, constants.dfuSubclass])} and the published filter takes ${JSON.stringify([PYDFU_PAIR.dfuClass, PYDFU_PAIR.dfuSubclass])}`,
    );
  }

  if (names.value === "") {
    faults.push(
      "the file writes no value on the device key, so there is nothing to read",
    );
  }

  /* The value is a list of names, not one word, so the type flag the file carries has to be the number the registry
     stores lists with; the live area reads whatever it finds as wide text and stops at the terminator, which is why
     it sees the first name rather than the whole list when the install writes more than one. */
  if (names.line.length !== 0 && names.type !== REG_MULTI_SZ) {
    faults.push(
      `the file writes the value with ${JSON.stringify(names.type)}, and the registry stores a list of names with ${JSON.stringify(REG_MULTI_SZ)}`,
    );
  }

  if (names.classGuid === "") {
    faults.push(
      "the file declares no class, so the element would say nothing about itself",
    );
  }

  /* A row answers under whatever name the value writes, and this binding reads the list through the class the
     element reports, which the file decides. So the two have to name one string; a file that writes another leaves
     the row present and the program refusing, which is what the live area reads as data today. */
  if (
    constants.class !== "" &&
    names.value !== "" &&
    names.value !== constants.class
  ) {
    faults.push(
      `the file writes ${JSON.stringify(names.value)} on the device key, which answers under a name this program cannot read through (${JSON.stringify(constants.class)})`,
    );
  }

  if (
    constants.class !== "" &&
    names.classGuid !== "" &&
    names.classGuid !== constants.class
  ) {
    faults.push(
      `the class the file declares (${JSON.stringify(names.classGuid)}) is not the name this program reads through (${JSON.stringify(constants.class)})`,
    );
  }

  return {
    samples: golden.samples.map((sample) => ({
      bytes: sample.bytes,
      published: hex(sample.published),
      specWorded: hex(sample.specWorded),
      recomputedPublished: hex(
        accumulate(bytesOf(sample.bytes), POLYNOMIALS.published, false),
      ),
      recomputedSpecWorded: hex(
        accumulate(bytesOf(sample.bytes), POLYNOMIALS.specWorded, true),
      ),
    })),
    fixture: {
      bytes: body.length,
      published: hex(golden.fixture),
      specWorded: hex(golden.specWorded),
      recomputedPublished: hex(recomputedFixture),
      recomputedSpecWorded: hex(recomputedFixtureSpec),
    },
    shapes,
    rules: RULES,
    names: {
      sourceClass: constants.class,
      infClassGuid: names.classGuid,
      infValue: names.value,
      /* Kept as data because it is a separate fact, not the name this program reads through. */
      sampleGuid: constants.sample,
      pair: [constants.dfuClass, constants.dfuSubclass],
      publishedPair: [PYDFU_PAIR.dfuClass, PYDFU_PAIR.dfuSubclass],
    },
    faults,
  };
}

/**
 * What the reading cannot reach, said in what it reports rather than only in its header: the numbers come from
 * a table written here from the polynomial, so nothing of the module is executed — the comparison between the
 * code's own rule and these tables is what the module's test carries. The writer is read as text, so a fixture
 * built by a shape the reader does not name is not seen; and the two readings are compared as numbers, which
 * says they disagree on these bytes and not which one a board answers with. The file the operator runs is read as
 * text too, so nothing here shows that an elevated install writes the value or that a row answers afterwards:
 * that is what the live area reads when the install has run.
 */
const LIMIT =
  "the numbers are worked out by a second implementation of the format written here, not by the module that carries them, so nothing of the module is executed and the agreement between its own rule and these tables is what the module's test carries. The writer is read as text: a shape the reading does not name is not seen, and the disagreement is said as two numbers for the same bytes, which does not say which reading a board answers with. The file the operator runs is read as text too, so nothing here shows that an elevated install writes the value or that a row answers afterwards.";

function main() {
  const testsPath = process.argv[2] ?? "crates/mpcraft-app/tests/dfu.rs";
  const sourcePath = process.argv[3] ?? "crates/mpcraft-app/src/dfu.rs";
  const infPath = process.argv[4] ?? "scripts/dfu-winusb.inf";
  const devicePath = process.argv[5] ?? "crates/mpcraft-winusb/src/lib.rs";

  const result = check({
    tests: readFileSync(testsPath, "utf8"),
    source: readFileSync(sourcePath, "utf8"),
    inf: readFileSync(infPath, "utf8"),
    device: readFileSync(devicePath, "utf8"),
  });

  console.log(
    JSON.stringify(
      {
        tests: testsPath,
        source: sourcePath,
        inf: infPath,
        device: devicePath,
        ...result,
        verdict: result.faults.length === 0 ? "PASS" : "FAIL",
        limit: LIMIT,
      },
      null,
      2,
    ),
  );

  process.exit(result.faults.length === 0 ? 0 : 1);
}

if (pathToFileURL(process.argv[1] ?? "").href === import.meta.url) {
  main();
}

export { check };
