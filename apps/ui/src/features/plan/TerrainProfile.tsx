import { useCallback, useEffect, useState } from "react";

import { planTerrainProfile } from "../../lib/ipc/bridge";
import type { CommandError, PlanItem, PlanKind } from "../../lib/ipc/types";
import styles from "./PlanEditor.module.css";

/**
 * The profile of the terrain along the plan.
 *
 * The profile is the shape of the ground the plan flies over: the elevation of the
 * terrain at every few metres, and the places of the plan on it. It is what tells
 * whether the plan has room to fly.
 */

/** One point of the profile. */
export interface TerrainPoint {
  /** The distance from the start of the plan, in metres. */
  readonly distance: number;
  /** The elevation of the terrain, in metres above the sea. */
  readonly elevation: number;
  /** Whether the point is a place of the plan. */
  readonly waypoint: boolean;
}

export interface TerrainProfileProps {
  /** The kind of plan the profile is of. */
  readonly kind: PlanKind;
  /** The items of the plan. */
  readonly items: readonly PlanItem[];
  /** The place the panel is put away, when there is one. */
  readonly onClose: (() => void) | undefined;
}

/** The width of the drawing, in the units of the view box. */
const WIDTH = 600;

/** The height of the drawing, in the units of the view box. */
const HEIGHT = 120;

/** The room left above and below the shape of the ground. */
const MARGIN = 12;

export function TerrainProfile({ kind, items, onClose }: TerrainProfileProps) {
  const [points, setPoints] = useState<readonly TerrainPoint[]>([]);
  const [error, setError] = useState<CommandError | null>(null);
  const [busy, setBusy] = useState(false);

  const ask = useCallback(async () => {
    setBusy(true);
    setError(null);

    try {
      const found = await planTerrainProfile(kind, items);
      setPoints(found);
    } catch (cause) {
      setError(cause as CommandError);
      setPoints([]);
    } finally {
      setBusy(false);
    }
  }, [items, kind]);

  // The profile is asked for when the panel opens: the terrain of a place does not
  // change, and asking for it again would download the same tiles again.
  useEffect(() => {
    void ask();
  }, [ask]);

  // The shape of the ground, in the units of the drawing.
  const shape = shapeOf(points);

  return (
    <section className={styles.profile} aria-label="The profile of the terrain">
      <header className={styles.profileHead}>
        <h3 className={styles.profileTitle}>Profile of the terrain</h3>
        {busy ? (
          <span className={styles.profileNote}>Looking for the ground…</span>
        ) : null}
        {onClose !== undefined ? (
          <button
            type="button"
            className="button"
            aria-label="Put the profile away"
            onClick={onClose}
          >
            ×
          </button>
        ) : null}
      </header>

      {error !== null ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message} — the ground
          could not be looked up.
        </p>
      ) : null}

      {!busy && error === null && points.length === 0 ? (
        <p className={styles.profileNote}>
          The plan has no places to look at, or the ground under them is not known.
        </p>
      ) : null}

      {shape !== null ? (
        <svg
          className={styles.profileChart}
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          preserveAspectRatio="none"
          role="img"
          aria-label="The elevation of the terrain along the plan"
        >
          <polyline
            className={styles.profileGround}
            points={shape.ground}
            fill="none"
            strokeWidth="1.5"
            vectorEffect="non-scaling-stroke"
          />
          <polyline
            className={styles.profileFill}
            points={shape.fill}
            fill="none"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
          {shape.places.map((place) => (
            <circle
              key={`${place.distance}-${place.elevation}`}
              className={styles.profilePlace}
              cx={place.x}
              cy={place.y}
              r="3"
            />
          ))}
        </svg>
      ) : null}
    </section>
  );
}

/** The shape of the ground, in the units of the drawing. */
function shapeOf(points: readonly TerrainPoint[]): {
  ground: string;
  fill: string;
  places: { x: number; y: number; distance: number; elevation: number }[];
} | null {
  if (points.length < 2) {
    return null;
  }

  const distances = points.map((point) => point.distance);
  const elevations = points.map((point) => point.elevation);

  const from = Math.min(...distances);
  const to = Math.max(...distances);
  const low = Math.min(...elevations);
  const high = Math.max(...elevations);

  const span = to - from;
  const height = high - low;

  const x = (distance: number) =>
    span === 0 ? 0 : ((distance - from) / span) * WIDTH;

  const y = (elevation: number) =>
    height === 0
      ? HEIGHT / 2
      : HEIGHT - MARGIN - ((elevation - low) / height) * (HEIGHT - 2 * MARGIN);

  const ground = points
    .map((point) => `${x(point.distance).toFixed(1)},${y(point.elevation).toFixed(1)}`)
    .join(" ");

  const fill = `${ground} ${WIDTH},${HEIGHT} 0,${HEIGHT}`;

  const places = points
    .filter((point) => point.waypoint)
    .map((point) => ({
      x: x(point.distance),
      y: y(point.elevation),
      distance: point.distance,
      elevation: point.elevation,
    }));

  return { ground, fill, places };
}
