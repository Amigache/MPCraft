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
    /* The two spellings of the value type, read through their own side: what an AddReg line writes and what the call
       that answers the value reports. They are carried apart in the crate, so a copy that writes them as one number
       is read as closing the difference between a file and a reading. */
    infType: (
      device.match(
        /pub const INF_TYPE_MULTI_SZ: &str = "(0x[0-9a-fA-F]+)"/,
      )?.[1] ?? ""
    ).toLowerCase(),
    registryType: (
      device.match(
        /pub const REGISTRY_TYPE_MULTI_SZ: &str = "(0x[0-9a-fA-F]+)"/,
      )?.[1] ?? ""
    ).toLowerCase(),
    /* The two readings of one value, read as the shapes they carry. They are two rules — one walks the entries
       between terminators, the other answers with the word up to the first one — and the difference is what this
       machine showed today on one value: the same bytes gave one word to the reader that stops, and one entry with a
       comma inside it to the reader that splits, which the rule that reads names refused. So a copy that splits by
       another mark, or a word reading that keeps going past its first terminator, says the pair is one fact seen
       through a different mark. */
    splitsAtTerminators:
      /pub fn entries_of\(units: &\[u16\]\)[\s\S]*?split\(\|unit\| \*unit == 0\)/.test(
        device,
      ),
    stopsAtFirstTerminator:
      /pub fn as_wide\(units: &\[u16\]\)[\s\S]*?split\('\\0'\)\s*\n?\s*\.next\(\)/.test(
        device,
      ),
    /* The pair a string form can carry, read as its own fact: both sides or neither. It comes from a published
       reader (`CLSIDFromString` takes a brace-enclosed string as well as the bare form) and it is not the same thing
       as the five parts these APIs print, so a copy that trims the opening brace without looking for the closing one
       reads a name out of a value that never said one. */
    pairReadAsItsOwnFact:
      /pub fn name_without_the_pair\(name: &str\)[\s\S]*?opens != closes/.test(
        device,
      ),
  };
}

/** One fact with two spellings, each from its own published source: the number an AddReg line of a class-install INF
    writes for a list of names, and the number the table of value types gives the same fact to the call that reads it. */
const SPELLINGS = { inf: "0x10000", registry: "0x7" };

/** What pydfu's published filter takes, said as data so the literals can be compared with it. */
const PYDFU_PAIR = { dfuClass: 0xfe, dfuSubclass: 1 };

