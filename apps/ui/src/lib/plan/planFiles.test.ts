import { describe, expect, it } from "vitest";

import { fileNameOf, formatOfPath, labelOfFormat } from "./planFiles";

describe("the format a file name asks for", () => {
  it("is the QGC file for the names Mission Planner uses", () => {
    expect(formatOfPath("C:\\plans\\flight.waypoints")).toBe("waypoints");
    expect(formatOfPath("flight.wpl")).toBe("waypoints");
    expect(formatOfPath("flight.qgc")).toBe("waypoints");
  });

  it("is the KML for the name Google Earth reads", () => {
    expect(formatOfPath("/home/pilot/plan.kml")).toBe("kml");
  });

  it("does not mind the case of the name", () => {
    expect(formatOfPath("FLIGHT.WAYPOINTS")).toBe("waypoints");
    expect(formatOfPath("Plan.KML")).toBe("kml");
  });

  it("is nothing for a name the ground station does not understand", () => {
    expect(formatOfPath("flight.txt")).toBeNull();
    expect(formatOfPath("flight")).toBeNull();
    expect(formatOfPath("")).toBeNull();
  });
});

describe("the name a file is given", () => {
  it("carries the extension of the format", () => {
    expect(fileNameOf("waypoints").endsWith(".waypoints")).toBe(true);
    expect(fileNameOf("kml").endsWith(".kml")).toBe(true);
  });
});

describe("the label a format is given", () => {
  it("says what the file is", () => {
    expect(labelOfFormat("kml")).toContain("KML");
    expect(labelOfFormat("waypoints")).toContain("waypoints");
  });
});
