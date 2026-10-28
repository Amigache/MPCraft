import { create } from "zustand";

import {
  listParameters,
  normaliseError,
  readParameters,
  writeParameter,
  writeParametersBatch,
} from "../ipc/bridge";
import type { CommandError, ParamChange, Parameter } from "../ipc/types";

/**
 * The parameters of the vehicle.
 *
 * The list is the truth about what the vehicle holds: a row draws what the store says,
 * and the store is filled from the answers the vehicle gives. A value that changes on
 * its own — the vehicle reports a parameter that has been changed elsewhere — comes
 * back through the same door.
 */

/** A parameter, as the list shows it. */
export interface KnownParameter {
  /** The name of the parameter. */
  readonly name: string;
  /** The value, or nothing when the vehicle has not reported one. */
  readonly value: number | null;
  /** The type the vehicle reported, as the MAVLink enumeration. */
  readonly reportedType: number;
}

/** What happened the last time a parameter was written. */
export type WriteOutcome =
  | { readonly kind: "written"; readonly parameter: KnownParameter }
  | {
      readonly kind: "failed";
      readonly code: string;
      readonly message: string;
    };

export interface ParamsStore {
  /** The parameters the vehicle has reported, by name. */
  readonly parameters: readonly KnownParameter[];
  /** Whether the whole list is being read. */
  readonly reading: boolean;
  /** The name of the parameter being written, when one is. */
  readonly writing: string | null;
  /** What the last write did. */
  readonly lastWrite: WriteOutcome | null;
  /** The last failure of a read, with its stable code. */
  readonly error: CommandError | null;
  /**
   * The parameters the operator has changed in this session.
   *
   * They are the ones that can be put back as they were: the value the
   * documentation gives as the default of each of them.
   */
  readonly changed: readonly string[];
  /** Reads the parameters the vehicle has reported so far. */
  readonly refresh: () => Promise<void>;
  /** Asks the vehicle for every parameter it has, and waits for the answers. */
  readonly readAll: () => Promise<void>;
  /** Writes one parameter and waits for the vehicle to confirm it. */
  readonly write: (name: string, value: number) => Promise<void>;
  /**
   * Writes several parameters, in the order the vehicle needs them.
   *
   * The order is the autopilots' knowledge and the backend applies it.
   */
  readonly writeBatch: (changes: readonly ParamChange[]) => Promise<void>;
}

/**
 * Turns what the backend sent into what the list draws.
 *
 * The value on the wire is a floating point number, and a floating point number that is
 * not a number — a `NaN` — arrives as nothing at all. Such a value is not a zero: it is
 * a value the vehicle has not reported.
 */
function known(parameter: Parameter): KnownParameter {
  return {
    name: parameter.name,
    value: Number.isFinite(parameter.value) ? parameter.value : null,
    reportedType: parameter.reportedType,
  };
}

/** The parameters in the order they are shown: by name. */
function byName(parameters: readonly Parameter[]): readonly KnownParameter[] {
  return parameters
    .map(known)
    .slice()
    .sort((left, right) =>
      left.name < right.name ? -1 : left.name > right.name ? 1 : 0,
    );
}

/**
 * Puts a fresh list of parameters into the old one.
 *
 * The list is read again and again, and most of the time it has not changed. The
 * parameters that have not changed keep their identity, so that the rows that draw them
 * are not drawn again for nothing.
 */
export function mergeParameters(
  previous: readonly KnownParameter[],
  incoming: readonly KnownParameter[],
): readonly KnownParameter[] {
  const before = new Map(
    previous.map((parameter) => [parameter.name, parameter]),
  );
  let changed = previous.length !== incoming.length;

  const merged = incoming.map((parameter) => {
    const old = before.get(parameter.name);
    if (
      old !== undefined &&
      old.value === parameter.value &&
      old.reportedType === parameter.reportedType
    ) {
      return old;
    }

    changed = true;
    return parameter;
  });

  return changed ? merged : previous;
}

/** Puts one parameter back into the list, where it was. */
export function replaceParameter(
  previous: readonly KnownParameter[],
  parameter: KnownParameter,
): readonly KnownParameter[] {
  const index = previous.findIndex((entry) => entry.name === parameter.name);

  if (index < 0) {
    return [...previous, parameter].sort((left, right) =>
      left.name < right.name ? -1 : left.name > right.name ? 1 : 0,
    );
  }

  const old = previous[index];
  if (
    old !== undefined &&
    old.value === parameter.value &&
    old.reportedType === parameter.reportedType
  ) {
    return previous;
  }

  const next = previous.slice();
  next[index] = parameter;
  return next;
}

export const useParamsStore = create<ParamsStore>((set) => ({
  parameters: [],
  reading: false,
  writing: null,
  lastWrite: null,
  error: null,
  changed: [],

  refresh: async () => {
    try {
      const parameters = byName(await listParameters());
      set((state) => ({
        parameters: mergeParameters(state.parameters, parameters),
        error: null,
      }));
    } catch (cause) {
      set({ error: normaliseError(cause) });
    }
  },

  readAll: async () => {
    set({ reading: true, error: null });
    try {
      const parameters = byName(await readParameters());
      set((state) => ({
        parameters: mergeParameters(state.parameters, parameters),
        reading: false,
      }));
    } catch (cause) {
      set({ reading: false, error: normaliseError(cause) });
    }
  },

  write: async (name, value) => {
    set({ writing: name, lastWrite: null });
    try {
      // The answer carries the value the vehicle stored, which is not always the value
      // that was asked for: the vehicle clamps to its own range. What the vehicle holds
      // is what is shown.
      const stored = known(await writeParameter(name, value));
      set((state) => ({
        parameters: replaceParameter(state.parameters, stored),
        writing: null,
        changed: state.changed.includes(name)
          ? state.changed
          : [...state.changed, name],
        lastWrite: { kind: "written", parameter: stored },
      }));
    } catch (cause) {
      const error = normaliseError(cause);
      set({
        writing: null,
        lastWrite: { kind: "failed", code: error.code, message: error.message },
      });
    }
  },

  writeBatch: async (changes) => {
    if (changes.length === 0) {
      return;
    }

    set({ writing: "the batch", lastWrite: null });

    try {
      const stored = (await writeParametersBatch(changes)).map(known);

      set((state) => {
        let parameters = state.parameters;

        for (const parameter of stored) {
          parameters = replaceParameter(parameters, parameter);
        }

        const changed = [...state.changed];

        for (const change of changes) {
          if (!changed.includes(change.name)) {
            changed.push(change.name);
          }
        }

        return {
          parameters,
          writing: null,
          changed,
          lastWrite:
            stored.length > 0
              ? { kind: "written", parameter: stored[0] as KnownParameter }
              : null,
        };
      });
    } catch (cause) {
      const error = normaliseError(cause);
      set({
        writing: null,
        lastWrite: { kind: "failed", code: error.code, message: error.message },
      });
    }
  },
}));
