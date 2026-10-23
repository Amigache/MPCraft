import { create } from "zustand";

import type { PicturePlace } from "../ipc/types";

/**
 * The pictures of a folder, and the log they are matched with.
 *
 * The sheet reads the list and the map draws it, so the pictures are kept in a store rather
 * than in the state of the sheet: a picture that is on the map has to be known by both, and a
 * sheet that is closed does not keep its own state.
 */
export interface PictureStore {
  /** The folder the pictures are read from. */
  readonly folder: string;
  /** The log the pictures are matched with. */
  readonly log: string;
  /**
   * The difference between the clock of the camera and the clock of the log, in seconds.
   *
   * It is what was worked out from the ends of the folder, and the operator can put it right
   * when the folder is too small for the estimate to say anything.
   */
  readonly offset: number;
  /** What the camera took between being told to take a picture and taking it, in seconds. */
  readonly lag: number;
  /** The pictures, in the order they were made. */
  readonly pictures: readonly PicturePlace[];

  /** Chooses the folder the pictures are read from. */
  readonly setFolder: (folder: string) => void;
  /** Chooses the log the pictures are matched with. */
  readonly setLog: (log: string) => void;
  /** Sets the difference between the two clocks, in seconds. */
  readonly setOffset: (offset: number) => void;
  /** Sets what the camera took between being told and taking the picture, in seconds. */
  readonly setLag: (lag: number) => void;
  /** Puts the pictures in the store. */
  readonly setPictures: (pictures: readonly PicturePlace[]) => void;
}

export const usePictureStore = create<PictureStore>((set) => ({
  folder: "",
  log: "",
  offset: 0,
  lag: 0,
  pictures: [],

  setFolder: (folder) => set({ folder }),
  setLog: (log) => set({ log }),
  setOffset: (offset) => set({ offset }),
  setLag: (lag) => set({ lag }),
  setPictures: (pictures) => set({ pictures }),
}));
