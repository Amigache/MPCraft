/**
 * The style of the map.
 *
 * The demo tiles of MapLibre are used, so that the map works without an account and
 * without a key. This is the one place the address is written down: change it here and
 * the whole application follows.
 */
export const MAP_STYLE_URL = "https://demotiles.maplibre.org/style.json";

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
