import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { liveTelemetry } from "../../lib/link/liveTelemetry";
import type { MavCommand, Plan, PlanItem, Telemetry } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import { useCommandStore } from "../../lib/plan/commandStore";
import { carriesPosition, commandOf } from "../../lib/plan/planItems";
import {
  VERTEX_EXCLUSION,
  circleRing,
  circlesOf,
  ringOf,
} from "../../lib/plan/fence";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { usePlanStore } from "../../lib/plan/planStore";
import { useUiStore } from "../../lib/ui/uiStore";
import {
  DEFAULT_CENTRE,
  DEFAULT_ZOOM,
  TRACK_LIMIT,
} from "./mapStyle";
import { PROVIDERS, providerOf, styleOf } from "./providers";
import styles from "./FlightMap.module.css";

/**
 * The map.
 *
 * The vehicle, the track it has flown, the plan it is flying and the place it started
 * from are drawn on it. The map is driven from outside React: the position, the heading
 * and the track are written into the map directly when a snapshot arrives, so a
 * telemetry tick does not re-render the tree.
 *
 * While the plan is being edited the items of the plan are drawn as **markers** and not
 * as a layer of circles: a circle cannot be dragged, and dragging a waypoint on the map
 * is how a plan is drawn.
 */

/** The identifier of the source of the track. */
const TRACK_SOURCE = "track";

/** The identifier of the source of the plan. */
const PLAN_SOURCE = "plan";

/** The identifier of the source of the items of the plan. */
const ITEMS_SOURCE = "plan-items";

/** The identifier of the source of the polygon of a fence. */
const FENCE_SOURCE = "fence";

/** The identifier of the source of the circles of a fence. */
const CIRCLES_SOURCE = "fence-circles";

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

