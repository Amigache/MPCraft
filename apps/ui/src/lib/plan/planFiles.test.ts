import { describe, expect, it } from "vitest";

import {
  fileNameOf,
  formatOfPath,
  isWritable,
  labelOfFormat,
} from "./planFiles";

describe("the format a file name asks for", () => {
  it("is the QGC file for the names Mission Planner uses", () => {
    expect(formatOfPath("C:\\plans\\flight.waypoints")).toBe("waypoints");
    expect(formatOfPath("flight.wpl")).toBe("waypoints");
    expect(formatOfPath("flight.qgc")).toBe("waypoints");
  });

  it("is the KML for the name Google Earth reads", () => {
    expect(formatOfPath("/home/pilot/plan.kml")).toBe("kml");
  });

  it("is the drawing for the files a GIS or a CAD program writes", () => {
    expect(formatOfPath("C:\\drawings\\farm.shp")).toBe("shp");
    expect(formatOfPath("track.dxf")).toBe("dxf");
    expect(formatOfPath("TRACK.DXF")).toBe("dxf");
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

describe("the way a format travels", () => {
  it("writes the formats every ground station reads", () => {
    expect(isWritable("waypoints")).toBe(true);
    expect(isWritable("kml")).toBe(true);
  });

  it("only reads a drawing, because it carries no command", () => {
    expect(isWritable("shp")).toBe(false);
    expect(isWritable("dxf")).toBe(false);
  });
});

describe("the label a format is given", () => {
  it("says what the file is", () => {
    // The bundles are not there in a unit test, so the words come from the code.
    const say = (source: string) => source;

    expect(labelOfFormat("kml", say)).toContain("KML");
    expect(labelOfFormat("waypoints", say)).toContain("waypoints");
    expect(labelOfFormat("shp", say)).toContain("shapefile");
    expect(labelOfFormat("dxf", say)).toContain("DXF");
  });
});
