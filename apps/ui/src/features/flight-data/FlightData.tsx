import { Splitter } from "../../app/layout/Splitter";
import { TabStrip } from "../../components/TabStrip";
import { ActionsPanel } from "../../features/actions/ActionsPanel";
import { FlightMap } from "../../features/map/FlightMap";
import { GaugesPanel } from "../../features/gauges/GaugesPanel";
import { Hud } from "../../features/hud/Hud";
import { MessageConsole } from "../../features/messages/MessageConsole";
import { QuickViewPanel } from "../../features/quick-view/QuickViewPanel";
import { StatusPanel } from "../../features/status/StatusPanel";
import {
  MAX_HUD_HEIGHT,
  MAX_LEFT_WIDTH,
  MIN_HUD_HEIGHT,
  MIN_LEFT_WIDTH,
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
 */
export function FlightData() {
  const showLeftColumn = useUiStore((state) => state.showLeftColumn);
  const leftColumnWidth = useUiStore((state) => state.leftColumnWidth);
  const hudHeight = useUiStore((state) => state.hudHeight);
  const activeTab = useUiStore((state) => state.activeTab);
  const setActiveTab = useUiStore((state) => state.setActiveTab);
  const setLeftColumnWidth = useUiStore((state) => state.setLeftColumnWidth);
  const setHudHeight = useUiStore((state) => state.setHudHeight);

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
        <FlightMap />
      </main>
    </div>
  );
}
