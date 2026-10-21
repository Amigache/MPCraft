import type { PlanItem } from "../ipc/types";

/**
 * The reticle of a survey.
 *
 * A vehicle that is to survey a field flies it in strips: it goes along the first one,
 * across to the next, and back along it, and so on until the field is covered. The
 * strips are apart by the distance the vehicle is to sweep with its sensor, and the
 * whole thing is turned by the angle the field is at.
 *
 * The generator is a pure function: it takes the field and says where the points are.
 * The plan is the vehicle's business.
 */

/** What the generator is asked for. */
export interface SurveyRequest {
  /** The middle of the field. */
  readonly centre: { readonly lat: number; readonly lng: number };
  /** How long the strips are, in metres. */
  readonly length: number;
  /** How far apart the strips are, in metres. */
  readonly spacing: number;
  /** How many strips there are. */
  readonly strips: number;
  /** The angle of the field, in degrees, from the north. */
  readonly heading: number;
  /** The height the vehicle flies at, in metres. */
  readonly altitude: number;
}

/** The earth, in metres. */
const EARTH_RADIUS = 6_371_008.8;

/** The radians of a degree. */
const TO_RADIANS = Math.PI / 180;

/**
 * The points of the survey.
 *
 * The strips are in the order the vehicle flies them: the first one from one side to
 * the other, the second one back, and so on. The turn between the strips is a point of
 * its own, so that the vehicle does not have to turn on the spot.
 */
export function surveyPoints(request: SurveyRequest): PlanItem[] {
  const { centre, length, spacing, strips, heading, altitude } = request;

  if (strips < 1 || length <= 0 || spacing <= 0) {
    return [];
  }

  const angle = heading * TO_RADIANS;

  // The direction of the strips, and the direction across them.
  const along = { north: Math.cos(angle), east: Math.sin(angle) };
  const across = { north: -Math.sin(angle), east: Math.cos(angle) };

  // The half-width of the field, and the length of each strip.
  const half = ((strips - 1) * spacing) / 2;
  const halfLength = length / 2;

  // The degrees a metre covers, at the latitude of the field.
  const perMetre = 1 / (EARTH_RADIUS * TO_RADIANS);
  const perLongitude = perMetre / Math.cos(centre.lat * TO_RADIANS);

  const points: PlanItem[] = [];

  for (let strip = 0; strip < strips; strip += 1) {
    // The middle of the strip, and the two ends of it.
    const offset = (strip * spacing) - half;

    const middle = {
      lat: centre.lat + across.north * offset * perMetre,
      lng: centre.lng + across.east * offset * perLongitude,
    };

    const first = {
      lat: middle.lat - along.north * halfLength * perMetre,
      lng: middle.lng - along.east * halfLength * perLongitude,
    };

    const last = {
      lat: middle.lat + along.north * halfLength * perMetre,
      lng: middle.lng + along.east * halfLength * perLongitude,
    };

    // The vehicle goes along the strip, and the turn is a point of its own.
    const waypoints = strip % 2 === 0 ? [first, last] : [last, first];

    for (const waypoint of waypoints) {
      points.push(point(waypoint.lat, waypoint.lng, altitude));
    }
  }

  return points;
}

/**
 * A point of a plan.
 *
 * The command is the one of a waypoint, and the frame the global one: a plan of
 * positions is a plan of positions wherever it is.
 */
function point(lat: number, lng: number, altitude: number): PlanItem {
  return {
    seq: 0,
    command: "MAV_CMD_NAV_WAYPOINT",
    commandValue: 16,
    frame: 3,
    params: [0, 0, 0, 0],
    x: lat,
    y: lng,
    z: altitude,
    current: false,
    autocontinue: true,
  };
}
