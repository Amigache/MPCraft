import { useEffect, useState } from "react";

import { LiveValue } from "../../components/LiveValue";
import {
  formatAltitude,
  formatCurrent,
  formatDistance,
  formatHeading,
  formatPercent,
  formatSpeed,
  formatVoltage,
  formatYesNo,
} from "../../lib/format";
import type { Telemetry } from "../../lib/ipc/types";
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
  readonly select: (telemetry: Telemetry) => number | string | boolean | null | undefined;
  /** Renders the value. */
  readonly format?: (value: number | string | boolean) => string;
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
  },
  {
    id: "satellites",
    label: "Satellites",
    // The number of satellites is not part of the telemetry the backend publishes yet,
    // so the tile stays empty rather than showing a number that was invented.
    select: () => null,
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

  useEffect(() => {
    writeChoice(chosen);
  }, [chosen]);

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
        <span className={styles.count}>{tiles.length} tiles</span>
        <button
          type="button"
          className="button"
          aria-expanded={configuring}
          onClick={() => setConfiguring((value) => !value)}
        >
          {configuring ? "Done" : "Configure"}
        </button>
      </div>

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
                <span>{tile.label}</span>
              </label>
            </li>
          ))}
        </ul>
      ) : null}

      <div className={styles.grid}>
        {tiles.map((tile) => (
          <div key={tile.id} className={styles.tile}>
            <span className={styles.tileLabel}>{tile.label}</span>
            <LiveValue
              className={styles.tileValue}
              select={tile.select}
              format={tile.format}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
