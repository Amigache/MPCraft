/**
 * The files a plan can be written to and read from.
 *
 * Mission Planner writes two kinds of file: the QGC `.waypoints` file, which every ground
 * station reads, and the KML, which Google Earth reads. The format follows the name of
 * the file: the operator chooses the file, and the ground station follows.
 */

/** The formats a plan can be written in. */
export type PlanFileFormat = "waypoints" | "kml";

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
export function formatOfPath(path: string): PlanFileFormat | null {
  const name = path.trim().toLowerCase();

  if (name.endsWith(".kml")) {
    return "kml";
  }
  if (name.endsWith(".waypoints") || name.endsWith(".wpl")) {
    return "waypoints";
  }
  if (name.endsWith(".qgc")) {
    return "waypoints";
  }

  return null;
}

/** The name a file of a format is usually given. */
export function fileNameOf(format: PlanFileFormat): string {
  return format === "kml" ? "plan.kml" : "plan.waypoints";
}

/** The label the format is given in the interface. */
export function labelOfFormat(format: PlanFileFormat): string {
  return format === "kml"
    ? "KML (Google Earth)"
    : "QGC waypoints";
}