/** A marker of an item of the plan. */
interface ItemMarker {
  /** The marker on the map. */
  readonly marker: maplibregl.Marker;
  /** The item it stands for. The plan is renumbered when it is changed. */
  seq: number;
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

/**
 * The items of a plan that carry a place.
 *
 * A command that does not fly to a place — a delay, a change of speed — has zeroes in
 * its coordinates, and a line drawn through them would run to the middle of the ocean.
 * The catalogue says which commands carry a position, and the map asks it.
 */
function positioned(
  items: readonly PlanItem[],
  commands: readonly MavCommand[],
): PlanItem[] {
  return items.filter((item) => {
    if (!Number.isFinite(item.x) || !Number.isFinite(item.y)) {
      return false;
    }
    if (commands.length === 0) {
      // The catalogue has not been read yet: everything that has a number is drawn.
      return true;
    }
    return carriesPosition(commandOf(commands, item.commandValue));
  });
}

/** The coordinates of the items of a plan that carry a position. */
function coordinatesOf(items: readonly PlanItem[]): [number, number][] {
  return items
    .filter((item) => Number.isFinite(item.x) && Number.isFinite(item.y))
    .map((item) => [item.y, item.x] as [number, number]);
}

/**
 * The most markers the map draws at once.
 *
 * A marker is an element in the document, and the map moves every one of them on every
 * frame it moves: a plan of a thousand waypoints would cost the map its frame rate.
 * The items that are in the view are the ones that can be worked on, and the operator
 * is told when there are more of them than the map will draw.
 */
const MAX_MARKERS = 150;

/**
 * Whether a place is in the view of the map, with a margin around it.
 *
 * A plan can be thousands of items long, and a marker for each of them is thousands of
 * elements in the document: the map would crawl. Only the items that are in the view are
 * given a marker, and the rest are drawn when the map comes to them.
 */
function inView(
  bounds: maplibregl.LngLatBounds,
  item: PlanItem,
): boolean {
  const west = bounds.getWest();
  const east = bounds.getEast();
  const south = bounds.getSouth();
  const north = bounds.getNorth();

  const marginX = (east - west) * 0.25;
  const marginY = (north - south) * 0.25;

  return (
    item.y >= west - marginX &&
    item.y <= east + marginX &&
    item.x >= south - marginY &&
    item.x <= north + marginY
  );
}

export function FlightMap() {
  const container = useRef<HTMLDivElement | null>(null);
  const map = useRef<maplibregl.Map | null>(null);
  const marker = useRef<maplibregl.Marker | null>(null);
  const loaded = useRef(false);
  /** Puts the sources and the layers of the map in, once it is ready. */
  const install = useRef<(() => void) | null>(null);
  const track = useRef<[number, number][]>([]);
  const last = useRef<[number, number] | null>(null);
  const lastHome = useRef<Telemetry["home"] | undefined>(undefined);
  const items = useRef(new Map<number, ItemMarker>());
  const fitted = useRef<{
    readonly editor: boolean;
    readonly count: number;
    readonly plan: unknown;
  } | null>(null);

  const [failure, setFailure] = useState<string | null>(null);
  const [ready, setReady] = useState(false);

  const follow = useUiStore((state) => state.followVehicle);
  const showControls = useUiStore((state) => state.showMapControls);
  const mapProvider = useUiStore((state) => state.mapProvider);
  const setMapProvider = useUiStore((state) => state.setMapProvider);
  const setFollow = useUiStore((state) => state.setFollowVehicle);
  const toggleControls = useUiStore((state) => state.toggleMapControls);

  const kind = usePlanStore((state) => state.kind);
  const plan = usePlanStore((state) => state.plans[kind]);
  const vehicles = useLinkStore((state) => state.vehicles);

  const editorOpen = usePlanEditorStore((state) => state.open);
  const draft = usePlanEditorStore((state) => state.draft);
  const selected = usePlanEditorStore((state) => state.selected);
  const adding = usePlanEditorStore((state) => state.adding);

  const commands = useCommandStore((state) => state.commands);
  const loadCommands = useCommandStore((state) => state.load);

  // The catalogue of the commands is read once: the map needs it to know which items
  // of a plan have a place at all.
  useEffect(() => {
    void loadCommands();
  }, [loadCommands]);

  // While the plan is being edited, the map shows the plan that is being edited: the
  // plan the vehicle holds is not the plan on the screen until the edits are uploaded.
  const shown = useMemo<Plan | null>(
    () => (editorOpen ? { kind, items: draft } : plan),
    [draft, editorOpen, kind, plan],
  );

  // The map itself, once.
  useEffect(() => {
    const element = container.current;
    if (!element) {
      return;
    }

    const instance = new maplibregl.Map({
      container: element,
      style: styleOf(providerOf(mapProvider)),
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
    // The marker is a graphic, not a control: the map library would otherwise make it
    // a button, and a button that cannot be pressed is a lie to a screen reader.
    vehicle.setAttribute("role", "presentation");
    marker.current = new maplibregl.Marker({
      element: vehicle,
      rotationAlignment: "map",
      pitchAlignment: "map",
    });
    // The marker needs a place before it is put on the map: until it has one, the map
    // has nothing to draw it at.
    marker.current.setLngLat([DEFAULT_CENTRE[0], DEFAULT_CENTRE[1]]);
    marker.current.addTo(instance);

    // The sources and the layers of the map belong to its style: when the style
    // changes, they go with it and have to be put back.
    const install = () => {
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
      instance.addSource(FENCE_SOURCE, {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: { type: "Polygon", coordinates: [] },
        },
      });
      instance.addSource(CIRCLES_SOURCE, {
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

      // The fence: the polygon the vehicle will not leave, and the circles that
      // are in it or out of it.
      instance.addLayer({
        id: "fence-fill",
        type: "fill",
        source: FENCE_SOURCE,
        paint: {
          "fill-color": "#e2a03f",
          "fill-opacity": 0.18,
        },
      });

      instance.addLayer({
        id: "fence-line",
        type: "line",
        source: FENCE_SOURCE,
        layout: { "line-join": "round" },
        paint: {
          "line-color": "#e2a03f",
          "line-width": 2,
          "line-opacity": 0.9,
        },
      });

      instance.addLayer({
        id: "fence-circles",
        type: "line",
        source: CIRCLES_SOURCE,
        layout: { "line-join": "round" },
        paint: {
          "line-color": "#e2a03f",
          "line-width": 1.5,
          "line-opacity": 0.8,
          "line-dasharray": [3, 2],
        },
      });

      // The points that are excluded from the polygon of a fence: they are holes
      // in it, and the vehicle keeps out of them.
      instance.addLayer({
        id: "plan-excluded",
        type: "circle",
        source: ITEMS_SOURCE,
        filter: ["==", ["get", "excluded"], true],
        paint: {
          "circle-radius": 4,
          "circle-color": "#f85149",
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
    };

    install.current = () => install();

    instance.on("load", () => {
      install();
    });

    instance.on("error", (event) => {
      const message =
        event.error instanceof Error ? event.error.message : String(event.error);
      setFailure(message);
    });

    // A click on the map adds an item, when the editor is armed for it.
    instance.on("click", (event) => {
      const editor = usePlanEditorStore.getState();
      if (!editor.open || !editor.adding) {
        return;
      }
      editor.addAt(event.lngLat.lat, event.lngLat.lng);
    });

    const observer = new ResizeObserver(() => {
      instance.resize();
    });
    observer.observe(element);

    return () => {
      observer.disconnect();
      marker.current?.remove();
      marker.current = null;
      for (const entry of items.current.values()) {
        entry.marker.remove();
      }
      items.current.clear();
      instance.remove();
      map.current = null;
      loaded.current = false;
    };
  }, []);

  // The provider of the tiles. The style of the map is the provider's, and the sources
  // and the layers go with it: they are put back when the new style has loaded.
  useEffect(() => {
    const instance = map.current;
    if (!instance || !loaded.current) {
      return;
    }

    instance.setStyle(styleOf(providerOf(mapProvider)));

    instance.once("styledata", () => {
      install.current?.();
    });
  }, [mapProvider]);

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

  // The line of the plan and the items on it. It is drawn again when the map is ready,
  // because the plan often arrives before the style of the map has finished loading.
  useEffect(() => {
    const instance = map.current;
    if (!instance || !loaded.current || !shown) {
      return;
    }

    const places = positioned(shown.items, commands);
    const coordinates = coordinatesOf(places);

    const line = instance.getSource(PLAN_SOURCE);
    if (line) {
      setSourceData(instance, PLAN_SOURCE, {
        type: "Feature",
        properties: {},
        geometry: { type: "LineString", coordinates },
      });
    }

    const source = instance.getSource(ITEMS_SOURCE);
    if (source) {
      setSourceData(instance, ITEMS_SOURCE, {
        type: "FeatureCollection",
        features: places.map((item) => ({
          type: "Feature",
          geometry: { type: "Point", coordinates: [item.y, item.x] },
          properties: {
            seq: item.seq,
            current: item.current,
            excluded: item.command === VERTEX_EXCLUSION,
          },
        })),
      });
    }

    // The polygon of the fence, and the circles that are in it or out of it.
    const ring = shown.kind === "fence" ? ringOf(shown.items) : null;

    const fence = instance.getSource(FENCE_SOURCE);
    if (fence) {
      setSourceData(instance, FENCE_SOURCE, {
        type: "Feature",
        properties: {},
        geometry: {
          type: "Polygon",
          coordinates: ring === null ? [] : [ring],
        },
      });
    }

    const circles = instance.getSource(CIRCLES_SOURCE);
    if (circles) {
      setSourceData(instance, CIRCLES_SOURCE, {
        type: "FeatureCollection",
        features:
          shown.kind === "fence"
            ? circlesOf(shown.items).map((circle) => ({
                type: "Feature",
                properties: { included: circle.included },
                geometry: {
                  type: "Polygon",
                  coordinates: [circleRing(circle)],
                },
              }))
            : [],
      });
    }
  }, [commands, shown, ready]);

  // The markers of the items, while the plan is being edited. They are draggable: a
  // waypoint is drawn by dragging it, not by typing its coordinates.
  //
  // The markers are drawn outside React: the map moves far more often than the plan
  // does, and a re-render of the tree for every frame of a pan would cost more than the
  // map itself.
  const drawItems = useCallback(() => {
    const instance = map.current;
    if (!instance || !loaded.current) {
      return;
    }

    const editor = usePlanEditorStore.getState();

    if (!editor.open) {
      for (const entry of items.current.values()) {
        entry.marker.remove();
      }
      items.current.clear();
      return;
    }

    const commands = useCommandStore.getState().commands;
    const bounds = instance.getBounds();
    const places = positioned(editor.draft, commands);
    const seen = new Set<number>();
    let drawn = 0;

    for (const item of places) {
      if (drawn >= MAX_MARKERS) {
        break;
      }
      if (!inView(bounds, item)) {
        continue;
      }

      seen.add(item.seq);
      drawn += 1;

      let entry = items.current.get(item.seq);
      if (entry === undefined) {
        const element = document.createElement("div");
        // A point that is excluded from the polygon of a fence is drawn in the
        // colour of the danger: it is the one the vehicle must keep out of.
        element.className =
          (item.command === VERTEX_EXCLUSION
            ? styles.excludedItem ?? ""
            : styles.item ?? "");
        // The marker is a graphic, not a control: the map library would otherwise make
        // it a button, and a plan of a thousand waypoints would be a thousand buttons
        // for the keyboard to walk through. The grid is the list of the items.
        element.setAttribute("role", "presentation");
        element.setAttribute("aria-hidden", "true");

        const created = new maplibregl.Marker({
          element,
          draggable: true,
          anchor: "center",
        });

        const created_entry: ItemMarker = { marker: created, seq: item.seq };

        created.on("dragend", () => {
          const position = created.getLngLat();
          usePlanEditorStore.getState().change(created_entry.seq, {
            x: position.lat,
            y: position.lng,
          });
        });

        element.addEventListener("click", (event) => {
          event.stopPropagation();
          usePlanEditorStore.getState().select(created_entry.seq);
        });

        // The marker needs a place before it is put on the map: until it has one, the
        // map has nothing to draw it at.
        created.setLngLat([item.y, item.x]);
        created.addTo(instance);
        items.current.set(item.seq, created_entry);
        entry = created_entry;
      }

      entry.seq = item.seq;
      entry.marker.setLngLat([item.y, item.x]);

      const element = entry.marker.getElement();
      element.textContent = String(item.seq + 1);
      element.title = `Item ${item.seq + 1}`;
      if (styles.itemSelected) {
        element.classList.toggle(styles.itemSelected, item.seq === editor.selected);
      }
      if (styles.itemCurrent) {
        element.classList.toggle(styles.itemCurrent, item.current);
      }
    }

    for (const [seq, entry] of items.current) {
      if (!seen.has(seq)) {
        entry.marker.remove();
        items.current.delete(seq);
      }
    }

    // The operator is told when the map is not showing every item of the plan.
    usePlanEditorStore.getState().setMarked(drawn, places.length);
  }, []);

  // The markers are drawn when the plan changes.
  useEffect(() => {
    drawItems();
  }, [commands, drawItems, draft, editorOpen, ready, selected]);

  // And when the map comes to another part of the plan.
  useEffect(() => {
    const instance = map.current;
    if (!instance) {
      return;
    }

    let pending = false;
    const onMove = () => {
      if (pending) {
        return;
      }
      pending = true;
      requestAnimationFrame(() => {
        pending = false;
        drawItems();
      });
    };

    instance.on("move", onMove);
    return () => {
      instance.off("move", onMove);
    };
  }, [drawItems, ready]);

  // The circles of the plan give way to the markers while the plan is being edited.
  useEffect(() => {
    const instance = map.current;
    if (!instance || !loaded.current) {
      return;
    }

    const visibility = editorOpen ? "none" : "visible";
    instance.setLayoutProperty("plan-items", "visibility", visibility);
    instance.setLayoutProperty("plan-current", "visibility", visibility);
  }, [editorOpen, ready]);

  // The view of the plan. It is fitted when the plan changes as a plan: when the
  // editor is opened or closed, and when the plan on the vehicle is read again. A
  // waypoint that is dragged does not move the map under the pointer that is dragging
  // it, so the view is left alone.
  useEffect(() => {
    const instance = map.current;
    if (!instance || !loaded.current || !shown) {
      return;
    }

    const coordinates = coordinatesOf(positioned(shown.items, commands));
    if (coordinates.length === 0) {
      return;
    }

    const last = fitted.current;
    if (
      last !== null &&
      last.editor === editorOpen &&
      last.count === shown.items.length &&
      (editorOpen || last.plan === plan)
    ) {
      return;
    }

    fitted.current = {
      editor: editorOpen,
      count: shown.items.length,
      plan,
    };

    const bounds = coordinates.reduce(
      (carry, coordinate) => carry.extend(coordinate),
      new maplibregl.LngLatBounds(coordinates[0], coordinates[0]),
    );
    instance.fitBounds(bounds, { padding: 60, maxZoom: 17, duration: 600 });
  }, [commands, editorOpen, plan, ready, shown]);

  return (
    <div className={styles.map}>
      <div className={styles.canvas} ref={container} />

      <div className={styles.controls}>
        {showControls ? (
          <select
            className={styles.provider}
            value={mapProvider}
            aria-label="The provider of the tiles"
            title="The provider of the tiles"
            onChange={(event) => {
              setMapProvider(event.target.value);
            }}
          >
            {PROVIDERS.map((provider) => (
              <option key={provider.id} value={provider.id}>
                {provider.name}
              </option>
            ))}
          </select>
        ) : null}

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

      <div className={styles.messages}>
        {adding ? (
          <p className={styles.hint} role="status">
            Click the map to add a waypoint
          </p>
        ) : null}

        {failure ? (
          <p className={styles.failure} role="alert">
            The map could not be drawn: {failure}
          </p>
        ) : null}
      </div>

      <p className={styles.caption}>
        {vehicles.length > 0
          ? `${vehicles.length} vehicle${vehicles.length === 1 ? "" : "s"} on the link`
          : "No vehicle on the link"}
      </p>
    </div>
  );
}
