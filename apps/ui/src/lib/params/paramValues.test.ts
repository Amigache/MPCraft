import { describe, expect, it } from "vitest";

import { DASH } from "../format";
import { isIntegralType, paramTypeName } from "./paramTypes";
import {
  formatParameterValue,
  parseParameterValue,
} from "./paramValues";

/** The types the backend reports, by the number it sends. */
const UINT8 = 1;
const UINT16 = 3;
const INT16 = 4;
const UINT32 = 5;
const REAL32 = 9;
const REAL64 = 10;

describe("the value of a parameter", () => {
  it("is a dash when the vehicle has not reported one", () => {
    expect(formatParameterValue(null, UINT8)).toBe(DASH);
  });

  it("is a zero when the vehicle has reported a zero", () => {
    expect(formatParameterValue(0, UINT8)).toBe("0");
    expect(formatParameterValue(0, UINT16)).toBe("0");
    expect(formatParameterValue(0, REAL32)).toBe("0");
    expect(formatParameterValue(0, REAL64)).toBe("0");
  });

  it("is a whole number for a type that holds one", () => {
    expect(formatParameterValue(5, UINT8)).toBe("5");
    expect(formatParameterValue(57600, UINT32)).toBe("57600");
    expect(formatParameterValue(-3, INT16)).toBe("-3");
  });

  it("does not show the noise of a single precision float", () => {
    // The vehicle sends a float32; read as a double it carries digits that were never
    // there.
    expect(formatParameterValue(1.100000023841858, REAL32)).toBe("1.1");
    expect(formatParameterValue(0.5, REAL32)).toBe("0.5");
  });

  it("keeps the digits of a double precision float", () => {
    expect(formatParameterValue(0.1, REAL64)).toBe("0.1");
    expect(formatParameterValue(1 / 3, REAL64)).toBe("0.333333333333333");
  });
});

describe("the type of a parameter", () => {
  it("is named after the enumeration the backend reports", () => {
    expect(paramTypeName(UINT8)).toBe("UINT8");
    expect(paramTypeName(INT16)).toBe("INT16");
    expect(paramTypeName(REAL32)).toBe("REAL32");
    expect(paramTypeName(REAL64)).toBe("REAL64");
  });

  it("says so when the type is one it has not heard of", () => {
    expect(paramTypeName(99)).toBe("TYPE 99");
  });

  it("is a whole number for the types that hold one", () => {
    expect(isIntegralType(UINT8)).toBe(true);
    expect(isIntegralType(INT16)).toBe(true);
    expect(isIntegralType(UINT32)).toBe(true);
    expect(isIntegralType(REAL32)).toBe(false);
    expect(isIntegralType(REAL64)).toBe(false);
  });
});

describe("what the operator typed", () => {
  it("is a number", () => {
    expect(parseParameterValue("12.5", REAL32)).toEqual({ ok: true, value: 12.5 });
    expect(parseParameterValue(" 12.5 ", REAL32)).toEqual({ ok: true, value: 12.5 });
    expect(parseParameterValue("-3", INT16)).toEqual({ ok: true, value: -3 });
    expect(parseParameterValue("1e3", REAL32)).toEqual({ ok: true, value: 1000 });
  });

  it("is refused when it is not a number", () => {
    expect(parseParameterValue("", REAL32)).toEqual({ ok: false, reason: "no value" });
    expect(parseParameterValue("abc", REAL32)).toEqual({
      ok: false,
      reason: "not a number",
    });
    expect(parseParameterValue("1,5", REAL32)).toEqual({
      ok: false,
      reason: "not a number",
    });
    expect(parseParameterValue("0x10", REAL32)).toEqual({
      ok: false,
      reason: "not a number",
    });
    expect(parseParameterValue("Infinity", REAL32)).toEqual({
      ok: false,
      reason: "not a number",
    });
  });

  it("is refused when it has a fraction and the type holds none", () => {
    expect(parseParameterValue("1.5", INT16)).toEqual({
      ok: false,
      reason: "must be a whole number",
    });
    expect(parseParameterValue("0.5", UINT8)).toEqual({
      ok: false,
      reason: "must be a whole number",
    });
  });

  it("is taken when it has a fraction and the type holds one", () => {
    expect(parseParameterValue("1.5", REAL32)).toEqual({ ok: true, value: 1.5 });
  });

  it("is taken when it is a whole number written as one", () => {
    expect(parseParameterValue("12.0", INT16)).toEqual({ ok: true, value: 12 });
  });
});
