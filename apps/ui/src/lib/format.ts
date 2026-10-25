/**
 * The formatting of the values the interface shows.
 *
 * These functions are the only place where a number becomes a string. They never change
 * a value: they render what the backend sent, and they render a dash when there is
 * nothing to render.
 *
 * The measurements are rendered in the unit the operator reads in, and the conversion
 * happens here and nowhere else: the values that travel over the wire are always in the
 * units of the protocol.
 */

import { useLocaleStore } from "./i18n/localeStore";
import { useUiStore } from "./ui/uiStore";
import {
  altitudeUnit,
  convertAltitude,
  convertDistance,
  convertSpeed,
  distanceUnit,
  speedUnit,
} from "./units";

/** What is shown when a value is not known. */
export const DASH = "—";

/** Whether a value is something that can be shown. */
export function present(value: number | null | undefined): value is number {
  return typeof value === "number" && Number.isFinite(value);
}

/** A number with a fixed number of decimals. */
export function formatNumber(
  value: number | null | undefined,
  digits = 1,
): string {
  return present(value) ? value.toFixed(digits) : DASH;
}

/**
 * A whole number, without a decimal point.
 *
 * The thousands separator is the one the language the window is read in uses: a count is read as
 * a word, and "1,234" and "1.234" are two different numbers to somebody who reads one of them.
 */
export function formatCount(value: number | null | undefined): string {
  return present(value)
    ? new Intl.NumberFormat(useLocaleStore.getState().tag, {
        maximumFractionDigits: 0,
      }).format(value)
    : DASH;
}

/**
 * An instant, in the words an operator reads.
 *
 * The seconds since the epoch are shown as they are, and not as the clock of the room shows
 * them: the instant a camera wrote came without a timezone, and showing it through the offset of
 * the machine would move it by that offset, which is exactly what the ground station is trying
 * to find out.
 */
export function formatInstant(seconds: number | null | undefined): string {
  if (!present(seconds)) {
    return DASH;
  }

  return new Date(seconds * 1000).toISOString().slice(0, 19).replace("T", " ");
}

/** The system of units the operator has chosen. */
function units() {
  return useUiStore.getState().units;
}

/** A distance, in metres or kilometres as the number asks. */
export function formatDistance(value: number | null | undefined): string {
  if (!present(value)) {
    return DASH;
  }

  const system = units();

  if (system === "imperial") {
    const feet = convertDistance(value, system);
    if (Math.abs(feet) < 1000) {
      return `${feet.toFixed(1)} ${distanceUnit(system)}`;
    }
    return `${(feet / 5280).toFixed(2)} mi`;
  }

  if (Math.abs(value) < 1000) {
    return `${value.toFixed(1)} ${distanceUnit(system)}`;
  }
  return `${(value / 1000).toFixed(2)} km`;
}

/** A speed, in metres per second or in miles per hour. */
export function formatSpeed(value: number | null | undefined): string {
  if (!present(value)) {
    return DASH;
  }

  const system = units();
  return `${convertSpeed(value, system).toFixed(1)} ${speedUnit(system)}`;
}

/** An altitude, in metres or in feet. */
export function formatAltitude(value: number | null | undefined): string {
  if (!present(value)) {
    return DASH;
  }

  const system = units();
  return `${convertAltitude(value, system).toFixed(1)} ${altitudeUnit(system)}`;
}

/** A vertical rate, in metres per second or in miles per hour. */
export function formatVerticalSpeed(value: number | null | undefined): string {
  if (!present(value)) {
    return DASH;
  }

  const system = units();
  return `${convertSpeed(value, system).toFixed(1)} ${speedUnit(system)}`;
}

/** A heading, in degrees, as a compass bearing would be written. */
export function formatHeading(value: number | null | undefined): string {
  if (!present(value)) {
    return DASH;
  }
  const normalised = ((value % 360) + 360) % 360;
  return `${normalised.toFixed(0)}°`;
}

/** A voltage, in volts. */
export function formatVoltage(value: number | null | undefined): string {
  return present(value) ? `${value.toFixed(2)} V` : DASH;
}

/** A current, in amperes. */
export function formatCurrent(value: number | null | undefined): string {
  return present(value) ? `${value.toFixed(2)} A` : DASH;
}

/** A percentage. */
export function formatPercent(value: number | null | undefined): string {
  return present(value) ? `${value.toFixed(0)} %` : DASH;
}

/** A pair of coordinates, in degrees. */
export function formatCoordinate(
  latitude: number | null | undefined,
  longitude: number | null | undefined,
): string {
  if (!present(latitude) || !present(longitude)) {
    return DASH;
  }
  return `${latitude.toFixed(6)}, ${longitude.toFixed(6)}`;
}

/** A signed angle, in degrees. */
export function formatAngle(value: number | null | undefined): string {
  return present(value) ? `${value.toFixed(1)}°` : DASH;
}

/** A text value, or a dash when there is none. */
export function formatText(value: string | null | undefined): string {
  if (value === null || value === undefined || value.length === 0) {
    return DASH;
  }
  return value;
}

/** A yes or a no. */
export function formatYesNo(value: boolean | null | undefined): string {
  if (value === null || value === undefined) {
    return DASH;
  }
  return value ? "yes" : "no";
}
