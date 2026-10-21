import { create } from "zustand";

import { listMavCommands, normaliseError } from "../ipc/bridge";
import type { CommandError, MavCommand } from "../ipc/types";

/**
 * The catalogue of the commands a plan can hold.
 *
 * The catalogue says what each command's parameters mean, and whether the command
 * carries a position at all. It belongs to the autopilot, so it is read from the
 * backend rather than written out again in the interface.
 *
 * It is read once and kept: the map needs it to know which items of a plan have a
 * place, and the editor needs it to know which parameters a command uses.
 */
export interface CommandStore {
  /** The commands, when the catalogue has been read. */
  readonly commands: readonly MavCommand[];
  /** Whether the catalogue has been read. */
  readonly loaded: boolean;
  /** The last failure, with its stable code. */
  readonly error: CommandError | null;
  /** Reads the catalogue, if it has not been read already. */
  readonly load: () => Promise<void>;
}

export const useCommandStore = create<CommandStore>((set, get) => ({
  commands: [],
  loaded: false,
  error: null,

  load: async () => {
    if (get().loaded) {
      return;
    }

    try {
      const commands = await listMavCommands();
      set({ commands, loaded: true, error: null });
    } catch (cause) {
      set({ error: normaliseError(cause) });
    }
  },
}));
