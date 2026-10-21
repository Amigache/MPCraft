import { useMemo } from "react";

import { formatDistance } from "../../lib/format";
import { distanceBetween } from "../../lib/geo";
import type { PlanItem } from "../../lib/ipc/types";
import { usePlanStore } from "../../lib/plan/planStore";
import styles from "./RouteBar.module.css";

/**
 * The bar of the route.
 *
 * The plan is a road: the places of it are the milestones, and the bar says how far
 * along the road the vehicle is and how far the road goes. The distance to the next
 * milestone is said in figures, because that is the number a pilot wants.
 */

/** A place of the plan with a place on the ground. */
interface Place {
  readonly item: PlanItem;
  readonly lat: number;
  readonly lng: number;
}

export interface RouteBarProps {
  /** Where the vehicle is now. */
  readonly latitude: number | null;
  /** Where the vehicle is now. */
  readonly longitude: number | null;
}

export function RouteBar({ latitude, longitude }: RouteBarProps) {
  const plans = usePlanStore((state) => state.plans);
  const kind = usePlanStore((state) => state.kind);

  const plan = plans[kind];

  const places = useMemo<Place[]>(() => {
    if (plan === null || plan === undefined) {
      return [];
    }

    return plan.items
      .filter((item) => Number.isFinite(item.x) && Number.isFinite(item.y))
      .map((item) => ({ item, lat: item.x, lng: item.y }));
  }, [plan]);

  if (places.length < 2 || latitude === null || longitude === null) {
    return null;
  }

  // The length of the whole route, and the length of the part that is behind the
  // vehicle.
  const legs = places.slice(1).map((place, index) => {
    const from = places[index];
    return from === undefined
      ? 0
      : distanceBetween(from.lat, from.lng, place.lat, place.lng);
  });

  const total = legs.reduce((sum, leg) => sum + leg, 0);

  const current = places.findIndex((place) => place.item.current);
  const travelled =
    current <= 0
      ? 0
      : legs.slice(0, current).reduce((sum, leg) => sum + leg, 0);

  // The distance to the next milestone, or to the last one when the vehicle is on the
  // last leg of the road.
  const next = places[Math.min(Math.max(current, 0), places.length - 1)];
  const remaining =
    next === undefined
      ? 0
      : distanceBetween(latitude, longitude, next.lat, next.lng);

  const fraction = total === 0 ? 0 : Math.min(1, travelled / total);

  return (
    <div className={styles.bar} role="group" aria-label="The route">
      <div className={styles.track}>
        <div
          className={styles.travelled}
          style={{ width: `${(fraction * 100).toFixed(1)}%` }}
        />
      </div>

      <div className={styles.figures}>
        <span className={styles.figure}>
          {formatDistance(travelled)} flown
        </span>
        <span className={styles.figure}>
          {formatDistance(remaining)} to go
        </span>
        <span className={styles.figure}>{formatDistance(total)} in all</span>
      </div>
    </div>
  );
}
