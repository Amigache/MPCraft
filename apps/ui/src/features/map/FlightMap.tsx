import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useEffect, useRef, useState } from "react";

import { liveTelemetry } from "../../lib/link/liveTelemetry";
import type { Telemetry } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { useUiStore } from "../../lib/ui/uiStore";
import {
  DEFAULT_CENTRE,
  DEFAULT_ZOOM,
  MAP_STYLE_URL,
  TRACK_LIMIT,
} from "./mapStyle";
import styles from "./FlightMap.module.css";

/**
 * The map.
 *
 * The vehicle, the track it has flown, the plan it is flying and the place it started
 * from are drawn on it. The map is driven from outside React: the position, the heading
 * and the track are written into the map directly when a snapshot arrives, so a
 * telemetry tick does not re-render the tree.
 */

/** The identifier of the source of the track. */
const TRACK_SOURCE = "track";

/** The identifier of the source of the plan. */
const PLAN_SOURCE = "plan";

/** The identifier of the source of the items of the plan. */
const ITEMS_SOURCE = "plan-items";

/** The identifier of the source of the home position. */
const HOME_SOURCE = "home";

/** The smallest movement, in degrees, that is worth a point on the track. */
const TRACK_STEP = 1e-5;

/**
 * Whether a position is one the map can be told about.
 *
 * The map library refuses a latitude outside the poles, and it refuses to be told
 * nothing at all. A position that cannot be drawn is skipped, not thrown.
 */
function usable(position: {
  readonly lat: number;
  readonly lng: number;
} | null): position is { readonly lat: number; readonly lng: number } {
  if (!position) {
    return false;
  }
  return (
    Number.isFinite(position.lat) &&
    Number.isFinite(position.lng) &&
    position.lat >= -90 &&
    position.lat <= 90 &&
    position.lng >= -180 &&
    position.lng <= 180
  );
}

/**
 * The map does its work in a web worker.
 *
 * The worker is a script of its own, served from the same place as the interface, rather
 * than a blob the page builds for itself: the content security policy of the desktop
 * shell allows the scripts of the application and nothing else.
 */
maplibregl.setWorkerUrl("/maplibre-gl-csp-worker.js");

/** The geometry a source of the map is given. */
type SourceData = NonNullable<maplibregl.GeoJSONSourceSpecification["data"]>;

/** Writes a new geometry into a source of the map. */
function setSourceData(
  instance: maplibregl.Map,
  id: string,
  data: SourceData,
): void {
  const source = instance.getSource(id);
  if (source instanceof maplibregl.GeoJSONSource) {
    source.setData(data);
  }
}

