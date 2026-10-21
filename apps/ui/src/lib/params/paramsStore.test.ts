import { describe, expect, it } from "vitest";

import type { KnownParameter } from "./paramsStore";
import { mergeParameters, replaceParameter } from "./paramsStore";

/** One parameter, for the test. */
function parameter(
  name: string,
  value: number | null,
  reportedType = 1,
): KnownParameter {
  return { name, value, reportedType };
}

describe("the list of parameters", () => {
  it("is the list that came in when nothing has changed", () => {
    const previous = [parameter("ARMING_CHECK", 1), parameter("RTL_ALT", 0)];
    const incoming = [parameter("ARMING_CHECK", 1), parameter("RTL_ALT", 0)];

    // The same objects are kept, so that the rows that draw them are not drawn again.
    expect(mergeParameters(previous, incoming)).toBe(previous);
  });

  it("is the list that came in when one value has changed", () => {
    const previous = [parameter("ARMING_CHECK", 1), parameter("RTL_ALT", 0)];
    const incoming = [parameter("ARMING_CHECK", 1), parameter("RTL_ALT", 50)];

    const merged = mergeParameters(previous, incoming);

    expect(merged).not.toBe(previous);
    expect(merged[1]?.value).toBe(50);
    expect(merged[0]).toBe(previous[0]);
  });

  it("forgets a parameter the vehicle no longer reports", () => {
    const previous = [parameter("ARMING_CHECK", 1), parameter("RTL_ALT", 0)];
    const incoming = [parameter("ARMING_CHECK", 1)];

    expect(mergeParameters(previous, incoming)).toHaveLength(1);
  });

  it("keeps a parameter the vehicle has not reported as nothing", () => {
    const previous = [parameter("RTL_ALT", null)];
    const incoming = [parameter("RTL_ALT", null)];

    expect(mergeParameters(previous, incoming)[0]?.value).toBeNull();
  });
});

describe("a parameter that has been written", () => {
  it("goes back where it was", () => {
    const previous = [
      parameter("ARMING_CHECK", 1),
      parameter("RTL_ALT", 0),
      parameter("WP_RADIUS", 90),
    ];

    const replaced = replaceParameter(previous, parameter("RTL_ALT", 50));

    expect(replaced.map((entry) => entry.name)).toEqual([
      "ARMING_CHECK",
      "RTL_ALT",
      "WP_RADIUS",
    ]);
    expect(replaced[1]?.value).toBe(50);
  });

  it("is added when the list did not have it", () => {
    const previous = [parameter("ARMING_CHECK", 1)];

    const replaced = replaceParameter(previous, parameter("RTL_ALT", 50));

    expect(replaced).toHaveLength(2);
    expect(replaced[1]?.name).toBe("RTL_ALT");
  });

  it("does not change the list when the value is the same", () => {
    const previous = [parameter("RTL_ALT", 50)];

    expect(replaceParameter(previous, parameter("RTL_ALT", 50))).toBe(previous);
  });
});
