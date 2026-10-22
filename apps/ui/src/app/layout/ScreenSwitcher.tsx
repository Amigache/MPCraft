import { SCREENS, useUiStore } from "../../lib/ui/uiStore";
import { TabStrip } from "../../components/TabStrip";

/**
 * The switcher of the screens.
 *
 * This is what Mission Planner's main switcher is: the flight data, the planner, the
 * configuration and the logs. It behaves as the tab strip of the left column behaves —
 * a tab list where the arrow keys move between the screens, the screen that is open is
 * the only one in the tab order, and the body of the window is the panel labelled by the
 * screen that opened it.
 *
 * The short tasks are not here: a calibration or a file transfer is a sheet that lies
 * over whichever screen is open, because the operator does them and comes back to where
 * they were.
 */
export function ScreenSwitcher() {
  const screen = useUiStore((state) => state.screen);
  const setScreen = useUiStore((state) => state.setScreen);

  return <TabStrip tabs={SCREENS} active={screen} onSelect={setScreen} />;
}
