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
import { StatusPanel } from "../../features/status/StatusPanel";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
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
  const leftColumnWidth = useUiStore((state) => state.leftColumnWidth);
  const hudHeight = useUiStore((state) => state.hudHeight);
  const graphHeight = useUiStore((state) => state.graphHeight);
  const showGraphs = useUiStore((state) => state.showGraphs);
  const activeTab = useUiStore((state) => state.activeTab);
  const setActiveTab = useUiStore((state) => state.setActiveTab);
  const setLeftColumnWidth = useUiStore((state) => state.setLeftColumnWidth);
  const setHudHeight = useUiStore((state) => state.setHudHeight);
  const setGraphHeight = useUiStore((state) => state.setGraphHeight);

  const editorOpen = usePlanEditorStore((state) => state.open);

  // The height of the area the graphs and the map share. It decides how tall the
  // graphs may be: the map is never left smaller than it can be worked in.
  const split = useRef<HTMLDivElement | null>(null);
  const [available, setAvailable] = useState(0);

  useEffect(() => {
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
  }, []);

  const tallestGraph = Math.max(
    MIN_GRAPH_HEIGHT,
    Math.min(MAX_GRAPH_HEIGHT, available - MIN_MAP_HEIGHT),
  );

  // The editor needs more room than a graph does: it holds a grid and a row of
  // buttons, and a grid that is two rows tall is not a grid.
  const minTop = editorOpen ? MIN_PLAN_HEIGHT : MIN_GRAPH_HEIGHT;
  const maxTop = Math.max(minTop, tallestGraph);
  const drawnGraphHeight = Math.min(Math.max(graphHeight, minTop), maxTop);

  // The graphs are put away while the plan is being edited.
  const graphsVisible = showGraphs && !editorOpen;

  return (
    <div className={styles.screen}>
      {showLeftColumn ? (
        <>
          <aside
            className={styles.left}
            style={{ width: `${leftColumnWidth}px` }}
            aria-label="Flight data"
          >
            <div className={styles.hud} style={{ height: `${hudHeight}px` }}>
              <Hud />
            </div>

            <Splitter
              orientation="horizontal"
              label="The height of the attitude indicator"
              value={hudHeight}
              min={MIN_HUD_HEIGHT}
              max={MAX_HUD_HEIGHT}
              onChange={setHudHeight}
            />

            <div className={styles.tabs}>
              <TabStrip tabs={TABS} active={activeTab} onSelect={setActiveTab} />

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
            label="The width of the flight data column"
            value={leftColumnWidth}
            min={MIN_LEFT_WIDTH}
            max={MAX_LEFT_WIDTH}
            onChange={setLeftColumnWidth}
          />
        </>
      ) : null}

      <main className={styles.main}>
        <div className={styles.split} ref={split}>
          {editorOpen ? (
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
                onChange={setGraphHeight}
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
                onChange={setGraphHeight}
              />
            </>
          ) : null}

          <div className={styles.mapArea}>
            <FlightMap />
          </div>
        </div>
      </main>
    </div>
  );
}
