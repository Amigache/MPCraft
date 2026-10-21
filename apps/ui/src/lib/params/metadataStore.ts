import { useEffect, useState } from "react";

import { parameterMetadata } from "../ipc/bridge";
import type { CommandError, ParameterMetadata } from "../ipc/types";

/**
 * What is known about the parameters.
 *
 * The description, the unit, the range and the value a parameter had before anybody
 * changed it are the autopilots' knowledge. The catalogue the backend keeps has them
 * for four thousand parameters, and it is the same for every vehicle: it is read once
 * and then it is only looked up.
 */

/** The metadata of the parameters, by name. */
export type MetadataIndex = ReadonlyMap<string, ParameterMetadata>;

/** The metadata of the parameters that the documentation does not know. */
export const EMPTY_INDEX: MetadataIndex = new Map();

/**
 * Reads the metadata of the given parameters.
 *
 * The names that the documentation does not know are simply absent from the answer: a
 * parameter the vehicle has and the documentation does not is not an error, it is a
 * parameter nobody has written about.
 */
export async function loadMetadata(
  names: readonly string[],
): Promise<MetadataIndex> {
  if (names.length === 0) {
    return EMPTY_INDEX;
  }

  const answers = await parameterMetadata(names);

  const index = new Map<string, ParameterMetadata>();

  answers.forEach((answer, position) => {
    if (answer !== null) {
      index.set(answer.name, answer);
    } else {
      const name = names[position];
      if (name !== undefined) {
        index.set(name, {
          name,
          displayName: name,
          group: "",
          default: "",
          description: "",
          longDescription: "",
          unit: "",
          minimum: "",
          maximum: "",
          values: [],
          vehicles: [],
        });
      }
    }
  });

  return index;
}

/**
 * The metadata of the given parameters, read once.
 *
 * The names are the ones the sheet is showing, and they change as the operator
 * searches, so the catalogue is read once for everything the vehicle has reported and
 * then it is looked up.
 */
export function useMetadata(
  names: readonly string[],
): { readonly index: MetadataIndex; readonly error: CommandError | null } {
  const [index, setIndex] = useState<MetadataIndex>(EMPTY_INDEX);
  const [error, setError] = useState<CommandError | null>(null);

  // The catalogue is read once for the names it is asked for, and again only when the
  // set of names is a different one.
  const key = [...names].sort().join("\n");

  useEffect(() => {
    if (names.length === 0) {
      setIndex(EMPTY_INDEX);
      return;
    }

    let cancelled = false;

    void loadMetadata(names)
      .then((loaded) => {
        if (!cancelled) {
          setIndex(loaded);
          setError(null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause as CommandError);
        }
      });

    return () => {
      cancelled = true;
    };
    // The key is the set of names, in a stable order.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  return { index, error };
}

/**
 * Whether a value is outside the range the documentation gives.
 *
 * A parameter whose value is out of its range is one the vehicle has clamped, or one
 * that was written wrong. The interface says so rather than hiding it.
 */
export function isOutOfRange(
  metadata: ParameterMetadata | undefined,
  value: number | null,
): boolean {
  if (metadata === undefined || value === null) {
    return false;
  }

  const minimum = Number(metadata.minimum);
  const maximum = Number(metadata.maximum);

  const below = Number.isFinite(minimum) && value < minimum;
  const above = Number.isFinite(maximum) && value > maximum;

  return below || above;
}
