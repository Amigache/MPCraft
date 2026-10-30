import maplibregl from "maplibre-gl";
import "maplibre-gl/dist/maplibre-gl.css";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useLocaleStore } from "../../lib/i18n/localeStore";
import {
  normaliseError,
  prefetchTiles,
  setHomePosition,
} from "../../lib/ipc/bridge";
import { liveTelemetry } from "../../lib/link/liveTelemetry";
import type {
  GeoPoint,
  MapProvider,
  MavCommand,
  Plan,
  PlanItem,
  Telemetry,
} from "../../lib/ipc/types";
import { chosenVehicle, useLinkStore } from "../../lib/link/linkStore";
import { useCommandStore } from "../../lib/plan/commandStore";
import { carriesPosition, commandOf } from "../../lib/plan/planItems";
import {
  VERTEX_EXCLUSION,
  circleRing,
  circlesOf,
  ringOf,
} from "../../lib/plan/fence";
import { flyTo } from "../../lib/flight/flightActions";
import {
  onPlanScreen,
  usePlanEditorStore,
} from "../../lib/plan/planEditorStore";
import {
  courseLine,
  insertionPoints,
  loiterCircles,
} from "../../lib/plan/overlays";
import { usePlanStore } from "../../lib/plan/planStore";
import { splinePath } from "../../lib/plan/spline";
import { useUiStore } from "../../lib/ui/uiStore";
import { usePictureStore } from "../../lib/georef/pictureStore";
import { usePoiStore } from "../../lib/poi/poiStore";
import { lineOf, useLogTrackStore } from "../../lib/logs/logTrackStore";
import { DEFAULT_CENTRE, DEFAULT_ZOOM, TRACK_LIMIT } from "./mapStyle";
import {
  knownProviders,
  loadProviders,
  providerOf,
  styleOf,
} from "./providers";
import { registerTileProtocol } from "./tiles";
import { vehicleIcon } from "./vehicleIcons";
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
 *
 * The path a log holds is drawn apart from the track, in the colour of what is known from a file
 * rather than from the wire: during a play-back the two are not the same thing. The track is what
 * has happened so far; the path of the log is the whole flight, and the moment the grid and the
 * graph show can be anywhere in it — a point at two minutes on a line that only drew thirty
 * seconds is a point on a line that is not there.
 */

/** The identifier of the source of the track. */
const TRACK_SOURCE = "track";

/** The identifier of the source of the line that shows where the vehicle is pointing. */
const COURSE_SOURCE = "course";

/** The identifier of the source of the line that runs to the item being flown to. */
const TARGET_SOURCE = "target";

/** The identifier of the source of the plan. */
const PLAN_SOURCE = "plan";

/** The identifier of the source of the items of the plan. */
const ITEMS_SOURCE = "plan-items";

/** The identifier of the source of the circles the items of a plan fly around. */
const PLAN_CIRCLES_SOURCE = "plan-circles";

/** The identifier of the source of the polygon of a fence. */
const FENCE_SOURCE = "fence";

/** The identifier of the source of the circles of a fence. */
const CIRCLES_SOURCE = "fence-circles";

/** The identifier of the source of the home position. */
const HOME_SOURCE = "home";

/** The identifier of the source of the traffic of the air. */
const ADSB_SOURCE = "adsb";

/** The identifier of the source of the places the pictures say they were taken. */
const PICTURES_SOURCE = "pictures";

/** The identifier of the source of the path a log holds. */
const LOG_SOURCE = "log-track";

/** The smallest movement, in degrees, that is worth a point on the track. */
const TRACK_STEP = 1e-5;

/**
 * Whether a position is one the map can be told about.
 *
 * The map library refuses a latitude outside the poles, and it refuses to be told
 * nothing at all. A position that cannot be drawn is skipped, not thrown.
 */
