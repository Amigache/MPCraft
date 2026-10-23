import { useEffect, useRef, useState } from "react";

import { Splitter } from "../../app/layout/Splitter";
import { TabStrip } from "../../components/TabStrip";
import { ActionsPanel } from "../../features/actions/ActionsPanel";
import { FlightMap } from "../../features/map/FlightMap";
import { GaugesPanel } from "../../features/gauges/GaugesPanel";
import { GraphPanel } from "../../features/graphs/GraphPanel";
import { Hud } from "../../features/hud/Hud";
import { MessageConsole } from "../../features/messages/MessageConsole";
import { PlanEditor } from "../../features/plan/PlanEditor";
import { PreFlightPanel } from "../../features/preflight/PreFlightPanel";
import { QuickViewPanel } from "../../features/quick-view/QuickViewPanel";
import { ServoPanel } from "../../features/servos/ServoPanel";
import { AuxPanel } from "../../features/servos/AuxPanel";
import { StatusPanel } from "../../features/status/StatusPanel";
import {
  MAX_GRAPH_HEIGHT,
  MAX_HUD_HEIGHT,
  MAX_LEFT_WIDTH,
  MIN_GRAPH_HEIGHT,
  MIN_HUD_HEIGHT,
  MIN_LEFT_WIDTH,
  MIN_MAP_HEIGHT,
  MIN_PLAN_HEIGHT,
  TABS,
  useUiStore,
} from "../../lib/ui/uiStore";
import { useWindowSize } from "../../lib/ui/windowSize";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import styles from "./FlightData.module.css";

/**
 * The flight data screen.
 *
 * The distribution is upstream's: a column on the left, the width of which can be
 * dragged, holding the attitude indicator above a set of tabs, and the map filling
 * everything else.
 *
 * The area on the right of the column is split in two, as upstream's is: the graphs
 * above, the map below, with an edge between them that can be dragged. The graphs can
 * be put away, and the map then takes the whole area.
 *
 * The plan editor takes the place of the graphs while a plan is being edited: upstream
 * edits a plan in a window of its own, and MPCraft is one window. The map stays where
 * it is, because the map is where the plan is edited.
 */
