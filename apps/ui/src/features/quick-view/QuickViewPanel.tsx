import { useEffect, useState } from "react";

import { LiveValue } from "../../components/LiveValue";
import { normaliseError, requestTelemetryStreams } from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import {
  formatAltitude,
  formatCount,
  formatCurrent,
  formatDistance,
  formatHeading,
  formatPercent,
  formatSpeed,
  formatVerticalSpeed,
  formatVoltage,
  formatYesNo,
} from "../../lib/format";
import type { Telemetry } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import styles from "./QuickViewPanel.module.css";

/**
 * The quick view: a grid of tiles, each one a value at a glance.
 *
 * Which tiles are shown is the user's choice, and the choice is kept between sessions.
 * A value the vehicle has not reported is shown as a dash, never as a zero.
 */

/** One tile of the grid. */
interface Tile {
  readonly id: string;
  readonly label: string;
  /** Picks the value out of a snapshot. */
  readonly select: (
    telemetry: Telemetry,
  ) => number | string | boolean | null | undefined;
  /** Renders the value. */
  readonly format?: (value: number | string | boolean) => string;
  /** Whether what the format gives is a word that the bundles say. */
  readonly word?: boolean;
}

const TILES: readonly Tile[] = [
  {
    id: "altitude",
    label: "Altitude",
    select: (telemetry) => telemetry.position?.alt ?? null,
    format: (value) => formatAltitude(value as number),
  },
  {
    id: "speed",
    label: "Ground speed",
    select: (telemetry) => telemetry.velocity?.groundSpeed ?? null,
    format: (value) => formatSpeed(value as number),
  },
  {
    id: "distance",
    label: "To home",
    select: (telemetry) => telemetry.distanceToHome ?? null,
    format: (value) => formatDistance(value as number),
  },
  {
    id: "heading",
    label: "Heading",
    select: (telemetry) => telemetry.attitude?.yaw ?? null,
    format: (value) => formatHeading(value as number),
  },
  {
    id: "voltage",
    label: "Battery",
    select: (telemetry) => telemetry.battery?.voltage ?? null,
    format: (value) => formatVoltage(value as number),
  },
  {
    id: "remaining",
    label: "Charge",
    select: (telemetry) => telemetry.battery?.remaining ?? null,
    format: (value) => formatPercent(value as number),
  },
  {
    id: "current",
    label: "Current",
    select: (telemetry) => telemetry.battery?.current ?? null,
    format: (value) => formatCurrent(value as number),
  },
  {
    id: "mode",
    label: "Mode",
    select: (telemetry) => telemetry.mode ?? null,
  },
  {
    id: "armed",
    label: "Armed",
    select: (telemetry) => telemetry.armed ?? null,
    format: (value) => formatYesNo(value as boolean),
    word: true,
  },
  {
    id: "satellites",
    label: "Satellites",
    select: (telemetry) => telemetry.gps?.satellites ?? null,
    format: (value) => (value as number).toString(),
  },
  {
    id: "throttle",
    label: "Throttle",
    select: (telemetry) => telemetry.airspeed?.throttle ?? null,
    format: (value) => formatPercent(value as number),
  },
  {
    id: "airspeed",
    label: "Air speed",
    select: (telemetry) => telemetry.airspeed?.airspeed ?? null,
    format: (value) => formatSpeed(value as number),
  },
  {
    id: "climb",
    label: "Climb",
    select: (telemetry) => telemetry.airspeed?.climb ?? null,
    format: (value) => formatVerticalSpeed(value as number),
  },
  {
    id: "signal",
    label: "Signal",
    select: (telemetry) => telemetry.controls?.signal ?? null,
    format: (value) => formatPercent(value as number),
  },
];

/** The tiles that are shown when the user has not chosen. */
const DEFAULT_TILES = [
  "altitude",
  "speed",
  "distance",
  "voltage",
  "remaining",
  "mode",
  "armed",
  "satellites",
];

/** Where the choice of tiles is kept. */
const STORAGE_KEY = "mpcraft.quickTiles";

