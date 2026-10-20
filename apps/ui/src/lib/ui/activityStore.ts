import { create } from "zustand";

import type { CommandError } from "../ipc/types";

/**
 * What the interface has to say about the last thing that happened.
 *
 * A command that fails does not throw at the operator: it leaves a note here, which the
 * status bar shows and the panels that ran it show in full. The failure is kept with its
 * stable code, so the interface can decide what to do with it without reading the
 * message.
 */
export interface ActivityStore {
  /** The last failure, with its stable code. */
  readonly error: CommandError | null;
  /** The last thing that worked. */
  readonly notice: string | null;
  /** Records a failure, or a success. */
  readonly report: (error: CommandError | null, notice: string | null) => void;
  /** Forgets both. */
  readonly clear: () => void;
}

export const useActivityStore = create<ActivityStore>((set) => ({
  error: null,
  notice: null,
  report: (error, notice) => set({ error, notice }),
  clear: () => set({ error: null, notice: null }),
}));
