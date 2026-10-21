import type { PlanItem } from "../ipc/types";

/**
 * The circle of a survey.
 *
 * A vehicle that flies a circle over a place — a field, a quarry, a crop — is given a
 * ring of points and flies from one to the next. The ring is not a circle on the ground:
 * it is a circle in the projection of the map, and the points are spread evenly along it.
 *
 * The generator is a pure function: it takes the centre, the radius and the number of
 * points, and gives back the points. The plan is the vehicle's business.
 */

/** The earth, in metres. */
const EARTH_RADIUS = 6_371_008.8;

/** The radians of a degree. */
const TO_RADIANS = Math.PI / 180;

/** The centre of a circle of a survey. */
export interface CircleCentre {
  /** The latitude of the centre, in degrees. */
  readonly lat: number;
  /** The longitude of the centre, in degrees. */
  readonly lng: number;
}

/** What the generator is asked for. */
export interface CircleRequest {
  /** Where the circle is. */
  readonly centre: CircleCentre;
  /** The radius of the circle, in metres. */
  readonly radius: number;
  /** How many points the circle has. */
  readonly points: number;
  /** The height the vehicle flies at, in metres. */
  readonly altitude: number;
  /** Whether the circle is turned the other way. */
  readonly clockwise: boolean;
}

/**
 * The points of the circle.
 *
 * The points are spread evenly along the circle, and the first one is the one that is
 * closest to the north, so that the circle does not turn with the number of points.
 */
export function circlePoints(request: CircleRequest): PlanItem[] {
  const { centre, radius, points, altitude, clockwise } = request;

  if (points < 3 || radius <= 0) {
    return [];
  }

  // The degrees of latitude that a metre covers, and the degrees of longitude at the
  // latitude of the centre: they get shorter towards the poles.
  const perMetre = 1 / (EARTH_RADIUS * TO_RADIANS);
  const perLongitude = perMetre / Math.cos(centre.lat * TO_RADIANS);

  const step = (2 * Math.PI) / points;
  const direction = clockwise ? -1 : 1;

  const generated: PlanItem[] = [];

  for (let index = 0; index < points; index += 1) {
    const angle = direction * step * index;

    generated.push({
      seq: 0,
      command: "MAV_CMD_NAV_WAYPOINT",
      commandValue: 16,
      frame: 3,
      params: [0, 0, 0, 0],
      x: centre.lat + Math.sin(angle) * radius * perMetre,
      y: centre.lng + Math.cos(angle) * radius * perLongitude,
      z: altitude,
      current: false,
      autocontinue: true,
    });
  }

  return generated;
}
