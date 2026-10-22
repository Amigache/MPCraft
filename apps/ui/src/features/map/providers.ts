import type { StyleSpecification } from "maplibre-gl";

import { listMapProviders } from "../../lib/ipc/bridge";
import type { MapProvider } from "../../lib/ipc/types";

/**
 * The providers of the map.
 *
 * Mission Planner offers a registry of sources: the ordinary map, the satellite one and
 * the hybrid, for every provider it knows. MPCraft offers the sources that are free and
 * that need neither an account nor a key, because a ground station that asks for a key
 * before it draws a map is a ground station that does not draw a map.
 *
 * The list is the backend's, because the backend is the one that fetches the tiles: the
 * map asks it for the tiles it needs, and the tiles that are on the disk are the ones
 * that are drawn.
 *
 * The choice is the operator's, and it is kept between two runs.
 */

/**
 * The scheme the tiles are asked for with.
 *
 * The map does not ask the service for the tiles: it asks the backend, which takes them
 * from the folder the operator has brought, from the disk where the ones that were
 * fetched are kept, and from the service when it is in neither.
 */
export const TILE_SCHEME = "mpcraft";

/** The providers, as they were read. */
let providers: readonly MapProvider[] = [];

/** The providers that are known.
 *
 * The list is empty until it is read, and the map waits for it.
 */
export function knownProviders(): readonly MapProvider[] {
  return providers;
}

/** Reads the list of providers from the backend.
 *
 * The list is read once: the sources of tiles do not change while the application is
 * running.
 */
export async function loadProviders(): Promise<readonly MapProvider[]> {
  providers = await listMapProviders();

  return providers;
}

/** The provider that is used when nothing has been chosen. */
export const DEFAULT_PROVIDER = "openfreemap-liberty";

/** The provider with the given identifier, when there is one. */
export function providerOf(id: string | null | undefined): MapProvider | null {
  const found = providers.find((provider) => provider.id === id);

  return found ?? providers[0] ?? null;
}

/**
 * The style of a provider.
 *
 * A map of vectors is a style the server gives; a map of pictures is a style of one
 * layer of tiles, which the library draws over the background.
 *
 * The tiles of the second kind are asked for with the local scheme, so that the backend
 * can answer them from the disk.
 */
export function styleOf(provider: MapProvider): string | StyleSpecification {
  if (provider.styleUrl !== null) {
    return provider.styleUrl;
  }

  return {
    version: 8,
    sources: {
      tiles: {
        type: "raster",
        tiles: [`${TILE_SCHEME}://${provider.id}/{z}/{x}/{y}.png`],
        tileSize: 256,
        attribution: provider.attribution,
      },
    },
    layers: [
      {
        id: "tiles",
        type: "raster",
        source: "tiles",
      },
    ],
  };
}
