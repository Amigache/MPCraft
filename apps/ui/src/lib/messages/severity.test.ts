import { describe, expect, it } from "vitest";

import { messageTone } from "./severity";

describe("the tone of a message", () => {
  it("is the most serious one for the severities that matter", () => {
    expect(messageTone(0)).toBe("critical");
    expect(messageTone(1)).toBe("critical");
    expect(messageTone(2)).toBe("critical");
    expect(messageTone(3)).toBe("critical");
  });

  it("is a warning for the one that warns", () => {
    expect(messageTone(4)).toBe("warning");
  });

  it("is information for what the vehicle has to say", () => {
    expect(messageTone(5)).toBe("info");
    expect(messageTone(6)).toBe("info");
  });

  it("is quiet for the debugging detail", () => {
    expect(messageTone(7)).toBe("debug");
    expect(messageTone(42)).toBe("debug");
  });
});
