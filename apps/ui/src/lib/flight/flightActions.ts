import {
  arm,
  call,
  disarm,
  jumpToItem,
  normaliseError,
  reboot,
  setCurrentItem,
  setMode,
  setPositionTarget,
  takeoff,
} from "../ipc/bridge";
import type { ConnectResponse } from "../ipc/types";
import { useActivityStore } from "../ui/activityStore";

/**
 * The flight actions, in one place.
 *
 * The menu bar and the actions tab do the same things, so they do them here: the command
 * is sent, and the outcome is reported through the activity store, which the status bar
 * and the panels read.
 */

/** Arms the vehicle. */
export async function armVehicle(): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await arm();
    report(null, "the vehicle was armed");
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Disarms the vehicle. */
export async function disarmVehicle(): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await disarm();
    report(null, "the vehicle was disarmed");
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Changes the flight mode. */
export async function changeMode(value: number, name: string): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await setMode(value);
    report(null, `the mode is now ${name}`);
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Takes off to a height, in metres above the home position. */
export async function takeOff(altitude: number): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await takeoff(altitude);
    report(null, `the vehicle was told to take off to ${altitude} m`);
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Says which item of the plan the vehicle is executing. */
export async function makeCurrent(seq: number): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await setCurrentItem(seq);
    report(null, `the vehicle is now on item ${seq + 1}`);
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Makes the vehicle jump to an item of the plan. */
export async function jumpTo(seq: number): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await jumpToItem(seq);
    report(null, `the vehicle jumped to item ${seq + 1}`);
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
  try {
    await setPositionTarget(latitude, longitude, altitude);
    report(
      null,
      `the vehicle was told to go to ${latitude.toFixed(5)}, ${longitude.toFixed(5)}`,
    );
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Asks the vehicle to restart. */
export async function rebootVehicle(): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await reboot();
    report(null, "the vehicle was asked to restart");
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}

/** Closes the link to the vehicle. */
export async function closeLink(): Promise<void> {
  const report = useActivityStore.getState().report;
  try {
    await call<ConnectResponse>("disconnect");
    report(null, "the link was closed");
  } catch (cause) {
    report(normaliseError(cause), null);
  }
}
