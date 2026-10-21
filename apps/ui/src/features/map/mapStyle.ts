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
