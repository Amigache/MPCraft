import type { StyleSpecification } from "maplibre-gl";

/**
 * The providers of the map.
 *
 * Mission Planner offers a registry of sources: the ordinary map, the satellite one and
 * the hybrid, for every provider it knows. MPCraft offers the sources that are free and
 * that need neither an account nor a key, because a ground station that asks for a key
 * before it draws a map is a ground station that does not draw a map.
 *
 * The choice is the operator's, and it is kept between two runs.
 */

/** One source of tiles. */
export interface MapProvider {
  /** The identifier the choice is kept under. */
  readonly id: string;
  /** The name the operator reads it with. */
  readonly name: string;
  /** The address of the style, for a map of vectors. */
  readonly styleUrl: string | null;
  /** The address of the tiles, for a map of pictures. */
  readonly tiles: string | null;
  /** The copyright the source asks to be shown. */
  readonly attribution: string;
}

/** The providers, in the order they are offered. */
export const PROVIDERS: readonly MapProvider[] = [
  {
    id: "openfreemap-liberty",
    name: "OpenFreeMap Liberty",
    styleUrl: "https://tiles.openfreemap.org/styles/liberty",
    tiles: null,
    attribution: "© OpenFreeMap contributors",
  },
  {
    id: "openfreemap-bright",
    name: "OpenFreeMap Bright",
    styleUrl: "https://tiles.openfreemap.org/styles/bright",
    tiles: null,
    attribution: "© OpenFreeMap contributors",
  },
  {
    id: "openfreemap-positron",
    name: "OpenFreeMap Positron",
    styleUrl: "https://tiles.openfreemap.org/styles/positron",
    tiles: null,
    attribution: "© OpenFreeMap contributors",
  },
  {
    id: "openstreetmap",
    name: "OpenStreetMap",
    styleUrl: null,
    tiles: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
    attribution: "© OpenStreetMap contributors",
  },
  {
    id: "opentopomap",
    name: "OpenTopoMap",
    styleUrl: null,
    tiles: "https://a.tile.opentopomap.org/{z}/{x}/{y}.png",
    attribution: "© OpenTopoMap contributors, © OpenStreetMap contributors",
  },
  {
    id: "esri-imagery",
    name: "Esri World Imagery",
    styleUrl: null,
    tiles: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    attribution: "© Esri, Maxar, Earthstar Geographics",
  },
  {
    id: "cartodb-light",
    name: "CartoDB Light",
    styleUrl: null,
    tiles: "https://basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png",
    attribution: "© OpenStreetMap contributors, © CARTO",
  },
];

/** The provider that is used when nothing has been chosen. */
export const DEFAULT_PROVIDER = "openfreemap-liberty";

/** The provider with the given identifier, when there is one. */
export function providerOf(id: string | null | undefined): MapProvider {
  const found = PROVIDERS.find((provider) => provider.id === id);
  return found ?? (PROVIDERS[0] as MapProvider);
}

/**
 * The style of a provider.
 *
 * A map of vectors is a style the server gives; a map of pictures is a style of one
 * layer of tiles, which the library draws over the background.
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
        tiles: [provider.tiles ?? ""],
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
