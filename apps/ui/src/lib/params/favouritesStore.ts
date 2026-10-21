import { create } from "zustand";

import {
  listFavouriteParameters,
  normaliseError,
  setFavouriteParameter,
} from "../ipc/bridge";
import type { CommandError } from "../ipc/types";
import { compareNames } from "./names";

/**
 * The parameters the operator has marked as favourites.
 *
 * Upstream keeps a column of stars in its parameter window, and the favourites are worth
 * keeping between two runs: the parameters an operator touches are the ones they touch
 * every time.
 *
 * The backend keeps them, in the order they were marked, and the interface shows them
 * first, in that order. The rest of the parameters follow, by name.
 *
 * The mark is put on the row at once, and taken off again when the backend refuses: a
 * star that shows a state the backend does not have is a lie, and the failure is said
 * out loud in the footer of the sheet, next to the outcome of the last write.
 */

export interface FavouritesStore {
  /** The names of the favourites, in the order the operator marked them. */
  readonly favourites: readonly string[];
  /** Whether the favourites are being read. */
  readonly loading: boolean;
  /** The last failure, with its stable code. */
  readonly error: CommandError | null;
  /** Reads the favourites the backend keeps. */
  readonly load: () => Promise<void>;
  /** Marks a parameter as a favourite, or takes the mark away. */
  readonly toggle: (name: string) => Promise<void>;
}

/**
 * The list with a name marked in it, or taken out of it.
 *
 * A name that is marked again goes to the end of the list, which is what the backend
 * does: the order of the list is the order the operator marked them in.
 */
export function markFavourite(
  favourites: readonly string[],
  name: string,
  favourite: boolean,
): readonly string[] {
  const without = favourites.filter((entry) => entry !== name);

  return favourite ? [...without, name] : without;
}

/**
 * The list as it was before a mark that the backend refused.
 *
 * Only the name that was touched is put back: another mark that has been made in the
 * meantime is left alone.
 */
function restoreFavourite(
  favourites: readonly string[],
  name: string,
  wasFavourite: boolean,
  index: number,
): readonly string[] {
  const without = favourites.filter((entry) => entry !== name);

  if (!wasFavourite) {
    return without;
  }

  const restored = [...without];
  restored.splice(Math.min(Math.max(index, 0), restored.length), 0, name);

  return restored;
}

/**
 * The parameters in the order they are shown.
 *
 * The favourites come first, in the order the operator marked them, and the rest follow
 * by name. Upstream does the same, and says why: the favourites are the ones the
 * operator is looking for.
 */
export function orderParameters<T extends { readonly name: string }>(
  parameters: readonly T[],
  favourites: readonly string[],
): readonly T[] {
  const rank = new Map(favourites.map((name, index) => [name, index]));

  return [...parameters].sort((left, right) => {
    const leftRank = rank.get(left.name);
    const rightRank = rank.get(right.name);

    if (leftRank !== undefined && rightRank !== undefined) {
      return leftRank - rightRank;
    }

    if (leftRank !== undefined) {
      return -1;
    }

    if (rightRank !== undefined) {
      return 1;
    }

    return compareNames(left.name, right.name);
  });
}

export const useFavouritesStore = create<FavouritesStore>((set, get) => ({
  favourites: [],
  loading: false,
  error: null,

  load: async () => {
    set({ loading: true });
    try {
      const favourites = await listFavouriteParameters();
      set({ favourites: [...favourites], loading: false, error: null });
    } catch (cause) {
      set({ loading: false, error: normaliseError(cause) });
    }
  },

  toggle: async (name) => {
    const before = get().favourites;
    const index = before.indexOf(name);
    const wasFavourite = index >= 0;

    // The star is turned at once: the operator asked for it, and the backend is told
    // afterwards. When the backend refuses, the star goes back to what it was.
    set({
      favourites: markFavourite(before, name, !wasFavourite),
      error: null,
    });

    try {
      await setFavouriteParameter(name, !wasFavourite);
    } catch (cause) {
      set((state) => ({
        favourites: restoreFavourite(state.favourites, name, wasFavourite, index),
        error: normaliseError(cause),
      }));
    }
  },
}));
