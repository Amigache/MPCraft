import { create } from "zustand";

import type { LogTrackPoint } from "../ipc/types";

/**
 * The path a log holds, and the log it was read from.
 *
 * The screen reads the log and the map draws it, so the path is kept in a store rather than in the
 * state of the screen: the grid, the graph and the map are the same flight seen from three sides,
 * and that only holds when the line on the ground is the whole path of the log rather than the
 * part of it that happened to be played. A row clicked at two minutes moves the play-head to two
 * minutes; a map that had only drawn the first thirty seconds would show a moment on a line that
 * is not there.
 *
 * The path is kept with the file it came from. A path read for one log is not drawn under another,
 * because a line on the ground that came from a different flight says something the operator did
 * not open.
 */
export interface LogTrackStore {
  /** The file the places were read from, empty when no log is open. */
  readonly log: string;
  /** The places of the track, in the order the log holds them. */
  readonly points: readonly LogTrackPoint[];

  /** Puts a path in the store, with the file it was read from. */
  readonly setTrack: (log: string, points: readonly LogTrackPoint[]) => void;
  /** Lets go of the path. */
  readonly clear: () => void;
}

/**
 * The coordinates a line is drawn from.
 *
 * A coordinate of GeoJSON is longitude first and latitude second, which is the other way round
 * from the fields of a point, so this is where the order is decided — once, rather than at every
 * place that draws. A point whose number is not a place is not part of the line: a record of a log
 * can carry a position of zeroes, and a line through it runs to the middle of the ocean.
 */
export function lineOf(points: readonly LogTrackPoint[]): [number, number][] {
  return points
    .filter(
      (point) =>
        Number.isFinite(point.latitude) && Number.isFinite(point.longitude),
    )
    .map((point) => [point.longitude, point.latitude]);
}

export const useLogTrackStore = create<LogTrackStore>((set) => ({
  log: "",
  points: [],

  setTrack: (log, points) => set({ log, points }),
  clear: () => set({ log: "", points: [] }),
}));
