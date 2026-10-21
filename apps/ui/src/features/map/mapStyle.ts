/**
 * The style of the map.
 *
 * The tiles are OpenFreeMap's, which are free and need neither an account nor a key. The
 * style is the classic one: roads, buildings and labels, the way an operator expects a
 * map to look.
 *
 * This is the one place the address is written down: change it here and the whole
 * application follows.
 */
export const MAP_STYLE_URL = "https://tiles.openfreemap.org/styles/liberty";

/**
 * The centre the map opens at, when the vehicle has not said where it is.
 *
 * The order is the one the map library uses: the longitude first, the latitude second.
 */
export const DEFAULT_CENTRE: readonly [number, number] = [149.13, -35.2809];

/** The zoom the map opens at. */
export const DEFAULT_ZOOM = 14;

/** How many positions the track keeps. */
export const TRACK_LIMIT = 2000;
