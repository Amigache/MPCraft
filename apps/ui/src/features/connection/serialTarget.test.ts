import { describe, expect, it } from "vitest";

import {
  BAUD_RATES,
  DEFAULT_BAUD,
  buildSerialTarget,
  isAutoBaud,
  parseSerialTarget,
  withAutoBaud,
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
    expect(withBaud("udp:127.0.0.1:14550", 115200)).toBe("udp:127.0.0.1:14550");
  });
});

describe("the rate that is left to the program", () => {
  it("is written on the target", () => {
    expect(withAutoBaud("serial:COM3:57600")).toBe(
      "serial:COM3:57600|baud=auto",
    );
  });

  it("replaces the rate that was there", () => {
    expect(withAutoBaud("serial:COM3:115200|baud=57600")).toBe(
      "serial:COM3:115200|baud=auto",
    );
  });

  it("leaves a target of another kind alone", () => {
    expect(withAutoBaud("udp:127.0.0.1:14550")).toBe("udp:127.0.0.1:14550");
  });

  it("is seen on the target", () => {
    expect(isAutoBaud("serial:COM3:57600|baud=auto")).toBe(true);
    expect(isAutoBaud("serial:COM3:57600")).toBe(false);
    expect(isAutoBaud("udp:127.0.0.1:14550|baud=auto")).toBe(false);
  });

  it("leaves the device of the target alone", () => {
    expect(parseSerialTarget("serial:COM3:57600|baud=auto")?.device).toBe(
      "COM3",
    );
  });
});

describe("the rates an autopilot uses", () => {
  it("are the ones Mission Planner offers, slowest first", () => {
    // The list ConnectionControl offers: a port left at 9600 and a link at 921600 are both one
    // choice away, and the order runs from the slowest to the fastest so the list reads as a scale.
    expect(BAUD_RATES).toEqual([
      1200, 2400, 4800, 9600, 19200, 38400, 57600, 111100, 115200, 230400,
      250000, 460800, 500000, 921600, 1000000, 1500000, 2000000, 3000000,
    ]);

    for (let index = 1; index < BAUD_RATES.length; index += 1) {
      expect(BAUD_RATES[index]).toBeGreaterThan(BAUD_RATES[index - 1] ?? 0);
    }
  });
});