export function FlightData() {
  const showLeftColumn = useUiStore((state) => state.showLeftColumn);
  const focusMap = useUiStore((state) => state.focusMap);
  const leftColumnFraction = useUiStore((state) => state.leftColumnFraction);
  const hudFraction = useUiStore((state) => state.hudFraction);
  const topFraction = useUiStore((state) => state.topFraction);
  const showGraphs = useUiStore((state) => state.showGraphs);
  const activeTab = useUiStore((state) => state.activeTab);
  const setActiveTab = useUiStore((state) => state.setActiveTab);
  const t = useLocaleStore((state) => state.t);
  const setLeftColumnFraction = useUiStore(
    (state) => state.setLeftColumnFraction,
  );
  const setHudFraction = useUiStore((state) => state.setHudFraction);
  const setTopFraction = useUiStore((state) => state.setTopFraction);

  // The flight data and the plan are the same distribution — the plan only changes what
  // goes above the map — so they share this component, and the component draws nothing
  // while the window is on another screen.
  const screen = useUiStore((state) => state.screen);
  const editorOpen = screen === "plan";
  const shown = screen === "flightdata" || screen === "plan";

  // The size of the window, and the height of the area the graphs and the map share.
  // They decide how many pixels the proportions are worth.
  const windowSize = useWindowSize();
  const split = useRef<HTMLDivElement | null>(null);
  const [available, setAvailable] = useState(0);

  useEffect(() => {
    if (!shown) {
      return;
    }

    const element = split.current;
    if (!element) {
      return;
    }

    const measure = () => {
      setAvailable(element.clientHeight);
    };

    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(element);

    return () => observer.disconnect();
  }, [shown]);

  const tallestGraph = Math.max(
    MIN_GRAPH_HEIGHT,
    Math.min(MAX_GRAPH_HEIGHT, available - MIN_MAP_HEIGHT),
  );

  // The editor needs more room than a graph does: it holds a grid and a row of
  // buttons, and a grid that is two rows tall is not a grid.
  const minTop = editorOpen ? MIN_PLAN_HEIGHT : MIN_GRAPH_HEIGHT;
  const maxTop = Math.max(minTop, tallestGraph);

  // The layout is kept as proportions, and the pixels are worked out here: the column
  // is a part of the window, the attitude indicator a part of the column, and the pane
  // above the map a part of the area it shares with the map.
  const leftColumnWidth = Math.min(
    MAX_LEFT_WIDTH,
    Math.max(MIN_LEFT_WIDTH, Math.round(windowSize.width * leftColumnFraction)),
  );

  const hudHeight = Math.min(
    MAX_HUD_HEIGHT,
    Math.max(MIN_HUD_HEIGHT, Math.round(windowSize.height * hudFraction)),
  );

  const drawnGraphHeight = Math.min(
    Math.max(Math.round(available * topFraction), minTop),
    maxTop,
  );

  // The graphs are put away while the plan is being edited.
  const graphsVisible = showGraphs && !editorOpen && !focusMap;

  if (!shown) {
    return null;
  }

  return (
    <div className={styles.screen}>
      {showLeftColumn && !focusMap ? (
        <>
          <aside
            className={styles.left}
            style={{ width: `${leftColumnWidth}px` }}
            aria-label={t("Flight data")}
          >
            <div className={styles.hud} style={{ height: `${hudHeight}px` }}>
              <Hud />
            </div>

            <Splitter
              orientation="horizontal"
              label={t("The height of the attitude indicator")}
              value={hudHeight}
              min={MIN_HUD_HEIGHT}
              max={MAX_HUD_HEIGHT}
              onChange={(height) => {
                setHudFraction(height / Math.max(1, windowSize.height));
              }}
            />

            <div className={styles.tabs}>
              <TabStrip
                tabs={TABS.map((entry) => ({
                  id: entry.id,
                  label: t(entry.label),
                }))}
                active={activeTab}
                onSelect={setActiveTab}
              />

              {/*
                Every panel stays mounted, so that the console keeps what it has
                collected and the tiles keep their choice when the tab is changed.
              */}
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-quick"
                aria-labelledby="tab-quick"
                hidden={activeTab !== "quick"}
              >
                <QuickViewPanel />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-actions"
                aria-labelledby="tab-actions"
                hidden={activeTab !== "actions"}
              >
                <ActionsPanel />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-messages"
                aria-labelledby="tab-messages"
                hidden={activeTab !== "messages"}
              >
                <MessageConsole />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-gauges"
                aria-labelledby="tab-gauges"
                hidden={activeTab !== "gauges"}
              >
                <GaugesPanel />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-status"
                aria-labelledby="tab-status"
                hidden={activeTab !== "status"}
              >
                <StatusPanel />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-servos"
                aria-labelledby="tab-servos"
                hidden={activeTab !== "servos"}
              >
                <ServoPanel />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-aux"
                aria-labelledby="tab-aux"
                hidden={activeTab !== "aux"}
              >
                <AuxPanel />
              </div>
              <div
                className={styles.panel}
                role="tabpanel"
                id="panel-preflight"
                aria-labelledby="tab-preflight"
                hidden={activeTab !== "preflight"}
              >
                <PreFlightPanel />
              </div>
            </div>
          </aside>

          <Splitter
            orientation="vertical"
            label={t("The width of the flight data column")}
            value={leftColumnWidth}
            min={MIN_LEFT_WIDTH}
            max={MAX_LEFT_WIDTH}
            onChange={(width) => {
              setLeftColumnFraction(width / Math.max(1, windowSize.width));
            }}
          />
        </>
      ) : null}

      <div className={styles.main}>
        <div className={styles.split} ref={split}>
          {editorOpen && !focusMap ? (
            <>
              <div
                className={styles.editor}
                style={{ height: `${drawnGraphHeight}px` }}
              >
                <PlanEditor />
              </div>

              <Splitter
                orientation="horizontal"
                label="The height of the plan editor"
                value={drawnGraphHeight}
                min={minTop}
                max={maxTop}
                onChange={(height) => {
                  setTopFraction(height / Math.max(1, available));
                }}
              />
            </>
          ) : graphsVisible ? (
            <>
              <div
                className={styles.graphs}
                style={{ height: `${drawnGraphHeight}px` }}
              >
                <GraphPanel />
              </div>

              <Splitter
                orientation="horizontal"
                label="The height of the graphs"
                value={drawnGraphHeight}
                min={MIN_GRAPH_HEIGHT}
                max={maxTop}
                onChange={(height) => {
                  setTopFraction(height / Math.max(1, available));
                }}
              />
            </>
          ) : null}

          <div className={styles.mapArea}>
            <FlightMap />
          </div>
        </div>
      </div>
    </div>
  );
}