function usable(
  position: {
    readonly lat: number;
    readonly lng: number;
  } | null,
): position is { readonly lat: number; readonly lng: number } {
  if (!position) {
    return false;
  }
  return (
    isPlace(position.lat, position.lng) &&
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
 * Whether a place is a place the map can draw.
 *
 * A vehicle without a fix reports zero for both coordinates, and so does a plan item that was
 * never given one. The point `0°, 0°` is in the Gulf of Guinea, where no vehicle of this station
 * is: a line drawn to it runs from wherever the plan is to the middle of the ocean, which is a
 * line that means nothing. Mission Planner treats the same pair as "no position", and so does
 * this.
 */
function isPlace(lat: number, lng: number): boolean {
  return (
    Number.isFinite(lat) &&
    Number.isFinite(lng) &&
    (Math.abs(lat) > 1e-9 || Math.abs(lng) > 1e-9)
  );
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
    if (!isPlace(item.x, item.y)) {
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
    .filter((item) => isPlace(item.x, item.y))
    .map((item) => [item.y, item.x] as [number, number]);
}

/**
 * The item of the plan the vehicle is flying to.
 *
 * The vehicle says which item it is on; the plan says where that item is. An item without a place
 * is not a target — a delay is on the way, not somewhere — so the first item with a place is used
 * when the one the vehicle names has none.
 */
function currentTarget(telemetry: Telemetry): PlanItem | null {
  const mission = usePlanStore.getState().plans.mission;
  if (mission === null) {
    return null;
  }

  const items = mission.items.filter((item) => isPlace(item.x, item.y));

  if (items.length === 0) {
    return null;
  }

  const seq = telemetry.mission?.seq ?? 0;

  return (
    items.find((item) => item.seq === seq) ??
    items.find((item) => item.seq > 0) ??
    null
  );
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
  lat: number,
  lng: number,
): boolean {
  const west = bounds.getWest();
  const east = bounds.getEast();
  const south = bounds.getSouth();
  const north = bounds.getNorth();

  const marginX = (east - west) * 0.25;
  const marginY = (north - south) * 0.25;

  return (
    lng >= west - marginX &&
    lng <= east + marginX &&
    lat >= south - marginY &&
    lat <= north + marginY
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
  /** The `+` between two items, by the item the new one goes after. */
  const inserts = useRef(new Map<number, maplibregl.Marker>());

  /** The markers of the traffic of the air, by the address of the aircraft. */
  const traffic = useRef(new Map<string, maplibregl.Marker>());
  /** The markers of the places of interest, by the name and place of each. */
  const poiMarkers = useRef(new Map<string, maplibregl.Marker>());
  const fitted = useRef<{
    readonly editor: boolean;
    readonly count: number;
    readonly plan: unknown;
  } | null>(null);

  const [failure, setFailure] = useState<string | null>(null);

  /** The menu of the map, so a press inside it is told apart from a press elsewhere. */
  const menuElement = useRef<HTMLDivElement | null>(null);
  /** The menu of the map: where the right button was pressed, and what is under it. */
  const [menu, setMenu] = useState<{
    readonly x: number;
    readonly y: number;
    readonly lng: number;
    readonly lat: number;
  } | null>(null);

  const t = useLocaleStore((state) => state.t);
  /**
   * Fetches the tiles of the area that is on the map and keeps them.
   *
   * The tiles are the pictures of the ground, and a survey is flown where there is no
   * signal: the tiles of the field are fetched before the flight, and the map is drawn
   * with the ones that are there.
   */
  const saveArea = async () => {
    const instance = map.current;
    const provider = providerOf(mapProvider);

    if (instance === null || provider === null || provider.tiles === null) {
      setSaving(false);
      return;
    }

    const bounds = instance.getBounds();
    const zoom = Math.floor(instance.getZoom());

    const zooms: number[] = [];
    for (let level = 0; level <= levels; level += 1) {
      zooms.push(Math.max(0, zoom - level));
    }

    setBusy(true);

    try {
      const fetched = await prefetchTiles(
        provider.tiles,
        provider.id,
        {
          north: bounds.getNorth(),
          south: bounds.getSouth(),
          east: bounds.getEast(),
          west: bounds.getWest(),
        },
        zooms,
      );

      setNotice(
        fetched === 0
          ? t("The tiles of the area were already on the disk.")
          : t(
              fetched === 1
                ? "{count} tile was fetched and kept."
                : "{count} tiles were fetched and kept.",
              { count: fetched },
            ),
      );
      setSaving(false);
    } catch (cause) {
      setNotice(
        t("{code} — the tiles could not be fetched.", {
          code: normaliseError(cause).code,
        }),
      );
    } finally {
      setBusy(false);
    }
  };

  const [ready, setReady] = useState(false);
  /** Whether the tiles of the area are being fetched. */
  const [saving, setSaving] = useState(false);
  /** How many levels of zoom below the present one are fetched. */
  const [levels, setLevels] = useState(2);
  /** What the last thing the map did said. */
  const [notice, setNotice] = useState<string | null>(null);
  /** Whether something is being done. */
  const [busy, setBusy] = useState(false);

  const follow = useUiStore((state) => state.followVehicle);
  const showControls = useUiStore((state) => state.showMapControls);
  const mapProvider = useUiStore((state) => state.mapProvider);
  const setMapProvider = useUiStore((state) => state.setMapProvider);
  const mapGoing = useUiStore((state) => state.mapGoing);
  const setMapGoing = useUiStore((state) => state.setMapGoing);
  const setFollow = useUiStore((state) => state.setFollowVehicle);
  const toggleControls = useUiStore((state) => state.toggleMapControls);

  const kind = usePlanStore((state) => state.kind);
  const plan = usePlanStore((state) => state.plans[kind]);
  const vehicles = useLinkStore((state) => state.vehicles);
  const linkStatus = useLinkStore((state) => state.status);
  /** The family of the vehicle the station speaks to, as the link reported it. */
  const vehicleKind = chosenVehicle(vehicles)?.kind ?? null;

  const editorOpen = useUiStore((state) => state.screen === "plan");
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

  // The sources of tiles are the backend's, and the backend is the one that fetches
  // them: the map asks it for the tiles it needs.
  const [sources, setSources] = useState<readonly MapProvider[]>([]);
  // A source that could not be read is said as the fact that it could not be read, and not as a
  // sentence kept inside the slot that holds what the map itself said went wrong: that slot is data,
  // and the word about it is worked out when the screen draws it.
  const [sourcesFailure, setSourcesFailure] = useState(false);

  useEffect(() => {
    let cancelled = false;

    void loadProviders()
      .then((found) => {
        if (!cancelled) {
          setSources(found);
        }
      })
      .catch(() => {
        // The map is drawn with what it has: a source that cannot be read is a map
        // without tiles, and the map says so.
        if (!cancelled) {
          setSourcesFailure(true);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  // The map itself, once the sources are known.
  useEffect(() => {
    const element = container.current;
    if (!element || sources.length === 0) {
      return;
    }

    // The provider is read from the store here rather than from the value the component holds: the
    // map is built once with the provider chosen at that moment, and a provider chosen afterwards is
    // applied by the effect that puts a new style on the map that already exists. A dependency on the
    // value would build a second map instead of changing the style of this one.
    const provider = providerOf(useUiStore.getState().mapProvider);
    if (provider === null) {
      return;
    }

    // The tiles are asked for with the local scheme, so that the backend can answer
    // them from the disk.
    registerTileProtocol();

    const instance = new maplibregl.Map({
      container: element,
      style: styleOf(provider),
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
    // A link that is not open has no vehicle to draw. The marker is placed at the centre of the
    // map before anything is known, and a mark sitting there would say a vehicle is there.
    vehicle.hidden = true;
    // What the board is drawn as: the family the link reported, or the delta it falls back on
    // before a heartbeat has said what the board is.
    vehicle.innerHTML = vehicleIcon(
      chosenVehicle(useLinkStore.getState().vehicles)?.kind ?? null,
    );
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
      // The map is left where a test can reach it, in a development build only. What the map draws
      // is a WebGL canvas, so the only way to read it from a script is to ask the map: the zoom it
      // is at, and the features each source holds.
      if (import.meta.env.DEV) {
        (window as unknown as { __mpcraftMap?: maplibregl.Map }).__mpcraftMap =
          instance;
      }

      instance.addSource(COURSE_SOURCE, {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: { type: "LineString", coordinates: [] },
        },
      });
      instance.addSource(TARGET_SOURCE, {
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
      instance.addSource(PLAN_CIRCLES_SOURCE, {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });

      instance.addSource(CIRCLES_SOURCE, {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });
      instance.addSource(HOME_SOURCE, {
        type: "geojson",
        data: { type: "FeatureCollection", features: [] },
      });

      // The traffic of the air: what the vehicle has heard.
      instance.addSource(ADSB_SOURCE, {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: [],
        },
      });

      // The pictures: a point for every one that has a place to be drawn at.
      instance.addSource(PICTURES_SOURCE, {
        type: "geojson",
        data: {
          type: "FeatureCollection",
          features: [],
        },
      });

      // The path a log holds: the whole flight, not the part of it that has been played.
      instance.addSource(LOG_SOURCE, {
        type: "geojson",
        data: {
          type: "Feature",
          properties: {},
          geometry: { type: "LineString", coordinates: [] },
        },
      });

      // The path of the log is drawn under the track, because during a play-back the track is what
      // has happened so far and the path is the flight: the two are known in two different ways,
      // one from the file and one from the wire, and they are drawn as two things.
      instance.addLayer({
        id: "log-line",
        type: "line",
        source: LOG_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#a371f7",
          "line-width": 1.5,
          "line-opacity": 0.5,
        },
      });

      instance.addLayer({
        id: "track-line",
        type: "line",
        source: TRACK_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#00c896",
          "line-width": 2,
          "line-opacity": 0.8,
        },
      });

      // The two lines Mission Planner draws from the vehicle: the heading it is pointing at, in
      // the colour of a heading, and the line that runs to the item it is flying to. They are
      // drawn under the plan, because they are readings of the vehicle and the plan is the plan.
      instance.addLayer({
        id: "course-line",
        type: "line",
        source: COURSE_SOURCE,
        layout: { "line-cap": "round" },
        paint: {
          "line-color": "#f85149",
          "line-width": 2,
          "line-opacity": 0.8,
        },
      });

      instance.addLayer({
        id: "target-line",
        type: "line",
        source: TARGET_SOURCE,
        layout: { "line-cap": "round" },
        paint: {
          "line-color": "#d29922",
          "line-width": 1.5,
          "line-opacity": 0.9,
        },
      });

      // The line of the plan is drawn twice: a dark halo under a light dashed stroke. The
      // dashes are what an operator reads as "planned, not flown" — the track is the solid
      // line — and the halo is what keeps both of them legible over a pale map and a satellite
      // one, which no single colour does.
      instance.addLayer({
        id: "plan-halo",
        type: "line",
        source: PLAN_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#0d1117",
          "line-width": 4.5,
          "line-opacity": 0.45,
        },
      });

      instance.addLayer({
        id: "plan-line",
        type: "line",
        source: PLAN_SOURCE,
        layout: { "line-cap": "round", "line-join": "round" },
        paint: {
          "line-color": "#e6edf3",
          "line-width": 2,
          "line-opacity": 0.95,
          "line-dasharray": [3, 2],
        },
      });

      // The circles the loiter items are flown around: the radius of the turn, which is a
      // number in the grid and a shape on the map.
      instance.addLayer({
        id: "plan-circles",
        type: "line",
        source: PLAN_CIRCLES_SOURCE,
        layout: { "line-join": "round" },
        paint: {
          "line-color": "#00c896",
          "line-width": 1.5,
          "line-opacity": 0.85,
          "line-dasharray": [3, 2],
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

      // The traffic of the air, drawn as a dot with a ring around it.
      instance.addLayer({
        id: "adsb-point",
        type: "circle",
        source: ADSB_SOURCE,
        paint: {
          "circle-radius": 4,
          "circle-color": "#f85149",
          "circle-stroke-color": "#f85149",
          "circle-stroke-width": 1,
          "circle-opacity": 0.35,
        },
      });

      // The pictures. The place the file itself carries and the place the log says the
      // vehicle was when the picture was made are drawn apart, because the ground station
      // knows one from the file and worked the other out from the flight; a picture that
      // the log does not hold has only the first of them.
      instance.addLayer({
        id: "pictures-file",
        type: "circle",
        source: PICTURES_SOURCE,
        filter: ["==", ["get", "kind"], "file"],
        paint: {
          "circle-radius": 5,
          "circle-color": "#58a6ff",
          "circle-stroke-color": "#0d1117",
          "circle-stroke-width": 1.5,
        },
      });

      instance.addLayer({
        id: "pictures-log",
        type: "circle",
        source: PICTURES_SOURCE,
        filter: ["==", ["get", "kind"], "log"],
        paint: {
          "circle-radius": 4,
          "circle-color": "#a371f7",
          "circle-stroke-color": "#0d1117",
          "circle-stroke-width": 1,
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
        event.error instanceof Error
          ? event.error.message
          : String(event.error);
      setFailure(message);
    });

    // The map has drawn itself: whatever went wrong before is over, and the message
    // about it goes away with it.
    instance.on("idle", () => {
      setFailure(null);
    });

    // A click on the map does one of two things: it tells the vehicle to go there, when
    // the tool is armed, and it adds an item to the plan, when the editor is armed for
    // it.
    instance.on("click", (event) => {
      // A click marks a place of interest when the sheet armed the map for it. The point is
      // added before the other tools look at the click, because the operator said what this
      // click is for.
      if (useUiStore.getState().poiPlacing) {
        const name = useUiStore.getState().poiName;

        useUiStore.getState().cancelPoiPlacement();
        void usePoiStore
          .getState()
          .add(name, event.lngLat.lat, event.lngLat.lng);
        return;
      }

      const going = useUiStore.getState().mapGoing;

      if (going) {
        useUiStore.getState().setMapGoing(false);

        // The vehicle keeps the height it has: the operator said where, not how high.
        const altitude = liveTelemetry.value.position?.alt ?? 0;

        void flyTo(event.lngLat.lat, event.lngLat.lng, altitude);
        return;
      }

      const editor = usePlanEditorStore.getState();
      if (!onPlanScreen() || !editor.adding) {
        return;
      }
      editor.addAt(event.lngLat.lat, event.lngLat.lng);
    });

    // The right button opens the menu of the map at the place it was pressed. The menu is the
    // station's: what an operator reaches for without leaving the map — send the vehicle there,
    // add an item of the plan, move the home, centre the map, take the coordinates away.
    instance.on("contextmenu", (event) => {
      event.preventDefault();
      setMenu({
        x: event.point.x,
        y: event.point.y,
        lng: event.lngLat.lng,
        lat: event.lngLat.lat,
      });
    });

    const observer = new ResizeObserver(() => {
      instance.resize();
    });
    observer.observe(element);

    // The refs are copied here rather than read inside the cleanup: by the time the cleanup runs, a
    // ref can point at something else, and the markers that this effect put on the map are the ones
    // it has to take off.
    const markers = items.current;
    const places = poiMarkers.current;
    const between = inserts.current;

    return () => {
      observer.disconnect();
      marker.current?.remove();
      marker.current = null;
      for (const entry of markers.values()) {
        entry.marker.remove();
      }
      markers.clear();
      for (const middle of between.values()) {
        middle.remove();
      }
      between.clear();
      for (const place of places.values()) {
        place.remove();
      }
      places.clear();
      instance.remove();
      map.current = null;
      loaded.current = false;
    };
  }, [sources]);

  // The traffic of the air: what the vehicle has heard is drawn on the map.
  const adsb = useLinkStore((state) => state.adsb);

  useEffect(() => {
    const instance = map.current;
    if (!instance || !ready) {
      return;
    }

    const features = adsb
      .filter(
        (aircraft) =>
          Number.isFinite(aircraft.latitude) &&
          Number.isFinite(aircraft.longitude),
      )
      .map((aircraft) => ({
        type: "Feature" as const,
        properties: {
          callsign: aircraft.callsign ?? "",
          altitude: aircraft.altitude,
          speed: aircraft.speed,
        },
        geometry: {
          type: "Point" as const,
          coordinates: [aircraft.longitude, aircraft.latitude],
        },
      }));

    setSourceData(instance, ADSB_SOURCE, {
      type: "FeatureCollection",
      features,
    });

    // The name of each aircraft is written beside it, with a marker of its own.
    const seen = new Set<string>();

    for (const aircraft of adsb) {
      const key = String(aircraft.icaoAddress);
      seen.add(key);

      let marker = traffic.current.get(key);

      if (marker === undefined) {
        const element = document.createElement("div");
        element.className = styles.adsb ?? "";
        element.setAttribute("role", "presentation");
        element.setAttribute("aria-hidden", "true");

        marker = new maplibregl.Marker({ element, anchor: "bottom" });

        // The marker needs a place before it is put on the map: until it has one, the
        // map has nothing to draw it at.
        marker.setLngLat([aircraft.longitude, aircraft.latitude]);

        traffic.current.set(key, marker);
        marker.addTo(instance);
      }

      marker.getElement().textContent =
        aircraft.callsign ??
        `0x${aircraft.icaoAddress.toString(16).toUpperCase()}`;

      marker.setLngLat([aircraft.longitude, aircraft.latitude]);
    }

    for (const [key, marker] of traffic.current) {
      if (!seen.has(key)) {
        marker.remove();
        traffic.current.delete(key);
      }
    }
  }, [adsb, ready]);

  // The places of interest: a place is a name and a position, and it is drawn on both the map of
  // the flight and the map of the plan because the store is the same one.
  const pois = usePoiStore((state) => state.points);

  useEffect(() => {
    const instance = map.current;
    if (!instance || !ready) {
      return;
    }

    const seen = new Set<string>();

    for (const poi of pois) {
      const key = `${poi.name}|${poi.latitude}|${poi.longitude}`;
      seen.add(key);

      let marker = poiMarkers.current.get(key);

      if (marker === undefined) {
        const element = document.createElement("div");
        element.className = styles.poi ?? "";
        element.textContent = poi.name;

        marker = new maplibregl.Marker({ element, anchor: "bottom" });
        marker.setLngLat([poi.longitude, poi.latitude]);

        poiMarkers.current.set(key, marker);
        marker.addTo(instance);
      }

      marker.setLngLat([poi.longitude, poi.latitude]);
    }

    for (const [key, marker] of poiMarkers.current) {
      if (!seen.has(key)) {
        marker.remove();
        poiMarkers.current.delete(key);
      }
    }
  }, [pois, ready]);

  // The pictures: the place the file itself carries and the place the log says the vehicle was
  // when each picture was made, drawn apart because they are known in two different ways.
  const pictures = usePictureStore((state) => state.pictures);

  useEffect(() => {
    const instance = map.current;
    if (!instance || !ready) {
      return;
    }

    const features = pictures.flatMap((picture) => {
      const places: { kind: string; point: GeoPoint }[] = [];

      if (picture.place !== null) {
        places.push({ kind: "file", point: picture.place });
      }

      if (picture.matchedPlace !== null) {
        places.push({ kind: "log", point: picture.matchedPlace });
      }

      return places.map(({ kind, point }) => ({
        type: "Feature" as const,
        properties: { name: picture.name, kind },
        geometry: {
          type: "Point" as const,
          coordinates: [point.lng, point.lat],
        },
      }));
    });

    setSourceData(instance, PICTURES_SOURCE, {
      type: "FeatureCollection",
      features,
    });
  }, [pictures, ready]);

  // The path of the log that is open on the screen of the logs. It is drawn for the whole flight,
  // so that the moment a row of the grid shows is a moment on a line that is there — and it is not
  // drawn when the flight has been let go of, because then the map is showing the vehicle on the
  // wire again.
  const trackOfLog = useLogTrackStore((state) => state.points);

  useEffect(() => {
    const instance = map.current;
    if (!instance || !ready) {
      return;
    }

    setSourceData(instance, LOG_SOURCE, {
      type: "Feature",
      properties: {},
      geometry: {
        type: "LineString",
        coordinates: lineOf(trackOfLog),
      },
    });
  }, [trackOfLog, ready]);

  // The provider of the tiles. The style of the map is the provider's, and the sources
  // and the layers go with it: they are put back when the new style has loaded.
  useEffect(() => {
    const instance = map.current;
    if (!instance || !loaded.current) {
      return;
    }

    const provider = providerOf(mapProvider);
    if (provider === null) {
      return;
    }

    instance.setStyle(styleOf(provider));

    instance.once("styledata", () => {
      install.current?.();
    });
  }, [mapProvider, sources]);

  // The menu of the map closes on anything that is not the menu: a click elsewhere, or Escape.
  // It is closed before the action is read rather than after, so an action that opens a sheet
  // does not leave the menu behind it.
  useEffect(() => {
    if (menu === null) {
      return;
    }

    const close = (event: PointerEvent) => {
      // A press inside the menu is the operator choosing something: closing on it would take
      // the button away before the click it belongs to arrives.
      if (
        event.target instanceof Node &&
        menuElement.current?.contains(event.target)
      ) {
        return;
      }
      setMenu(null);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setMenu(null);
      }
    };

    window.addEventListener("pointerdown", close);
    window.addEventListener("keydown", escape);

    return () => {
      window.removeEventListener("pointerdown", close);
      window.removeEventListener("keydown", escape);
    };
  }, [menu]);

  // A vehicle that is not on the link is not drawn. The mark says where the vehicle is, and the
  // last place it was is not where it is: a station with no link has nothing to point at.
  useEffect(() => {
    const element = marker.current?.getElement();
    if (
      element &&
      linkStatus !== "connected" &&
      linkStatus !== "reconnecting"
    ) {
      element.hidden = true;
    }
  }, [linkStatus]);

  // The mark of the vehicle, by the family it belongs to. The element is kept and only its
  // drawing is replaced, so a board that says what it is a moment after it is heard changes from
  // the delta to its own shape without the marker being taken off the map.
  useEffect(() => {
    const element = marker.current?.getElement();
    if (element) {
      element.innerHTML = vehicleIcon(vehicleKind);
    }
  }, [vehicleKind]);

  // The position, the heading and the track, written into the map directly.
  useEffect(() => {
    const draw = (telemetry: Telemetry) => {
      const instance = map.current;
      const position = telemetry.position;

      if (!instance) {
        return;
      }

      // The place the vehicle started from, when it has reported one. A home that was never set
      // is reported as zero, and a mark at `0°, 0°` is a home in the Gulf of Guinea that no
      // vehicle has.
      const home = telemetry.home;
      const homePlace = home !== null && isPlace(home.lat, home.lng);
      if (loaded.current && home !== lastHome.current) {
        lastHome.current = home;
        setSourceData(instance, HOME_SOURCE, {
          type: "FeatureCollection",
          features: homePlace
            ? [
                {
                  type: "Feature",
                  properties: {},
                  geometry: {
                    type: "Point",
                    coordinates: [home.lng, home.lat],
                  },
                },
              ]
            : [],
        });
      }

      // A vehicle is drawn where it says it is, and a vehicle that has no fix says it is nowhere.
      // The home it reports is the other place it is known to be: an operator watches the board
      // it will come back to, so the mark sits there rather than nowhere.
      const place = usable(position)
        ? { lat: position.lat, lng: position.lng }
        : homePlace
          ? { lat: home.lat, lng: home.lng }
          : null;

      const element = marker.current?.getElement();

      if (place === null) {
        if (element) {
          element.hidden = true;
        }
        for (const id of [COURSE_SOURCE, TARGET_SOURCE]) {
          setSourceData(instance, id, {
            type: "Feature",
            properties: {},
            geometry: { type: "LineString", coordinates: [] },
          });
        }
        return;
      }

      const point: [number, number] = [place.lng, place.lat];
      marker.current?.setLngLat(point);

      if (element) {
        element.hidden = false;
      }

      // The line of the heading: the direction the nose points, not the direction the vehicle is
      // travelling — with a wind from the side the two are not the same, and the operator wants
      // to see both. It is short, and the same length whatever the speed.
      const heading = telemetry.attitude?.yaw ?? 0;
      marker.current?.setRotation(heading);

      const course = instance.getSource(COURSE_SOURCE);
      if (course) {
        const line = courseLine(place.lat, place.lng, heading, 150);
        setSourceData(instance, COURSE_SOURCE, {
          type: "Feature",
          properties: {},
          geometry: { type: "LineString", coordinates: [line.start, line.end] },
        });
      }

      // The line to the item the plan is flying to: the leg being flown now, which is what the
      // vehicle is being steered to rather than where it happens to point.
      const target = currentTarget(telemetry);
      const targetSource = instance.getSource(TARGET_SOURCE);
      if (targetSource) {
        setSourceData(instance, TARGET_SOURCE, {
          type: "Feature",
          properties: {},
          geometry: {
            type: "LineString",
            coordinates:
              target === null
                ? []
                : [
                    [place.lng, place.lat],
                    [target.y, target.x],
                  ],
          },
        });
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

    // The path of the plan is a straight line from one place to the next, unless the
    // operator asks for the smooth one: the curve the vehicle follows when it is told
    // to follow a spline.
    const coordinates =
      useUiStore.getState().spline && shown.kind !== "fence"
        ? splinePath(places).map(
            (place) => [place.y, place.x] as [number, number],
          )
        : coordinatesOf(places);

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

    // The circles the loiter items are flown around, which are in a mission and not in a fence.
    const planCircles = instance.getSource(PLAN_CIRCLES_SOURCE);
    if (planCircles) {
      setSourceData(instance, PLAN_CIRCLES_SOURCE, {
        type: "FeatureCollection",
        features:
          shown.kind === "mission"
            ? loiterCircles(shown.items, commands).map((circle) => ({
                type: "Feature",
                properties: { seq: circle.seq },
                geometry: {
                  type: "Polygon",
                  coordinates: [circleRing({ ...circle, included: true })],
                },
              }))
            : [],
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

    if (!editorOpen) {
      for (const entry of items.current.values()) {
        entry.marker.remove();
      }
      items.current.clear();
      for (const marker of inserts.current.values()) {
        marker.remove();
      }
      inserts.current.clear();
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
      if (!inView(bounds, item.x, item.y)) {
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
          item.command === VERTEX_EXCLUSION
            ? (styles.excludedItem ?? "")
            : (styles.item ?? "");
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
        element.classList.toggle(
          styles.itemSelected,
          item.seq === editor.selected,
        );
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

    // The `+` between two items, the way Mission Planner puts one on every leg: a press adds a
    // waypoint at the middle of that leg without moving anything else. They are buttons of
    // their own rather than marks of the map, because they are pressed rather than read.
    const seenInserts = new Set<number>();
    let insertsDrawn = 0;

    for (const place of insertionPoints(places)) {
      if (insertsDrawn >= MAX_MARKERS) {
        break;
      }
      if (!inView(bounds, place.lat, place.lng)) {
        continue;
      }

      seenInserts.add(place.after);
      insertsDrawn += 1;

      let marker = inserts.current.get(place.after);
      if (marker === undefined) {
        const element = document.createElement("button");
        element.type = "button";
        element.className = styles.insert ?? "";
        element.textContent = "+";
        element.title = t("Add a waypoint between the two");
        element.setAttribute("aria-label", t("Add a waypoint between the two"));

        element.addEventListener("click", (event) => {
          event.stopPropagation();

          // The place is read from the plan as it stands, not from the closure: the leg may
          // have been dragged since this button was made.
          const editor = usePlanEditorStore.getState();
          const at = insertionPoints(
            positioned(editor.draft, useCommandStore.getState().commands),
          ).find((entry) => entry.after === place.after);

          if (at === undefined) {
            return;
          }

          editor.select(at.after);
          editor.addAt(at.lat, at.lng);
        });

        const created = new maplibregl.Marker({
          element,
          anchor: "center",
        });
        created.setLngLat([place.lng, place.lat]);
        created.addTo(instance);
        inserts.current.set(place.after, created);
        marker = created;
      }

      marker.setLngLat([place.lng, place.lat]);
    }

    for (const [after, marker] of inserts.current) {
      if (!seenInserts.has(after)) {
        marker.remove();
        inserts.current.delete(after);
      }
    }

    // The operator is told when the map is not showing every item of the plan.
    usePlanEditorStore.getState().setMarked(drawn, places.length);
    // The plan on the screen is the plan being edited only while the editor is the screen that is
    // open, so the guard is part of what this callback reads.
  }, [editorOpen, t]);

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
      // An empty plan has nothing to frame, and the editor is marked as framed all the same: the
      // first point the operator adds would otherwise be framed on, and the map would jump under
      // the hand that is drawing it.
      fitted.current = { editor: editorOpen, count: shown.items.length, plan };
      return;
    }

    // While the plan is being edited the map is where the operator put it: adding a point or
    // dragging one is not a reason to move the view, and a map that re-framed itself on every
    // press would zoom in and out under the hand that is drawing. The editor frames the plan
    // once, when it opens.
    const last = fitted.current;
    if (last !== null && last.editor === editorOpen) {
      if (editorOpen) {
        return;
      }
      if (last.count === shown.items.length && last.plan === plan) {
        return;
      }
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

  // The menu stays inside the map: a right click near the right or the bottom edge would
  // otherwise open a menu that is half outside the window.
  const menuPlacement =
    menu === null
      ? null
      : {
          left: Math.max(
            8,
            Math.min(menu.x, (container.current?.clientWidth ?? 0) - 232),
          ),
          top: Math.max(
            8,
            Math.min(menu.y, (container.current?.clientHeight ?? 0) - 248),
          ),
        };

  return (
    <div className={styles.map}>
      <div className={styles.canvas} ref={container} />

      <div className={styles.controls}>
        {showControls && adsb.length > 0 ? (
          <span
            className={styles.traffic}
            title={t("The aircraft the vehicle has heard")}
          >
            {t("{count} in the air", { count: adsb.length })}
          </span>
        ) : null}

        {showControls ? (
          <select
            className={styles.provider}
            value={mapProvider}
            aria-label={t("The provider of the tiles")}
            title={t("The provider of the tiles")}
            onChange={(event) => {
              setMapProvider(event.target.value);
            }}
          >
            {knownProviders().map((provider) => (
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
            {follow ? t("Following") : t("Follow")}
          </button>
        ) : null}

        {showControls ? (
          <button
            type="button"
            className={`${styles.follow} ${mapGoing ? styles.followOn : ""}`}
            aria-pressed={mapGoing}
            title={t("Click the map to tell the vehicle to go there")}
            onClick={() => setMapGoing(!mapGoing)}
          >
            {mapGoing ? t("Click the map") : t("Go here")}
          </button>
        ) : null}
        {providerOf(mapProvider)?.tiles !== null &&
        providerOf(mapProvider) !== null ? (
          <button
            type="button"
            className={styles.control}
            title={t(
              "Fetch the tiles of this area and keep them, for a flight without a signal",
            )}
            onClick={() => {
              setSaving(true);
            }}
          >
            {t("Save the area")}
          </button>
        ) : null}
        <button
          type="button"
          className={styles.control}
          aria-pressed={showControls}
          onClick={toggleControls}
        >
          {showControls ? t("Hide controls") : t("Show controls")}
        </button>
      </div>

      {menu !== null && menuPlacement !== null ? (
        <div
          className={styles.menu}
          style={{ left: menuPlacement.left, top: menuPlacement.top }}
          role="menu"
          aria-label={t("The map menu")}
          ref={menuElement}
        >
          <p className={styles.menuWhere}>
            {menu.lat.toFixed(5)}, {menu.lng.toFixed(5)}
          </p>

          {vehicles.length > 0 ? (
            <button
              type="button"
              role="menuitem"
              className={styles.menuItem}
              onClick={() => {
                const altitude = liveTelemetry.value.position?.alt ?? 0;
                void flyTo(menu.lat, menu.lng, altitude);
                setMenu(null);
              }}
            >
              {t("Send the vehicle here")}
            </button>
          ) : null}

          {editorOpen ? (
            <button
              type="button"
              role="menuitem"
              className={styles.menuItem}
              onClick={() => {
                usePlanEditorStore.getState().addAt(menu.lat, menu.lng);
                setMenu(null);
              }}
            >
              {t("Add a waypoint here")}
            </button>
          ) : null}

          {vehicles.length > 0 ? (
            <button
              type="button"
              role="menuitem"
              className={styles.menuItem}
              onClick={() => {
                const altitude = liveTelemetry.value.position?.alt ?? 0;
                void setHomePosition({
                  lat: menu.lat,
                  lng: menu.lng,
                  alt: altitude,
                });
                setMenu(null);
              }}
            >
              {t("Move the home here")}
            </button>
          ) : null}

          <button
            type="button"
            role="menuitem"
            className={styles.menuItem}
            onClick={() => {
              const instance = map.current;
              if (instance) {
                instance.easeTo({
                  center: [menu.lng, menu.lat],
                  duration: 400,
                });
              }
              setMenu(null);
            }}
          >
            {t("Centre the map here")}
          </button>

          <button
            type="button"
            role="menuitem"
            className={styles.menuItem}
            onClick={() => {
              const text = `${menu.lat.toFixed(6)}, ${menu.lng.toFixed(6)}`;
              void navigator.clipboard
                ?.writeText(text)
                .then(() => {
                  setNotice(t("The coordinates were copied."));
                })
                .catch(() => {
                  setNotice(t("The coordinates could not be copied."));
                });
              setMenu(null);
            }}
          >
            {t("Copy the coordinates")}
          </button>
        </div>
      ) : null}

      <div className={styles.messages}>
        {saving ? (
          <div
            className={styles.area}
            role="group"
            aria-label={t("Save the area")}
          >
            <p className={styles.hint}>
              {t(
                "The tiles of the area are fetched and kept, so that the map is drawn with them when there is no signal.",
              )}
            </p>

            <label className={styles.areaField}>
              <span className={styles.hint}>{t("Levels below")}</span>
              <input
                className="input"
                type="number"
                min={0}
                max={8}
                value={levels}
                aria-label={t(
                  "How many levels of zoom below the present one are fetched",
                )}
                onChange={(event) => {
                  setLevels(Number(event.target.value));
                }}
              />
            </label>

            <div className={styles.areaActions}>
              <button
                type="button"
                className="button button--primary"
                disabled={busy}
                onClick={() => {
                  void saveArea();
                }}
              >
                {busy ? t("Fetching…") : t("Fetch the tiles")}
              </button>

              <button
                type="button"
                className="button"
                onClick={() => {
                  setSaving(false);
                }}
              >
                {t("Cancel")}
              </button>
            </div>
          </div>
        ) : null}

        {notice !== null ? (
          <p className={styles.hint} role="status">
            {notice}
          </p>
        ) : null}

        {adding ? (
          <p className={styles.hint} role="status">
            {t("Click the map to add a waypoint")}
          </p>
        ) : null}

        {sourcesFailure ? (
          <p className={styles.failure} role="alert">
            {t("The sources of tiles could not be read.")}
          </p>
        ) : null}

        {failure ? (
          <p className={styles.failure} role="alert">
            {t("The map could not be drawn: {failure}", { failure })}
          </p>
        ) : null}
      </div>

      <p className={styles.caption}>
        {linkStatus === "connected" || linkStatus === "reconnecting"
          ? vehicles.length > 0
            ? t(
                vehicles.length === 1
                  ? "{count} vehicle on the link"
                  : "{count} vehicles on the link",
                { count: vehicles.length },
              )
            : t("No vehicle on the link")
          : null}
      </p>
    </div>
  );
}
