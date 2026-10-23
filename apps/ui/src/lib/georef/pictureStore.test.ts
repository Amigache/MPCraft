import { beforeEach, describe, expect, it } from "vitest";

import { usePictureStore } from "./pictureStore";

/**
 * The store of the pictures holds what the sheet reads and what the map draws, so the two see
 * the same pictures. What is tested here is that it keeps them as they were given, and that the
 * things the operator names — a folder, a log, a difference between two clocks — are kept with
 * them.
 */

beforeEach(() => {
  usePictureStore.setState({
    folder: "",
    log: "",
    offset: 0,
    lag: 0,
    pictures: [],
  });
});

describe("the pictures and where they were taken", () => {
  it("keeps the folder and the log the operator has named", () => {
    usePictureStore.getState().setFolder("C:/flight/photos");
    usePictureStore.getState().setLog("C:/flight/mission.tlog");

    const state = usePictureStore.getState();

    expect(state.folder).toBe("C:/flight/photos");
    expect(state.log).toBe("C:/flight/mission.tlog");
  });

  it("keeps the pictures that were read", () => {
    usePictureStore.getState().setPictures([
      {
        name: "P001.jpg",
        width: 640,
        height: 480,
        orientation: "turned a quarter to the right",
        takenAt: 1_792_753_449,
        place: { lat: -33.8707, lng: 151.2093, alt: 92 },
        matchedAt: null,
        awaySeconds: null,
        matchedPlace: null,
      },
    ]);

    const picture = usePictureStore.getState().pictures[0];

    expect(picture?.name).toBe("P001.jpg");
    expect(picture?.matchedPlace).toBeNull();
  });

  it("keeps the clock difference that was worked out, and what the shutter took", () => {
    usePictureStore.getState().setOffset(67.8125);
    usePictureStore.getState().setLag(1.5);

    const state = usePictureStore.getState();

    expect(state.offset).toBeCloseTo(67.8125);
    expect(state.lag).toBe(1.5);
  });
});
