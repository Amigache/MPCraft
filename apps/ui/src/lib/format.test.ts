import { describe, expect, it } from "vitest";

import {
  DASH,
  formatAltitude,
  formatAngle,
  formatCoordinate,
  formatCount,
  formatCurrent,
  formatDistance,
  formatHeading,
  formatNumber,
  formatPercent,
  formatSpeed,
  formatText,
  formatVoltage,
  formatYesNo,
  present,
} from "./format";

describe("the formatting of the values", () => {
  it("shows a dash when there is nothing to show", () => {
    expect(formatNumber(null)).toBe(DASH);
    expect(formatNumber(undefined)).toBe(DASH);
    expect(formatDistance(null)).toBe(DASH);
    expect(formatSpeed(null)).toBe(DASH);
    expect(formatHeading(null)).toBe(DASH);
    expect(formatVoltage(null)).toBe(DASH);
    expect(formatCurrent(null)).toBe(DASH);
    expect(formatPercent(null)).toBe(DASH);
    expect(formatYesNo(null)).toBe(DASH);
    expect(formatText(null)).toBe(DASH);
    expect(formatText("")).toBe(DASH);
  });

  it("does not show a value that is not a number", () => {
    expect(formatNumber(Number.NaN)).toBe(DASH);
    expect(formatNumber(Number.POSITIVE_INFINITY)).toBe(DASH);
    expect(present(Number.NaN)).toBe(false);
    expect(present(0)).toBe(true);
  });

  it("shows a distance in metres until it is a kilometre", () => {
    expect(formatDistance(12.34)).toBe("12.3 m");
    expect(formatDistance(999.9)).toBe("999.9 m");
    expect(formatDistance(1000)).toBe("1.00 km");
    expect(formatDistance(15400)).toBe("15.40 km");
  });

  it("shows a heading as a bearing between nought and three hundred and sixty", () => {
    expect(formatHeading(0)).toBe("0°");
    expect(formatHeading(90)).toBe("90°");
    expect(formatHeading(359.6)).toBe("360°");
    expect(formatHeading(-90)).toBe("270°");
    expect(formatHeading(450)).toBe("90°");
  });

  it("shows a pair of coordinates", () => {
    expect(formatCoordinate(-35.2809, 149.13)).toBe("-35.280900, 149.130000");
    expect(formatCoordinate(-35.2809, null)).toBe(DASH);
  });

  it("shows a whole count with a thousands separator", () => {
    expect(formatCount(1234)).toBe("1,234");
    expect(formatCount(0)).toBe("0");
  });

  it("shows the other values with their unit", () => {
    expect(formatAltitude(584.25)).toBe("584.3 m");
    expect(formatSpeed(12)).toBe("12.0 m/s");
    expect(formatVoltage(12.6)).toBe("12.60 V");
    expect(formatCurrent(-4.2)).toBe("-4.20 A");
    expect(formatPercent(87)).toBe("87 %");
    expect(formatAngle(-1.25)).toBe("-1.3°");
    expect(formatYesNo(true)).toBe("yes");
    expect(formatYesNo(false)).toBe("no");
  });
});
