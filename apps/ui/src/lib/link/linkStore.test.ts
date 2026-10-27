import { beforeEach, describe, expect, it } from "vitest";

import type {
  BoardOutput,
  PlaybackState,
  StateEvent,
  VehicleEvent,
} from "../ipc/types";
import { liveTelemetry } from "./liveTelemetry";
import {
  applyPlayback,
  applyState,
  chosenVehicle,
  initialState,
  sameOutput,
  useLinkStore,
} from "./linkStore";

/**
 * The vehicle the interface shows as the one it is reading.
 *
 * The choice is made by the link — the boards that answered are what it knows — so the store only
 * reads the mark the event carries. What is tested here is that the rule reads the mark rather
 * than the order of the list, because the two are not the same thing: a link that heard board 2
 * first lists it first, and an operator can then choose board 1.
 */

function vehicle(
  systemId: number,
  componentId: number,
  name: string,
  chosen: boolean,
): VehicleEvent {
  return { systemId, componentId, kind: null, name, chosen };
}

describe("the vehicle the station speaks to", () => {
  it("says there is none before the link has selected one", () => {
    // A link that has heard boards but has not satisfied the selection rule has no vehicle it
    // speaks to; saying "the first one" would name a board the telemetry is not being read from.
    const vehicles = [
      vehicle(1, 1, "1/1", false),
      vehicle(2, 62, "2/62", false),
    ];

    expect(chosenVehicle(vehicles)).toBeNull();
    expect(chosenVehicle([])).toBeNull();
  });

  it("reads the mark rather than the order of the list", () => {
    // The hand: board 2 was heard first, so it is listed first, and the operator chose it. The
    // rule has to answer with board 2 even though board 1 is at the front of the list.
    const vehicles = [
      vehicle(1, 1, "ArduCopter", false),
      vehicle(2, 1, "ArduPlane", true),
    ];

    expect(chosenVehicle(vehicles)?.systemId).toBe(2);

    // And when the choice moves, the rule follows it rather than staying where the list starts.
    const moved = [
      vehicle(1, 1, "ArduCopter", true),
      vehicle(2, 1, "ArduPlane", false),
    ];

    expect(chosenVehicle(moved)?.name).toBe("ArduCopter");
  });
});

describe("the words the board printed", () => {
  it("keep their reference when nothing changed", () => {
    // The state arrives several times a second. The console draws the capture, so a comparison
    // that could not tell an unchanged capture from a new one would redraw the block on every
    // tick, which is what the store exists to avoid.
    const capture: BoardOutput = {
      lines: ["ArduCopter V4.3.7 (0123abcd)"],
      dropped: 0,
      looksLikeCanBus: false,
    };

    expect(sameOutput(capture, capture)).toBe(true);
    expect(sameOutput(capture, { ...capture })).toBe(true);
  });

  it("are not the same when what was dropped changed", () => {
    // The number of lines that are not in the window is drawn beside the lines, so a comparison
    // that stopped at the lines would keep showing an old count — and the operator would read a
    // capture as the whole output when it is not.
    const before: BoardOutput = {
      lines: ["line 52", "line 51"],
      dropped: 0,
      looksLikeCanBus: false,
    };

    const after: BoardOutput = {
      lines: ["line 52", "line 51"],
      dropped: 148,
      looksLikeCanBus: false,
    };

    expect(sameOutput(before, after)).toBe(false);

    // The decision about what the text looks like travels with the lines for the same reason: it
    // is the link's, and the console shows what the link says.
    const decided: BoardOutput = {
      lines: before.lines,
      dropped: before.dropped,
      looksLikeCanBus: true,
    };

    expect(sameOutput(before, decided)).toBe(false);
  });
});

describe("the documents the boundary speaks", () => {
  /**
   * The payload as the backend writes it, read from the bytes rather than from an object written in the
   * contract's own names. The point of reading it this way is that a reader holding its own shape — which
   * is what the store had — updates nothing when the two disagree, and nothing on either side could say
   * which of them was wrong: the pair arrived as `system_id` while the interface read `systemId`.
   */
  function tick(document: string): StateEvent {
    return JSON.parse(document) as StateEvent;
  }

  /** A tick of a link that has heard one board, written the way the boundary writes it. */
  const LIVE_TICK =
    '{"status":"connected","vehicles":[],"telemetry":{"mode":"RTL"},"loss":{"received":1,"lost":0},' +
    '"messages":[],"board":null,"output":{"lines":[],"dropped":0,"looksLikeCanBus":false},' +
    '"adsb":[],"gimbals":[],"cameras":[]}';

  beforeEach(() => {
    useLinkStore.setState(initialState);
    liveTelemetry.reset();
  });

  it("updates the store from the spellings the struct writes", () => {
    applyState(
      tick(
        '{"status":"connected",' +
          '"vehicles":[{"systemId":2,"componentId":1,"kind":"ArduPlane","name":"ArduPlane","chosen":true}],' +
          '"telemetry":{"mode":"AUTO","armed":true},"loss":{"received":121,"lost":0},"messages":[],' +
          '"board":"Detected Pixhawk","output":{"lines":["BOOT"],"dropped":0,"looksLikeCanBus":false},' +
          '"adsb":[],"gimbals":[],"cameras":[]}',
      ),
    );

    const state = useLinkStore.getState();

    expect(state.status).toBe("connected");
    // The pair is what the store keys a vehicle by, so a spelling that does not arrive is a list that
    // looks empty while a board is answering.
    expect(chosenVehicle(state.vehicles)?.systemId).toBe(2);
    expect(chosenVehicle(state.vehicles)?.name).toBe("ArduPlane");
    expect(state.board).toBe("Detected Pixhawk");
    expect(state.output.lines).toEqual(["BOOT"]);
    expect(state.telemetry.mode).toBe("AUTO");
  });

  it("keeps the flight that is being played back rather than taking the window back", () => {
    applyPlayback(
      JSON.parse(
        '{"open":true,"position":12.5,"duration":60,"telemetry":{"mode":"AUTO"}}',
      ) as PlaybackState,
    );

    // The drawing reads the published snapshot, so the play-back is what it shows.
    expect(liveTelemetry.value.mode).toBe("AUTO");

    // A tick of the link arrives while the play-back is open: the window stays with the file, because
    // the operator is looking at a flight that has already happened.
    applyState(tick(LIVE_TICK));

    const state = useLinkStore.getState();

    expect(state.playback).toBe(true);
    expect(state.playbackPosition).toBe(12.5);
    expect(liveTelemetry.value.mode).toBe("AUTO");

    // And closing the log gives the window back to the wire, so the next tick is drawn again.
    applyPlayback(
      JSON.parse(
        '{"open":false,"position":0,"duration":60,"telemetry":{}}',
      ) as PlaybackState,
    );

    expect(useLinkStore.getState().playback).toBe(false);

    applyState(tick(LIVE_TICK));

    expect(liveTelemetry.value.mode).toBe("RTL");
  });
});
