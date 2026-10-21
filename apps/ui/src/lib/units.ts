/**
 * The units the interface measures in.
 *
 * The values always travel in the units of the protocol — metres, metres per second,
 * volts — and the operator reads them in the units they think in. The conversion
 * happens where the number becomes a string, and nowhere else.
 *
 * The factors are upstream's: a metre is 3.2808399 feet, and a metre per second is
 * 2.23693629 miles per hour.
 */

/** The system of units the operator reads in. */
export type UnitSystem = "metric" | "imperial";

/** How many feet a metre is. */
export const FEET_PER_METRE = 3.2808399;

/** How many miles per hour a metre per second is. */
export const MPH_PER_METRE_PER_SECOND = 2.23693629;

/** How many kilometres per hour a metre per second is. */
export const KPH_PER_METRE_PER_SECOND = 3.6;

/** The system of units that are used when nothing has been chosen. */
export const DEFAULT_UNITS: UnitSystem = "metric";

/** The name of the unit of a distance. */
export function distanceUnit(units: UnitSystem): string {
  return units === "imperial" ? "ft" : "m";
}

/** The name of the unit of an altitude. */
export function altitudeUnit(units: UnitSystem): string {
  return units === "imperial" ? "ft" : "m";
}

/** The name of the unit of a speed. */
export function speedUnit(units: UnitSystem): string {
  return units === "imperial" ? "mph" : "m/s";
}

/** A distance, in the unit of the system. */
export function convertDistance(value: number, units: UnitSystem): number {
  return units === "imperial" ? value * FEET_PER_METRE : value;
}

/** An altitude, in the unit of the system. */
export function convertAltitude(value: number, units: UnitSystem): number {
  return units === "imperial" ? value * FEET_PER_METRE : value;
}

/** A speed, in the unit of the system. */
export function convertSpeed(value: number, units: UnitSystem): number {
  return units === "imperial" ? value * MPH_PER_METRE_PER_SECOND : value;
}
