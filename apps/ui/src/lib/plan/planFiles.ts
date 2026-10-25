/**
 * The files a plan can be written to and read from.
 *
 * Mission Planner writes two kinds of file: the QGC `.waypoints` file, which every ground
 * station reads, and the KML, which Google Earth reads. The format follows the name of
 * the file: the operator chooses the file, and the ground station follows.
 *
 * A plan can also be *read* from a drawing — the shapefile of a GIS or the DXF of a CAD
 * program — which is how a route that was drawn on a map is flown without drawing it
 * again. A drawing carries places and no commands, so the way it travels is one way: the
 * ground station reads it and writes a plan, never the other way round.
 */

/** The formats a plan can be written in. */
export type PlanFileFormat = "waypoints" | "kml";

/** The formats a plan can be read from. */
export type PlanLoadFormat = PlanFileFormat | "shp" | "dxf";

/** The name the QGC file is given. */
export const WAYPOINTS_FILE = "waypoints";

/** The name the KML file is given. */
export const KML_FILE = "kml";

/**
 * The format a file name asks for.
 *
 * `null` when the name says nothing the ground station understands: a plan written to a
 * file with an unknown extension would be a file nobody can read back.
 */
export function formatOfPath(path: string): PlanLoadFormat | null {
  const name = path.trim().toLowerCase();

  if (name.endsWith(".kml")) {
    return "kml";
  }
  if (name.endsWith(".shp")) {
    return "shp";
  }
  if (name.endsWith(".dxf")) {
    return "dxf";
  }
  if (name.endsWith(".waypoints") || name.endsWith(".wpl")) {
    return "waypoints";
  }
  if (name.endsWith(".qgc")) {
    return "waypoints";
  }

  return null;
}

/** Whether a plan can be written in a format, or only read from it. */
export function isWritable(format: PlanLoadFormat): format is PlanFileFormat {
  return format === "kml" || format === "waypoints";
}

/** The name a file of a format is usually given. */
export function fileNameOf(format: PlanFileFormat): string {
  return format === "kml" ? "plan.kml" : "plan.waypoints";
}

/** How a part of the interface says something in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

/** The label the format is given in the interface. */
export function labelOfFormat(format: PlanLoadFormat, t: Say): string {
  switch (format) {
    case "kml":
      return t("KML (Google Earth)");
    case "shp":
      return t("ESRI shapefile (the places of the drawing)");
    case "dxf":
      return t("DXF drawing (the places of the drawing)");
    default:
      return t("QGC waypoints");
  }
}
