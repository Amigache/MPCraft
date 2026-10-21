import type { PlanItem } from "../ipc/types";

/**
 * The fence.
 *
 * A geofence is a polygon: the points that are **included** in it are its vertices, and
 * the points that are **excluded** from it are the holes in it. A fence may also hold
 * circles — areas that are included in, or excluded from, the fence.
 *
 * The distinction is in the command of each point, and it is the vehicle's knowledge: the
 * interface draws what the commands say.
 */

/** The command of a vertex of the polygon of a fence. */
export const VERTEX_INCLUSION = "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION";

/** The command of a point that is excluded from the polygon of a fence. */
export const VERTEX_EXCLUSION = "MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION";

/** The command of a circle that is included in the fence. */
export const CIRCLE_INCLUSION = "MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION";

/** The command of a circle that is excluded from the fence. */
export const CIRCLE_EXCLUSION = "MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION";

/** The command of the point the vehicle returns to when it leaves the fence. */
export const RETURN_POINT = "MAV_CMD_NAV_FENCE_RETURN_POINT";

/** The command of a rally point. */
export const RALLY_POINT = "MAV_CMD_NAV_RALLY_POINT";

/** How many points a circle is drawn with. */
const CIRCLE_POINTS = 36;

/** The earth, in metres, as the geodesy crate counts it. */
const EARTH_RADIUS = 6_371_008.8;

/** One point of a fence. */
export interface FencePoint {
  /** The latitude, in degrees. */
  readonly lat: number;
  /** The longitude, in degrees. */
  readonly lng: number;
}

/** A circle of a fence. */
export interface FenceCircle extends FencePoint {
  /** The radius of the circle, in metres. */
  readonly radius: number;
  /** Whether the circle is included in the fence or excluded from it. */
  readonly included: boolean;
}

/** The points of the polygon of a fence, in order. */
export function verticesOf(items: readonly PlanItem[]): FencePoint[] {
  return items
    .filter((item) => item.command === VERTEX_INCLUSION)
    .map((item) => ({ lat: item.x, lng: item.y }));
}

/** The points that are excluded from the polygon of a fence. */
export function exclusionsOf(items: readonly PlanItem[]): FencePoint[] {
  return items
    .filter((item) => item.command === VERTEX_EXCLUSION)
    .map((item) => ({ lat: item.x, lng: item.y }));
}

/** The circles of a fence. */
export function circlesOf(items: readonly PlanItem[]): FenceCircle[] {
  const circles: FenceCircle[] = [];

  for (const item of items) {
    const included =
      item.command === CIRCLE_INCLUSION || item.command === CIRCLE_EXCLUSION;

    if (!included) {
      continue;
    }

    circles.push({
      lat: item.x,
      lng: item.y,
      radius: item.params[0],
      included: item.command === CIRCLE_INCLUSION,
    });
  }

  return circles;
}

/**
 * The ring of the polygon of a fence.
 *
 * A polygon is closed: the first point is the last one. A fence of fewer than three
 * vertices has no ring, because a polygon needs three.
 */
export function ringOf(items: readonly PlanItem[]): [number, number][] | null {
  const vertices = verticesOf(items);

  if (vertices.length < 3) {
    return null;
  }

  const ring = vertices.map((point) => [point.lng, point.lat] as [number, number]);
  const first = ring[0];

  return first === undefined ? null : [...ring, first];
}

/**
 * The ring of a circle of a fence.
 *
 * A circle is drawn with a ring of points: the map has no circles of their own, and a
 * ring of thirty-six points is closer to a circle than a dozen would be.
 */
export function circleRing(circle: FenceCircle): [number, number][] {
  const ring: [number, number][] = [];

  // The degrees of latitude and of longitude that a distance in metres covers.
  //
  // The degrees of longitude get shorter towards the poles: a circle that is drawn with
  // the same number of degrees of longitude everywhere is an oval, and at the latitude of
  // Canberra it is a circle that is a quarter too wide.
  const perDegree = Math.PI / 180;
  const latitude = circle.lat * perDegree;

  // The degrees that a metre covers, in each direction.
  const perLatitude = 1 / perDegree / EARTH_RADIUS;
  const perLongitude = 1 / perDegree / (EARTH_RADIUS * Math.cos(latitude));

  for (let step = 0; step < CIRCLE_POINTS; step += 1) {
    const angle = (step / CIRCLE_POINTS) * 2 * Math.PI;

    ring.push([
      circle.lng + circle.radius * Math.cos(angle) * perLongitude,
      circle.lat + circle.radius * Math.sin(angle) * perLatitude,
    ]);
  }

  return ring;
}
