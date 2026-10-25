import {
  arm,
  call,
  disarm,
  homePosition,
  jumpToItem,
  normaliseError,
  reboot,
  runMavCommand,
  setCurrentItem,
  setHomePosition,
  setMode,
  setPositionTarget,
  takeoff,
} from "../ipc/bridge";
import type { ConnectResponse, MavCommand } from "../ipc/types";
import { formatAltitude, formatCoordinate, formatCount } from "../format";
import { useLocaleStore } from "../i18n/localeStore";
import { useActivityStore } from "../ui/activityStore";

/**
 * The flight actions, in one place.
 *
 * The menu bar and the actions tab do the same things, so they do them here: the command
 * is sent, and the outcome is reported through the activity store, which the status bar
 * and the panels read.
 *
 * What is reported is a word, and the words the interface says are looked up in the catalogue —
 * a sentence written straight into this file would be said in the language the code is written
 * in, whatever the window was chosen to be read in. So each one is asked for here, with the parts
 * of the sentence named rather than joined by hand, and the numbers that go into them are made
 * into strings by the formatting module, as they are everywhere else.
 */

/** Arms the vehicle. */
export async function armVehicle(): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await arm();
    report(null, t("the vehicle was armed"));
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Disarms the vehicle. */
export async function disarmVehicle(): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await disarm();
    report(null, t("the vehicle was disarmed"));
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Changes the flight mode. */
export async function changeMode(value: number, name: string): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await setMode(value);
    report(null, t("the mode is now {mode}", { mode: name }));
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Takes off to a height, in metres above the home position. */
export async function takeOff(altitude: number): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await takeoff(altitude);
    report(
      null,
      t("the vehicle was told to take off to {height}", {
        height: formatAltitude(altitude),
      }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Says which item of the plan the vehicle is executing. */
export async function makeCurrent(seq: number): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await setCurrentItem(seq);
    report(
      null,
      t("the vehicle is now on item {item}", { item: formatCount(seq + 1) }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Makes the vehicle jump to an item of the plan. */
export async function jumpTo(seq: number): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await jumpToItem(seq);
    report(
      null,
      t("the vehicle jumped to item {item}", { item: formatCount(seq + 1) }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/**
 * Tells the vehicle to go to a place.
 *
 * The vehicle has to be in the guided mode to obey, and the height is the one it is
 * told to keep.
 */
export async function flyTo(
  latitude: number,
  longitude: number,
  altitude: number,
): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await setPositionTarget(latitude, longitude, altitude);
    report(
      null,
      t("the vehicle was told to go to {place}", {
        place: formatCoordinate(latitude, longitude),
      }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/**
 * Moves the home of the vehicle to a place.
 *
 * The vehicle is told where the home is, and it confirms the move by reporting the home
 * it now holds.
 */
export async function moveHome(
  latitude: number,
  longitude: number,
  altitude: number,
): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await setHomePosition({ lat: latitude, lng: longitude, alt: altitude });
    report(
      null,
      t("the home was moved to {place}", {
        place: formatCoordinate(latitude, longitude),
      }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Asks the vehicle where its home is. */
export async function askHome(): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    const home = await homePosition();
    report(
      null,
      t("the home is at {place}", {
        place: formatCoordinate(home.lat, home.lng),
      }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Asks the vehicle to restart. */
export async function rebootVehicle(): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await reboot();
    report(null, t("the vehicle was asked to restart"));
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/**
 * Sends one command of the catalogue to the vehicle.
 *
 * The values are the seven slots as the operator wrote them, and the name of what is reported is
 * the answer the vehicle gave rather than the command that was sent: an executor has to say what the
 * vehicle said, not only that it worked. A value the catalogue does not describe is refused by the
 * backend before anything is sent, so it never reaches the wire.
 */
export async function sendCatalogueCommand(
  command: MavCommand,
  values: readonly number[],
): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    const answer = await runMavCommand(command.value, values);
    report(
      null,
      t("the vehicle answered {result} to {command}", {
        result: answer.result,
        command: answer.command,
      }),
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Closes the link to the vehicle. */
export async function closeLink(): Promise<void> {
  const report = useActivityStore.getState().report;
  const t = useLocaleStore.getState().t;
  try {
    await call<ConnectResponse>("disconnect");
    report(null, t("the link was closed"));
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}
