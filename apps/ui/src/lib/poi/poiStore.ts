import { create } from "zustand";

import { addPoi, clearPois, pois, removePoi } from "../ipc/bridge";
import type { Poi } from "../ipc/types";

/**
 * The places the operator marked on the map.
 *
 * The sheet edits them and the map draws them, so they live in a store rather than in the sheet:
 * a place that is drawn on the map has to be known by both, and the sheet is not always open.
 * The list is read once when the window starts, and each change is answered by the backend with
 * the list it stored, which is what the map then draws.
 */
export interface PoiStore {
  /** The places, in the order they were added. */
  readonly points: readonly Poi[];
  /** Reads the places from the settings. */
  readonly load: () => Promise<void>;
  /** Adds a place and keeps what the backend says is stored. */
  readonly add: (
    name: string,
    latitude: number,
    longitude: number,
  ) => Promise<void>;
  /** Takes a place off the map. */
  readonly remove: (poi: Poi) => Promise<void>;
  /** Takes every place off the map. */
  readonly clear: () => Promise<void>;
}

export const usePoiStore = create<PoiStore>((set) => ({
  points: [],

  load: async () => {
    try {
      set({ points: await pois() });
    } catch {
      // A window without a backend (the browser run) has no places to read, and that is not a
      // failure of the window.
    }
  },

  add: async (name, latitude, longitude) => {
    set({ points: await addPoi(name, latitude, longitude) });
  },

  remove: async (poi) => {
    set({ points: await removePoi(poi) });
  },

  clear: async () => {
    set({ points: await clearPois() });
  },
}));
