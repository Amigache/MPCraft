import { create } from "zustand";

import { listFlightModes, normaliseError } from "../ipc/bridge";
import type { CommandError, FlightMode } from "../ipc/types";

/**
 * The flight modes the vehicle can be in.
 *
 * The list comes from the backend, which owns the numbering: the interface never invents
 * a mode number. It asks once and keeps the answer.
 */
export interface ModesStore {
  /** The modes the vehicle offers. */
  readonly modes: readonly FlightMode[];
  /** Whether the list has been read. */
  readonly loaded: boolean;
  /** The last failure, with its stable code. */
  readonly error: CommandError | null;
  /** Reads the list from the backend. */
  readonly load: () => Promise<void>;
}

export const useModesStore = create<ModesStore>((set) => ({
  modes: [],
  loaded: false,
  error: null,

  load: async () => {
    try {
      const modes = await listFlightModes();
      set({ modes, loaded: true, error: null });
    } catch (cause) {
      set({ loaded: true, error: normaliseError(cause) });
    }
  },
}));

/**
 * The number of a mode, by its name.
 *
 * The names are matched without regard to case, because the backend is free to spell them
 * the way the vehicle does.
 */
export function modeValue(
  modes: readonly FlightMode[],
  name: string,
): number | null {
  const wanted = name.trim().toLowerCase();
  const found = modes.find((mode) => mode.name.trim().toLowerCase() === wanted);
  return found ? found.value : null;
}
