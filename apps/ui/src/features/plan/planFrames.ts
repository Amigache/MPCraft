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
  { value: 0, label: "Above the sea" },
  { value: 3, label: "Above the home" },
  { value: 10, label: "Above the ground" },
];

/** The frame a new item is given when there is nothing to copy it from. */
export const DEFAULT_FRAME = 3;

/** How a part of the interface says something in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

/** The name of a frame, when the frame is one of the ones offered. */
export function labelOfFrame(value: number, t: Say): string {
  const frame = FRAMES.find((entry) => entry.value === value);

  return frame ? t(frame.label) : t("Frame {number}", { number: value });
}
