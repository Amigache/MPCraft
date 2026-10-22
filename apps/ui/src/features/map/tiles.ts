import maplibregl from "maplibre-gl";

import { mapTile } from "../../lib/ipc/bridge";
import { TILE_SCHEME } from "./providers";

/**
 * The tiles of the map.
 *
 * The map does not ask the service for the tiles: it asks the backend, which takes them
 * from the folder the operator has brought, from the disk where the tiles that were
 * fetched are kept, and from the service when it is in neither.
 *
 * That is what makes a map work where there is no signal: the tiles of the field are
 * fetched before the flight, and the map is drawn with the ones that are there.
 *
 * The scheme is registered once: the library keeps the handlers it is given, and a
 * second registration would be a second handler for the same tiles.
 */

let registered = false;

/** Registers the handler of the tiles, once. */
export function registerTileProtocol(): void {
  if (registered) {
    return;
  }

  registered = true;

  maplibregl.addProtocol(TILE_SCHEME, async (params) => {
    // The address of a tile is the scheme, the source and the place on the grid.
    const rest = params.url.split("://")[1] ?? "";
    const parts = rest.split("/");

    const provider = parts[0] ?? "";
    const zoom = Number(parts[1]);
    const x = Number(parts[2]);
    const y = Number((parts[3] ?? "").split(".")[0]);

    if (
      provider === "" ||
      !Number.isFinite(zoom) ||
      !Number.isFinite(x) ||
      !Number.isFinite(y)
    ) {
      throw new Error(`the address of the tile is not one that can be used: ${params.url}`);
    }

    const bytes = await mapTile(provider, zoom, x, y);
    const picture = new Uint8Array(bytes);

    return {
      data: picture.buffer.slice(
        picture.byteOffset,
        picture.byteOffset + picture.byteLength,
      ),
    };
  });
}