/** Reads the chosen tiles. */
function readChoice(): string[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (raw === null) {
      return [...DEFAULT_TILES];
    }
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      return [...DEFAULT_TILES];
    }
    const known = parsed.filter(
      (id): id is string =>
        typeof id === "string" && TILES.some((tile) => tile.id === id),
    );
    return known.length > 0 ? known : [...DEFAULT_TILES];
  } catch {
    return [...DEFAULT_TILES];
  }
}

/** Keeps the chosen tiles. */
function writeChoice(ids: readonly string[]): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(ids));
  } catch {
    // A storage that refuses is not a reason to break the interface.
  }
}

export function QuickViewPanel() {
  const [chosen, setChosen] = useState<string[]>(readChoice);
  const [configuring, setConfiguring] = useState(false);
  const [refused, setRefused] = useState<string[]>([]);

  const t = useLocaleStore((state) => state.t);
  const report = useActivityStore((state) => state.report);

  useEffect(() => {
    writeChoice(chosen);
  }, [chosen]);

  // A vehicle sends by the rates configured on its port, and those rates can all be zero, so a
  // station that does not ask sees nothing. The panel asks once when the link is open — a vehicle
  // that is asked keeps streaming, and asking every second would put fourteen commands on the wire
  // for nothing — and shows what the vehicle answered, because an operator has to know which of
  // the tiles will stay empty. What the tiles draw afterwards is what the event carries.
  const status = useLinkStore((state) => state.status);
  const vehicles = useLinkStore((state) => state.vehicles.length);

  // The link is open as soon as the port is; the vehicle is on it only once it has spoken. Asking
  // before that is asking nobody, and the answer — `no_vehicle` — would be shown as a fault of the
  // window rather than as the vehicle that has not arrived yet.
  const open = status === "connected" && vehicles > 0;

  useEffect(() => {
    if (!open) {
      return;
    }

    void requestTelemetryStreams()
      .then((answers) => {
        setRefused(
          answers
            .filter((answer) => answer.answer !== "MAV_RESULT_ACCEPTED")
            .map((answer) => answer.name),
        );
      })
      .catch((cause) => {
        report(normaliseError(cause), null);
      });
  }, [open, report]);

  const toggle = (id: string) => {
    setChosen((current) =>
      current.includes(id)
        ? current.filter((entry) => entry !== id)
        : [...current, id],
    );
  };

  const tiles = TILES.filter((tile) => chosen.includes(tile.id));

  return (
    <div className={styles.panel}>
      <div className={styles.bar}>
        <span className={styles.count}>
          {t(tiles.length === 1 ? "{count} tile" : "{count} tiles", {
            count: tiles.length,
          })}
        </span>
        <button
          type="button"
          className="button"
          aria-expanded={configuring}
          onClick={() => setConfiguring((value) => !value)}
        >
          {configuring ? t("Done") : t("Configure")}
        </button>
      </div>

      {refused.length > 0 ? (
        <p className={styles.note}>
          {t(
            refused.length === 1
              ? "{count} of the messages the tiles read was refused by the vehicle, or did not answer: {names}"
              : "{count} of the messages the tiles read were refused by the vehicle, or did not answer: {names}",
            { count: formatCount(refused.length), names: refused.join(", ") },
          )}
        </p>
      ) : null}

      {configuring ? (
        <ul className={styles.config}>
          {TILES.map((tile) => (
            <li key={tile.id}>
              <label className={styles.configItem}>
                <input
                  type="checkbox"
                  checked={chosen.includes(tile.id)}
                  onChange={() => toggle(tile.id)}
                />
                <span>{t(tile.label)}</span>
              </label>
            </li>
          ))}
        </ul>
      ) : null}

      <div className={styles.grid}>
        {tiles.map((tile) => (
          <div key={tile.id} className={styles.tile}>
            <span className={styles.tileLabel}>{t(tile.label)}</span>
            <LiveValue
              className={styles.tileValue}
              select={tile.select}
              format={(value) => {
                const said = tile.format ? tile.format(value) : String(value);

                return tile.word ? t(said) : said;
              }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