export function FlightMap() {
  const container = useRef<HTMLDivElement | null>(null);
  const map = useRef<maplibregl.Map | null>(null);
  const marker = useRef<maplibregl.Marker | null>(null);
  const loaded = useRef(false);
  const track = useRef<[number, number][]>([]);
  const last = useRef<[number, number] | null>(null);
  const lastHome = useRef<Telemetry["home"] | undefined>(undefined);

  const [failure, setFailure] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  const follow = useUiStore((state) => state.followVehicle);
  const showControls = useUiStore((state) => state.showMapControls);
  const setFollow = useUiStore((state) => state.setFollowVehicle);
  const toggleControls = useUiStore((state) => state.toggleMapControls);

  const plan = usePlanStore((state) => state.plans.mission);
  const vehicles = useLinkStore((state) => state.vehicles);

  // The map itself, once.
  useEffect(() => {
    const element = container.current;
    if (!element) {
      return;
    }

    const instance = new maplibregl.Map({
      container: element,
      style: MAP_STYLE_URL,
      center: [DEFAULT_CENTRE[0], DEFAULT_CENTRE[1]],
      zoom: DEFAULT_ZOOM,
      attributionControl: false,
    });

    map.current = instance;

    instance.addControl(
      new maplibregl.NavigationControl({ visualizePitch: true }),
      "top-right",
    );
    instance.addControl(
      new maplibregl.AttributionControl({ compact: true }),
      "bottom-right",
    );

    const vehicle = document.createElement("div");
    vehicle.className = styles.vehicle ?? "";
    marker.current = new maplibregl.Marker({
      element: vehicle,
      rotationAlignment: "map",
      pitchAlignment: "map",
    });
    // The marker needs a place before it is put on the map: until it has one, the map
    // has nothing to draw it at.
    marker.current.setLngLat([DEFAULT_CENTRE[0], DEFAULT_CENTRE[1]]);
    marker.current.addTo(instance);

    instance.on("load", () => {
      loaded.current = true;
      instance.addSource(TRACK_SOURCE, {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: { type: "LineString", coordinates: [] },
        },
      });
      instance.addSource(PLAN_SOURCE, {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: { type: "LineString", coordinates: [] },
        },
      });
      instance.addSource(ITEMS_SOURCE, {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });
      instance.addSource(HOME_SOURCE, {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: { type: "Point", coordinates: [0, 0] },
        },
      });

      instance.addLayer({
        id: "track-line",
        type: "line",
        source: TRACK_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: { "line-color": "#00c896", "line-width": 2, "line-opacity": 0.8 },
      });

      instance.addLayer({
        id: "plan-line",
        type: "line",
        source: PLAN_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#e6edf3",
          "line-width": 1.5,
          "line-opacity": 0.65,
          "line-dasharray": [2, 2],
        },
      });

      instance.addLayer({
        id: "plan-items",
        type: "circle",
        source: ITEMS_SOURCE,
        paint: {
          "circle-radius": 4,
          "circle-color": "#161b22",
          "circle-stroke-color": "#e6edf3",
          "circle-stroke-width": 1.5,
        },
      });

      instance.addLayer({
        id: "plan-current",
        type: "circle",
        source: ITEMS_SOURCE,
        filter: ["==", ["get", "current"], true],
        paint: {
          "circle-radius": 6,
          "circle-color": "#00c896",
          "circle-stroke-color": "#0d1117",
          "circle-stroke-width": 1.5,
        },
      });

      instance.addLayer({
        id: "home-point",
        type: "circle",
        source: HOME_SOURCE,
        paint: {
          "circle-radius": 5,
          "circle-color": "#00c896",
          "circle-stroke-color": "#0d1117",
          "circle-stroke-width": 2,
        },
      });

      setReady(true);
    });

    instance.on("error", (event) => {
      const message =
        event.error instanceof Error ? event.error.message : String(event.error);
      setFailure(message);
    });

    const observer = new ResizeObserver(() => {
      instance.resize();
    });
    observer.observe(element);

    return () => {
      observer.disconnect();
      marker.current?.remove();
      marker.current = null;
      instance.remove();
      map.current = null;
      loaded.current = false;
    };
  }, []);

  // The position, the heading and the track, written into the map directly.
  useEffect(() => {
    const draw = (telemetry: Telemetry) => {
      const instance = map.current;
      const position = telemetry.position;

      if (!instance) {
        return;
      }

      // The place the vehicle started from, when it has reported one.
      const home = telemetry.home;
      if (loaded.current && home !== lastHome.current) {
        lastHome.current = home;
        setSourceData(instance, HOME_SOURCE, {
          type: "Feature",
          properties: {},
          geometry: {
            type: "Point",
            coordinates: home ? [home.lng, home.lat] : [0, 0],
          },
        });
      }

      if (!usable(position)) {
        return;
      }

      const point: [number, number] = [position.lng, position.lat];
      marker.current?.setLngLat(point);

      const heading = telemetry.attitude?.yaw;
      if (typeof heading === "number") {
        marker.current?.setRotation(heading);
      }

      const previous = last.current;
      if (
        previous === null ||
        Math.abs(previous[0] - point[0]) > TRACK_STEP ||
        Math.abs(previous[1] - point[1]) > TRACK_STEP
      ) {
        last.current = point;
        track.current = [...track.current, point].slice(-TRACK_LIMIT);

        const source = instance.getSource(TRACK_SOURCE);
        if (source) {
          setSourceData(instance, TRACK_SOURCE, {
            type: "Feature",
            properties: {},
            geometry: { type: "LineString", coordinates: track.current },
          });
        }
      }

      if (useUiStore.getState().followVehicle) {
        instance.easeTo({ center: point, duration: 400 });
      }
    };

    draw(liveTelemetry.value);

    return liveTelemetry.subscribe(draw);
  }, []);

  // The plan, when it is downloaded. It is drawn again when the map is ready, because
  // the plan often arrives before the style of the map has finished loading.
  useEffect(() => {
    const instance = map.current;
    if (!instance || !loaded.current || !plan) {
      return;
    }

    const coordinates = plan.items
      .filter((item) => Number.isFinite(item.x) && Number.isFinite(item.y))
      .map((item) => [item.y, item.x] as [number, number]);

    const line = instance.getSource(PLAN_SOURCE);
    if (line) {
      setSourceData(instance, PLAN_SOURCE, {
        type: "Feature",
        properties: {},
        geometry: { type: "LineString", coordinates },
      });
    }

    const items = instance.getSource(ITEMS_SOURCE);
    if (items) {
      setSourceData(instance, ITEMS_SOURCE, {
        type: "FeatureCollection",
        features: plan.items
          .filter((item) => Number.isFinite(item.x) && Number.isFinite(item.y))
          .map((item) => ({
            type: "Feature",
            geometry: { type: "Point", coordinates: [item.y, item.x] },
            properties: { seq: item.seq, current: item.current },
          })),
      });
    }

    if (coordinates.length > 0) {
      const bounds = coordinates.reduce(
        (carry, coordinate) => carry.extend(coordinate),
        new maplibregl.LngLatBounds(coordinates[0], coordinates[0]),
      );
      instance.fitBounds(bounds, { padding: 60, maxZoom: 17, duration: 600 });
    }
  }, [plan, ready]);

  return (
    <div className={styles.map}>
      <div className={styles.canvas} ref={container} />

      <div className={styles.controls}>
        {showControls ? (
          <button
            type="button"
            className={`${styles.follow} ${follow ? styles.followOn : ""}`}
            aria-pressed={follow}
            onClick={() => setFollow(!follow)}
          >
            {follow ? "Following" : "Follow"}
          </button>
        ) : null}
        <button
          type="button"
          className={styles.control}
          aria-pressed={showControls}
          onClick={toggleControls}
        >
          {showControls ? "Hide controls" : "Show controls"}
        </button>
      </div>

      {failure ? (
        <p className={styles.failure} role="alert">
          The map could not be drawn: {failure}
        </p>
      ) : null}

      <p className={styles.caption}>
        {vehicles.length > 0
          ? `${vehicles.length} vehicle${vehicles.length === 1 ? "" : "s"} on the link`
          : "No vehicle on the link"}
      </p>
    </div>
  );
}
