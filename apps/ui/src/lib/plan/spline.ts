/**
 * The smooth path of a plan.
 *
 * The plan is a list of places, and the vehicle goes from one to the next in a straight
 * line. A path that curves looks like what the vehicle does when it is told to follow
 * a spline, and it is what the plan is drawn with when the operator asks for it.
 *
 * The curve is the one of Catmull-Rom: it passes through every point of the plan, and
 * it is smooth at each of them. The path is the same everywhere — the plan, the map
 * and the KML — so the smooth path is a function and not a drawing.
 */

/** A place of the path. */
export interface Point {
  readonly x: number;
  readonly y: number;
}

/**
 * The points of the smooth path through the given places.
 *
 * The first and the last place are repeated at the ends, so that the curve starts and
 * ends where the plan does. A plan with fewer than three places has no curve: there
 * is nothing to curve between, and the straight line is the path.
 */
export function splinePath(places: readonly Point[]): Point[] {
  const first = places[0];

  if (places.length < 3 || first === undefined) {
    return places.map((place) => ({ x: place.x, y: place.y }));
  }

  const last = places[places.length - 1] ?? first;

  // The place at the given index, with the ends repeated: the curve starts and ends
  // where the plan does.
  const at = (index: number): Point =>
    places[Math.min(Math.max(index, 0), places.length - 1)] ?? first;

  const points: Point[] = [];

  for (let index = 0; index < places.length - 1; index += 1) {
    const previous = at(index - 1);
    const current = at(index);
    const next = at(index + 1);
    const after = at(index + 2);

    // How many points each stretch of the curve has. The more of them, the smoother
    // the curve and the longer the path.
    const steps = 16;

    for (let step = 0; step < steps; step += 1) {
      const t = step / steps;

      points.push({
        x: catmullRom(previous.x, current.x, next.x, after.x, t),
        y: catmullRom(previous.y, current.y, next.y, after.y, t),
      });
    }
  }

  points.push({ x: last.x, y: last.y });

  return points;
}

/**
 * One coordinate of the curve.
 *
 * The curve is the one of Catmull-Rom: it passes through the two places it is between,
 * and its direction at each of them is the one of the line that joins it to its
 * neighbour.
 */
function catmullRom(
  first: number,
  second: number,
  third: number,
  fourth: number,
  t: number,
): number {
  const t2 = t * t;
  const t3 = t2 * t;

  return (
    0.5 *
    (2 * second +
      (-first + third) * t +
      (2 * first - 5 * second + 4 * third - fourth) * t2 +
      (-first + 3 * second - 3 * third + fourth) * t3)
  );
}
