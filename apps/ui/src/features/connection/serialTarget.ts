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

/** The value of the rate that leaves the choice to the program. */
export const AUTO_BAUD = "auto";

/**
 * Whether the target leaves the rate to the program.
 *
 * A vehicle that is talking at a rate nobody wrote down is found by trying them one
 * after another: the port is opened at each rate in turn, and the rate at which the
 * vehicle answers is the one it talks at.
 */
export function isAutoBaud(target: string): boolean {
  const trimmed = target.trim().toLowerCase();

  if (!trimmed.startsWith("serial:")) {
    return false;
  }

  return trimmed.split("|").some((option) => {
    const [key, value] = option.split("=");

    return (
      key?.trim() === "baud" && (value ?? "").trim().toLowerCase() === AUTO_BAUD
    );
  });
}

/**
 * Leaves the rate of a serial link to the program.
 *
 * A target that is not a serial one is left as it is: the rate of a serial link means
 * nothing to a UDP address.
 */
export function withAutoBaud(target: string): string {
  const parsed = parseSerialTarget(target);

  if (!parsed) {
    return target;
  }

  const withoutOptions = target.split("|").filter((option) => {
    const key = option.split("=")[0];

    return key?.trim().toLowerCase() !== "baud";
  });

  return `${withoutOptions.join("|")}|baud=${AUTO_BAUD}`;
}

/** The rate at which the ports of this machine are opened. */
/**
 * The rates a serial port can be opened at, in the order a station offers them.
 *
 * The list is the one Mission Planner offers (`ConnectionControl`): every rate a board or an
 * adapter in common use speaks, from the slowest to the fastest, so a port that was left at
 * 9600 and a link that runs at 921600 are both one choice away.
 */
export const BAUD_RATES: readonly number[] = [
  1200, 2400, 4800, 9600, 19200, 38400, 57600, 111100, 115200, 230400, 250000,
  460800, 500000, 921600, 1000000, 1500000, 2000000, 3000000,
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

  const rate =
    parts[1] === undefined ? Number.NaN : Number.parseInt(parts[1], 10);
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
