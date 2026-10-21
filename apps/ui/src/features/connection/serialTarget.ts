/**
 * The serial connection target.
 *
 * A target is a single string the backend takes: `serial:COM3:57600`,
 * `udp:127.0.0.1:14550`, `tcp:192.168.1.10:5760`. The panel builds one from the port
 * and the rate the user has chosen, and the user may write one by hand — so the string
 * has to be read as well as written.
 *
 * These are the rules of the `serial:` form, as the backend reads it:
 *
 *  - the device comes first, and may be a name (`COM3`) or a path (`/dev/ttyUSB0`);
 *  - the rate comes next, and is a whole number of bits per second;
 *  - the options come last, after a `|`, so that a path with a colon in it stays
 *    unambiguous.
 */

/** The parts of a serial target. */
export interface SerialTarget {
  /** The device, for example `COM3` or `/dev/ttyUSB0`. */
  readonly device: string;
  /** The rate, in bits per second. */
  readonly baud: number;
}

/** The rate a serial link is opened at when the target does not say. */
export const DEFAULT_BAUD = 57600;

/** The rate at which the ports of this machine are opened. */
export const BAUD_RATES: readonly number[] = [
  57600, 115200, 230400, 460800, 921600, 14400, 38400, 19200, 9600,
];

/**
 * Reads the serial target, if that is what the string is.
 *
 * A target of another kind — a `udp:` one, a `tcp:` one — is not a serial target, and
 * the panel leaves it alone.
 */
export function parseSerialTarget(target: string): SerialTarget | null {
  const trimmed = target.trim();

  if (!trimmed.toLowerCase().startsWith("serial:")) {
    return null;
  }

  // The options are not part of the device or the rate.
  const withoutOptions = trimmed.slice("serial:".length).split("|")[0] ?? "";
  const parts = withoutOptions.split(":");

  const device = (parts[0] ?? "").trim();
  if (device.length === 0) {
    return null;
  }

  const rate = parts[1] === undefined ? Number.NaN : Number.parseInt(parts[1], 10);
  const baud = Number.isFinite(rate) && rate > 0 ? rate : DEFAULT_BAUD;

  return { device, baud };
}

/**
 * Builds the target of a serial link.
 */
export function buildSerialTarget(device: string, baud: number): string {
  return `serial:${device}:${baud}`;
}

/**
 * Puts another rate on the target.
 *
 * A target that is not a serial one is left as it is: the rate of a serial link means
 * nothing to a UDP address.
 */
export function withBaud(target: string, baud: number): string {
  const parsed = parseSerialTarget(target);

  if (!parsed) {
    return target;
  }

  return buildSerialTarget(parsed.device, baud);
}