/** What the file the operator runs with elevation says: the class it declares, and the value it writes. */
function namesOfInf(text) {
  return {
    classGuid: (
      text.match(/^ClassGuid=\{?([0-9a-fA-F][0-9a-fA-F-]*)\}?/m)?.[1] ?? ""
    ).toLowerCase(),
    /* `HKR` in an AddReg section called from a DDInstall is the device key, which is where
       `interface_guid_value` looks, so this line is the reading the crate will get. The type and the names are read
       as separate parts of it rather than assumed, because the type decides what a reader can walk and the names
       decide under which name a row would answer. */
    type: (
      text.match(/DeviceInterfaceGUIDs,(0x[0-9a-fA-F]+)/)?.[1] ?? ""
    ).toLowerCase(),
    valueRaw:
      text.match(/DeviceInterfaceGUIDs,0x[0-9a-fA-F]+,"([^"]*)"/)?.[1] ?? "",
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

  /* One fact with two spellings, kept apart because each comes from its own source: what an AddReg line writes for a
     list of names is `0x10000`, and what the call that answers the value reports is `7`. Writing them as the same
     number closes the difference the reading carries, and the shape itself decides what a reader can walk — one
     reading stops at the terminator and answers with one word, the other splits at the terminators and answers with
     the entries, so two names inside one entry are refused by the rule that reads names while two entries would each
     be read, which is what this machine answered for the instance the operator wrote. A single absence is said once,
     so the type is read only when there is a line to read. */
  /* The name in the line is read as the file writes it, and the pair around it is its own fact: both sides or neither.
     A line that opens the pair without closing it writes a text that is not a name on either reading — which is what a
     command lost on this machine when it was pasted, so the gate says it rather than reading a class out of an
     incomplete string. */
  if (names.valueRaw !== "") {
    const opens = names.valueRaw.startsWith("{");
    const closes = names.valueRaw.endsWith("}");

    if (opens !== closes) {
      faults.push(
        `the line writes the name with the pair on one side only (${JSON.stringify(names.valueRaw)}), which is not a name either reading gives`,
      );
    }
  }

  if (names.value === "") {
    faults.push(
      "the file writes no value on the device key, so there is nothing to read",
    );
  } else if (names.type !== SPELLINGS.inf) {
    faults.push(
      `the file writes the value with ${JSON.stringify(names.type)}, and an AddReg line spells a list of names with ${JSON.stringify(SPELLINGS.inf)}`,
    );
  }

  if (constants.infType === "") {
    faults.push(
      "the crate carries no spelling of the value type to compare the file with",
    );
  }

  if (constants.registryType === "") {
    faults.push(
      "the crate carries no number from the call that reads the value",
    );
  }

  if (
    constants.infType !== "" &&
    constants.registryType !== "" &&
    constants.infType === constants.registryType
  ) {
    faults.push(
      "the two spellings of the value type are written as the same number, which is the difference the reading carries",
    );
  }

  if (constants.infType !== "" && constants.infType !== SPELLINGS.inf) {
    faults.push(
      `the crate carries ${JSON.stringify(constants.infType)} for the AddReg spelling of this value, and the published shape of a line writes lists as ${JSON.stringify(SPELLINGS.inf)}`,
    );
  }

  if (
    constants.registryType !== "" &&
    constants.registryType !== SPELLINGS.registry
  ) {
    faults.push(
      `the crate carries ${JSON.stringify(constants.registryType)} for what the call reads this value as, and the published table of value types gives a list of names as ${JSON.stringify(SPELLINGS.registry)}`,
    );
  }

  /* The two readings have to stay two rules: one walks the entries between terminators and the other answers with
     the word up to the first one. Today's machine showed the difference on one value — the same bytes gave one word
     to the reader that stops and one entry with a comma inside to the reader that splits, and the rule that reads
     names refused that entry, so a copy that splits by another mark, or a word reading that keeps going past its
     first terminator, says the two readings are one rule seen through a different mark. */
  if (!constants.splitsAtTerminators) {
    faults.push(
      "the reading that walks entries does not split at the terminators, which is where a list of names differs from a word",
    );
  }

  if (!constants.stopsAtFirstTerminator) {
    faults.push(
      "the reading that answers with a word but does not stop at the first terminator, which is what makes it a word",
    );
  }

  /* And the braces are a third fact, read as both-or-neither: `StringFromCLSID` writes a name with them and these
     APIs print it without them, so a name that opens the pair and does not close it — which is what this machine
     answered after the corrected write — has to be refused rather than read as a class. A copy that trims the opening
     brace on its own turns an incomplete text into a name. */
  if (!constants.pairReadAsItsOwnFact) {
    faults.push(
      "the pair around a string form of a name is read as decoration rather than as its own fact, so a name that opens it without closing it would be read as a class",
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
      infType: constants.infType,
      registryType: constants.registryType,
      publishedSpellings: SPELLINGS,
      /* The name as the line writes it, kept with its pair rather than read through a reader that strips it. */
      valueAsWritten: names.valueRaw,
      valuePairComplete:
        names.valueRaw.startsWith("{") && names.valueRaw.endsWith("}"),
      /* The two readings of one value, kept as the shapes they carry so a copy that collapses them is seen. */
      splitsAtTerminators: constants.splitsAtTerminators,
      stopsAtFirstTerminator: constants.stopsAtFirstTerminator,
      pairReadAsItsOwnFact: constants.pairReadAsItsOwnFact,
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
  "the numbers are worked out by a second implementation of the format written here, not by the module that carries them, so nothing of the module is executed and the agreement between its own rule and these tables is what the module's test carries. The writer is read as text: a shape the reading does not name is not seen, and the disagreement is said as two numbers for the same bytes, which does not say which reading a board answers with. The file the operator runs is read as text too, so nothing here shows that an elevated install writes the value or that a row answers afterwards, and the two spellings of the value type are compared as text, which does not show what the call answered for a machine: the number the bus gave — 7 for this instance, against the `0x10000` the file writes — is what the live area reads. The three shapes the gate compares here (splitting at the terminators, stopping at the first one, and taking the brace pair as both-or-neither) are read as text, so they say how the rule is written and not what a machine stored on the key.";

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
