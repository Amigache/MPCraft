import { create } from "zustand";

import {
  clearMission,
  downloadMission,
  normaliseError,
  uploadMission,
} from "../ipc/bridge";
import type { CommandError, Plan, PlanItem, PlanKind } from "../ipc/types";

/**
 * The plans the vehicle holds.
 *
 * A vehicle holds three plans: the mission it flies, the fence it will not leave and the
 * rally points it comes back to. They are downloaded from the vehicle and uploaded back
 * to it, and they are kept in a store because three parts of the interface show them:
 * the actions panel lists the items, the status panel reports the progress, and the map
 * draws them.
 */

/** The plans, none of them read yet. */
const NO_PLANS: Readonly<Record<PlanKind, Plan | null>> = {
  mission: null,
  fence: null,
  rally: null,
};

export interface PlanStore {
  /** The plan of each kind, when it has been downloaded. */
  readonly plans: Readonly<Record<PlanKind, Plan | null>>;
  /** The kind of plan the interface is working on. */
  readonly kind: PlanKind;
  /** Whether a plan operation is in flight. */
  readonly busy: boolean;
  /** The last failure, with its stable code. */
  readonly error: CommandError | null;
  /** Downloads a plan from the vehicle, and gives it back. */
  readonly download: (kind: PlanKind) => Promise<Plan | null>;
  /** Uploads a plan to the vehicle. */
  readonly upload: (kind: PlanKind, items: readonly PlanItem[]) => Promise<void>;
  /** Empties one of the plans the vehicle holds. */
  readonly clear: (kind: PlanKind) => Promise<void>;
  /** Chooses the kind of plan the interface works on. */
  readonly setKind: (kind: PlanKind) => void;
  /** Forgets the last failure. */
  readonly dismissError: () => void;
}

export const usePlanStore = create<PlanStore>((set) => ({
  plans: NO_PLANS,
  kind: "mission",
  busy: false,
  error: null,

  setKind: (kind) => set({ kind }),

  download: async (kind) => {
    set({ busy: true, error: null });
    try {
      const plan = await downloadMission(kind);
      set((state) => ({
        plans: { ...state.plans, [kind]: plan },
        busy: false,
      }));
      return plan;
    } catch (cause) {
      set({ busy: false, error: normaliseError(cause) });
      return null;
    }
  },

  upload: async (kind, items) => {
    set({ busy: true, error: null });
    try {
      await uploadMission(items, kind);
      set({ busy: false });
    } catch (cause) {
      set({ busy: false, error: normaliseError(cause) });
    }
  },

  clear: async (kind) => {
    set({ busy: true, error: null });
    try {
      await clearMission(kind);
      set((state) => ({
        plans: { ...state.plans, [kind]: null },
        busy: false,
      }));
    } catch (cause) {
      set({ busy: false, error: normaliseError(cause) });
    }
  },

  dismissError: () => set({ error: null }),
}));

/**
 * The item of a plan the vehicle is executing.
 *
 * The backend marks the item in the plan it sends, so the answer is read from there and
 * not derived.
 */
export function currentItem(plan: Plan | null): PlanItem | null {
  if (!plan) {
    return null;
  }
  return plan.items.find((item) => item.current) ?? null;
}

/** The number of the item the vehicle is executing, counted from one. */
export function currentItemNumber(plan: Plan | null): number | null {
  const item = currentItem(plan);
  if (!item) {
    return null;
  }
  return item.seq + 1;
}
