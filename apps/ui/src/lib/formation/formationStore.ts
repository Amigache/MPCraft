import { create } from "zustand";

import type { FormationSlot, FormationStatus } from "../ipc/types";

/**
 * The formation as the sheet holds it.
 *
 * Two things are kept apart here for the same reason they are kept apart in the file: the places
 * an operator wrote, and the decision to run them. Writing a fleet is not sending orders to a
 * fleet, so the store holds what was written and shows what the backend says about it — including
 * the names it could not read as vehicles, which stay where the operator can see them.
 *
 * The store does not decide which names are vehicles. That is the layer that speaks the protocol;
 * what this file does is keep what was typed and say which of those names the backend left out.
 */
export interface FormationStore {
  /** Who leads, named the way the file names it. */
  readonly leader: string;
  /** Whether the operator decided to run the loop. */
  readonly enabled: boolean;
  /** The places, in the order they were written. */
  readonly slots: readonly FormationSlot[];

  /** Whether the loop is running now, which is what the backend says rather than what was asked for. */
  readonly running: boolean;
  /** How many vehicles the last tick told. */
  readonly told: number;
  /** The names the backend could not read as vehicles. */
  readonly refused: readonly string[];
  /** The last failure of the loop, when there was one. */
  readonly error: string | null;

  /** Chooses who leads. */
  readonly setLeader: (leader: string) => void;
  /** Decides whether the loop is run. */
  readonly setEnabled: (enabled: boolean) => void;
  /** Puts the places in the store. */
  readonly setSlots: (slots: readonly FormationSlot[]) => void;
  /** Adds a vehicle with a place at the leader's own position. */
  readonly addSlot: (vehicle: string) => void;
  /** Takes a vehicle out of the formation. */
  readonly removeSlot: (vehicle: string) => void;
  /** Moves one number of one slot. */
  readonly setOffset: (
    vehicle: string,
    axis: "x" | "y" | "z",
    metres: number,
  ) => void;
  /** Puts what the backend says into the store. */
  readonly apply: (status: FormationStatus) => void;
}

/** Names a vehicle the way the file and the link name it: `system:component`. */
export function vehicleName(systemId: number, componentId: number): string {
  return `${systemId}:${componentId}`;
}

export const useFormationStore = create<FormationStore>((set) => ({
  leader: "",
  enabled: false,
  slots: [],

  running: false,
  told: 0,
  refused: [],
  error: null,

  setLeader: (leader) => set({ leader }),
  setEnabled: (enabled) => set({ enabled }),
  setSlots: (slots) => set({ slots }),

  addSlot: (vehicle) =>
    set((state) => {
      if (state.slots.some((slot) => slot.vehicle === vehicle)) {
        return state;
      }

      return { slots: [...state.slots, { vehicle, x: 0, y: 0, z: 0 }] };
    }),

  removeSlot: (vehicle) =>
    set((state) => ({
      slots: state.slots.filter((slot) => slot.vehicle !== vehicle),
    })),

  setOffset: (vehicle, axis, metres) =>
    set((state) => ({
      slots: state.slots.map((slot) =>
        slot.vehicle === vehicle ? { ...slot, [axis]: metres } : slot,
      ),
    })),

  apply: (status) =>
    set({
      leader: status.leader ?? "",
      enabled: status.enabled,
      slots: status.slots,
      running: status.running,
      told: status.told,
      refused: status.refused,
      error: status.error,
    }),
}));
