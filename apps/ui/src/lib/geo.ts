/**
 * The measures of the ground.
 *
 * The distances and the bearings of two places, the way the geodesy crate computes them.
 * They are here because the interface has to measure the distance between the vehicle
 * and the places of the plan without asking the backend for it twenty times a second.
 */

/** The earth, in metres. */
const EARTH_RADIUS = 6_371_008.8;

/** The radians of a degree. */
const TO_RADIANS = Math.PI / 180;

/**
 * The distance between two places, in metres.
 *
 * The great circle between them, which is the shortest way round the earth.
 */
export function distanceBetween(
  fromLat: number,
  fromLng: number,
  toLat: number,
  toLng: number,
): number {
  const lat1 = fromLat * TO_RADIANS;
  const lat2 = toLat * TO_RADIANS;
  const deltaLat = (toLat - fromLat) * TO_RADIANS;
  const deltaLng = (toLng - fromLng) * TO_RADIANS;

  const a =
    Math.sin(deltaLat / 2) * Math.sin(deltaLat / 2) +
    Math.cos(lat1) *
      Math.cos(lat2) *
      Math.sin(deltaLng / 2) *
      Math.sin(deltaLng / 2);

  return 2 * EARTH_RADIUS * Math.asin(Math.min(1, Math.sqrt(a)));
}

/**
 * The bearing from one place to another, in degrees.
 *
 * The bearing is measured from the north, and it says where to go to get there.
 */
export function bearingBetween(
  fromLat: number,
  fromLng: number,
  toLat: number,
  toLng: number,
): number {
  const lat1 = fromLat * TO_RADIANS;
  const lat2 = toLat * TO_RADIANS;
  const deltaLng = (toLng - fromLng) * TO_RADIANS;

  const y = Math.sin(deltaLng) * Math.cos(lat2);
  const x =
    Math.cos(lat1) * Math.sin(lat2) -
    Math.sin(lat1) * Math.cos(lat2) * Math.cos(deltaLng);

  return (Math.atan2(y, x) / TO_RADIANS + 360) % 360;
}
