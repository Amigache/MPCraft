import { SCREENS, useUiStore } from "../../lib/ui/uiStore";
import { useLocaleStore } from "../../lib/i18n/localeStore";
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
  const t = useLocaleStore((state) => state.t);

  // The names are said through the bundles, so that the switcher says the same thing as the rest
  // of the window in the language the operator reads.
  return (
    <TabStrip
      tabs={SCREENS.map((entry) => ({ id: entry.id, label: t(entry.label) }))}
      active={screen}
      onSelect={setScreen}
    />
  );
}
