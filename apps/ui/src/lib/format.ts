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

/** A whole number, without a decimal point. */
export function formatCount(value: number | null | undefined): string {
  return present(value) ? Math.round(value).toLocaleString("en-US") : DASH;
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
