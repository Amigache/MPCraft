import { DASH } from "../format";
import { isIntegralType } from "./paramTypes";

/**
 * The value of a parameter, or nothing when the vehicle has not reported one.
 *
 * The two are not the same thing. A parameter whose value is zero is a parameter whose
 * value is zero; a parameter the vehicle has not reported is a parameter the interface
 * knows nothing about, and it says so with a dash.
 */
export type ParameterValue = number | null;

/**
 * How many digits a real is shown with.
 *
 * A single-precision float carries about seven digits of meaning and a double about
 * fifteen. Showing more of them than that is showing noise.
 */
const DIGITS: Readonly<Record<number, number>> = {
  9: 7,
  10: 15,
};

/**
 * The value as the list shows it.
 *
 * A parameter the vehicle has not reported shows a dash. A parameter whose value is
 * zero shows a zero. Merging the two is how a ground station starts to lie.
 */
export function formatParameterValue(
  value: ParameterValue,
  reportedType: number,
): string {
  if (value === null || !Number.isFinite(value)) {
    return DASH;
  }

  if (isIntegralType(reportedType)) {
    // The vehicle reports a whole number for a whole-numbered type. It is shown as it
    // stands, so that a vehicle which reports something else does not go unnoticed.
    return String(value);
  }

  const digits = DIGITS[reportedType] ?? 15;
  return String(Number(value.toPrecision(digits)));
}

/** What came of reading what the operator typed. */
export type ParsedValue =
  | { readonly ok: true; readonly value: number }
  | { readonly ok: false; readonly reason: string };

/**
 * A number as it may be written: an optional sign, digits, an optional fraction and an
 * optional exponent.
 *
 * Nothing else is a number: not `0x10`, not `1,5`, not `Infinity`, not `NaN`.
 */
const NUMBER = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

/**
 * Reads what the operator typed.
 *
 * A parameter of a whole-numbered type accepts only a whole number. A fraction is
 * refused rather than rounded: a value the operator did not ask for is a value the
 * vehicle was never told about, and rounding it behind their back is a lie of the same
 * family as showing a zero for a value that is not known.
 */
export function parseParameterValue(
  draft: string,
  reportedType: number,
): ParsedValue {
  const text = draft.trim();

  if (text.length === 0) {
    return { ok: false, reason: "no value" };
  }

  if (!NUMBER.test(text)) {
    return { ok: false, reason: "not a number" };
  }

  const value = Number(text);

  if (!Number.isFinite(value)) {
    return { ok: false, reason: "not a number" };
  }

  if (isIntegralType(reportedType) && !Number.isInteger(value)) {
    return { ok: false, reason: "must be a whole number" };
  }

  return { ok: true, value };
}
