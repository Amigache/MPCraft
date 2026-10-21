import { describe, expect, it } from "vitest";

import {
  BAUD_RATES,
  DEFAULT_BAUD,
  buildSerialTarget,
  parseSerialTarget,
  withBaud,
} from "./serialTarget";

describe("the serial target", () => {
  it("is read as a device and a rate", () => {
    expect(parseSerialTarget("serial:COM3:57600")).toEqual({
      device: "COM3",
      baud: 57600,
    });
  });

  it("is read when it has a path as its device", () => {
    expect(parseSerialTarget("serial:/dev/ttyUSB0:115200")).toEqual({
      device: "/dev/ttyUSB0",
      baud: 115200,
    });
  });

  it("is read when it has options after it", () => {
    expect(parseSerialTarget("serial:COM3:57600|nodtr")).toEqual({
      device: "COM3",
      baud: 57600,
    });
  });

  it("falls back to the usual rate when it has none", () => {
    expect(parseSerialTarget("serial:COM3")).toEqual({
      device: "COM3",
      baud: DEFAULT_BAUD,
    });
  });

  it("is not a serial target when it is another kind", () => {
    expect(parseSerialTarget("udp:127.0.0.1:14550")).toBeNull();
    expect(parseSerialTarget("tcp:192.168.1.10:5760")).toBeNull();
    expect(parseSerialTarget("")).toBeNull();
    expect(parseSerialTarget("serial:")).toBeNull();
  });

  it("is built from a device and a rate", () => {
    expect(buildSerialTarget("COM3", 57600)).toBe("serial:COM3:57600");
    expect(buildSerialTarget("/dev/ttyUSB0", 115200)).toBe(
      "serial:/dev/ttyUSB0:115200",
    );
  });

  it("takes another rate", () => {
    expect(withBaud("serial:COM3:57600", 115200)).toBe("serial:COM3:115200");
  });

  it("leaves a target of another kind alone", () => {
    expect(withBaud("udp:127.0.0.1:14550", 115200)).toBe(
      "udp:127.0.0.1:14550",
    );
  });
});

describe("the rates an autopilot uses", () => {
  it("are the usual ones", () => {
    expect(BAUD_RATES).toContain(57600);
    expect(BAUD_RATES).toContain(115200);
    expect(BAUD_RATES).toContain(230400);
    expect(BAUD_RATES).toContain(460800);
    expect(BAUD_RATES).toContain(921600);
    expect(BAUD_RATES).toContain(14400);
    expect(BAUD_RATES).toContain(38400);
    expect(BAUD_RATES).toContain(19200);
    expect(BAUD_RATES).toContain(9600);
    expect(BAUD_RATES).toHaveLength(9);
  });
});
