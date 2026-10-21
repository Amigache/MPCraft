import { create } from "zustand";

import {
  listConnectionProfiles,
  setConnectionProfiles,
} from "../ipc/bridge";
import type { CommandError, ConnectionProfile } from "../ipc/types";

/**
 * The connections the operator has named.
 *
 * The profiles are kept by the backend, between two runs, and they are the links an
 * operator comes back to: the vehicle on the bench, the simulator, the drone in the
 * field. A profile that is marked to be opened at the launch is opened when the
 * application starts.
 */
export interface ProfilesStore {
  /** The profiles, in the order they were saved. */
  readonly profiles: readonly ConnectionProfile[];
  /** The failure of the last operation, with its stable code. */
  readonly error: CommandError | null;
  /** Whether an operation is in flight. */
  readonly busy: boolean;
  /** Reads the profiles. */
  readonly load: () => Promise<void>;
  /** Saves a profile, replacing the one with the same name. */
  readonly save: (profile: ConnectionProfile) => Promise<void>;
  /** Forgets a profile. */
  readonly remove: (name: string) => Promise<void>;
  /** Says whether a profile is opened at the launch. */
  readonly setAutoConnect: (name: string, autoConnect: boolean) => Promise<void>;
}

export const useProfilesStore = create<ProfilesStore>((set, get) => ({
  profiles: [],
  error: null,
  busy: false,

  load: async () => {
    set({ busy: true, error: null });

    try {
      const profiles = await listConnectionProfiles();
      set({ profiles, busy: false });
    } catch (cause) {
      set({
        busy: false,
        error: cause as CommandError,
      });
    }
  },

  save: async (profile) => {
    // The profile that is saved replaces the one with the same name: a name is what
    // tells two profiles apart.
    const others = get().profiles.filter((known) => known.name !== profile.name);

    set({ busy: true, error: null });

    try {
      await setConnectionProfiles([...others, profile]);
      set({ profiles: [...others, profile], busy: false });
    } catch (cause) {
      set({ busy: false, error: cause as CommandError });
    }
  },

  remove: async (name) => {
    const remaining = get().profiles.filter((known) => known.name !== name);

    set({ busy: true, error: null });

    try {
      await setConnectionProfiles(remaining);
      set({ profiles: remaining, busy: false });
    } catch (cause) {
      set({ busy: false, error: cause as CommandError });
    }
  },

  setAutoConnect: async (name, autoConnect) => {
    const changed = get().profiles.map((profile) =>
      profile.name === name ? { ...profile, autoConnect } : profile,
    );

    set({ busy: true, error: null });

    try {
      await setConnectionProfiles(changed);
      set({ profiles: changed, busy: false });
    } catch (cause) {
      set({ busy: false, error: cause as CommandError });
    }
  },
}));
