/**
 * The coordinate frames the editor offers.
 *
 * Mission Planner names the three it offers in its own words, and the operator knows
 * those words, so they are the words used here. The values are the ones that travel on
 * the wire; the backend takes them as they are and does not have to guess what the
 * operator meant.
 */

/** One coordinate frame. */
export interface Frame {
  /** The value the frame travels with. */
  readonly value: number;
  /** The name the operator knows the frame by. */
  readonly label: string;
}

/** The frames a plan item can be expressed in. */
export const FRAMES: readonly Frame[] = [
  { value: 0, label: "Global (AMSL)" },
  { value: 3, label: "Relative (AGL)" },
  { value: 10, label: "Terrain" },
];

/** The frame a new item is given when there is nothing to copy it from. */
export const DEFAULT_FRAME = 3;

/** The name of a frame, when the frame is one of the ones offered. */
export function labelOfFrame(value: number): string {
  return FRAMES.find((frame) => frame.value === value)?.label ?? `Frame ${value}`;
}
