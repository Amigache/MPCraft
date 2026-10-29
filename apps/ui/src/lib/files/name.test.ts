import { describe, expect, it } from "vitest";

import { fileNameOf } from "./name";

describe("the name a vehicle gives a file", () => {
  it("is the last component of the path the vehicle wrote", () => {
    expect(fileNameOf("/APM/LOGS/00000001.BIN")).toBe("00000001.BIN");
    expect(fileNameOf("APM/LOGS/00000001.BIN")).toBe("00000001.BIN");
  });

  it("is the last component whichever separator the vehicle uses", () => {
    expect(fileNameOf("C:\\logs\\flight.bin")).toBe("flight.bin");
    expect(fileNameOf("logs\\flight.bin")).toBe("flight.bin");
  });

  it("is not the directory above it", () => {
    for (const path of ["..", "logs/..", "..\\", "/"]) {
      expect(fileNameOf(path)).toBe("file");
    }
  });

  it("is not empty when the path names nothing", () => {
    expect(fileNameOf("")).toBe("file");
  });
});
