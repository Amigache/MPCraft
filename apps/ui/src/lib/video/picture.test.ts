import { describe, expect, it } from "vitest";

import { canDraw, canRelay, drawnSize, placeFromClick } from "./picture";

/** The box the picture is drawn in, for the test: a box of a different shape than the picture. */
const BOX = { width: 640, height: 480 };

/** The picture, whose proportions are 16:9, so the box cannot hold it without a band. */
const PICTURE = { width: 1280, height: 720 };

describe("the picture on the screen", () => {
  it("draws the picture keeping its proportions, which leaves a band", () => {
    const drawn = drawnSize(BOX, PICTURE);

    // 640 wide would need 360 high at these proportions, and that is less than the box, so the
    // band is above and below: 60 pixels of nothing on each side.
    expect(drawn).toEqual({ width: 640, height: 360 });
  });

  it("reads a click as a place in the picture, not in the box", () => {
    // The middle of the drawn picture is at y = 60 + 180 = 240, which is also the middle of the
    // box. A click there says the middle of the picture, which is `0`.
    expect(placeFromClick(BOX, PICTURE, { x: 320, y: 240 })).toEqual({
      x: 0,
      y: 0,
    });
  });

  it("does not agree with upstream's divisor on an axis that has a band", () => {
    // Upstream scales the click by the box over the drawn size (`VideoBox.Height / imageHeight`,
    // line 689) and then normalises by the drawn size, which for these numbers gives 0.333 rather
    // than 0 for the same click. The two only agree where there is no band on that axis.
    const upstream = (2 * ((240 - 60) * (480 / 360))) / 360 - 1;

    expect(upstream).toBeCloseTo(0.333, 3);
    expect(placeFromClick(BOX, PICTURE, { x: 320, y: 240 })).toEqual({
      x: 0,
      y: 0,
    });
  });

  it("refuses a click that falls in the band instead of reading it as the edge", () => {
    // 30 pixels from the top of the box is inside the band, not inside the picture. Upstream
    // clamps this to the edge and sends the top of the picture.
    expect(placeFromClick(BOX, PICTURE, { x: 320, y: 30 })).toBeNull();
  });

  it("reads a click on the edge of the picture as that edge", () => {
    expect(placeFromClick(BOX, PICTURE, { x: 320, y: 60 })).toEqual({
      x: 0,
      y: -1,
    });
    expect(placeFromClick(BOX, PICTURE, { x: 320, y: 420 })).toEqual({
      x: 0,
      y: 1,
    });
    expect(placeFromClick(BOX, PICTURE, { x: 0, y: 240 })).toEqual({
      x: -1,
      y: 0,
    });
  });

  it("takes the band off the other way when the picture is taller than the box", () => {
    const tallBox = { width: 400, height: 400 };
    const tallPicture = { width: 720, height: 1280 };

    // The picture is drawn 225 wide, so the band is 87.5 pixels on each side.
    const drawn = drawnSize(tallBox, tallPicture);

    expect(drawn).toEqual({ width: 225, height: 400 });

    expect(placeFromClick(tallBox, tallPicture, { x: 200, y: 200 })).toEqual({
      x: 0,
      y: 0,
    });
    expect(placeFromClick(tallBox, tallPicture, { x: 40, y: 200 })).toBeNull();
  });

  it("says nothing about a place when the picture has not arrived", () => {
    // A picture element has no size before its first frame; the size the camera *said* is kept
    // apart from the size that arrived, so one is not read as the other.
    expect(drawnSize(BOX, { width: 0, height: 0 })).toBeNull();
    expect(
      placeFromClick(BOX, { width: 0, height: 0 }, { x: 320, y: 240 }),
    ).toBeNull();
  });
});

describe("what can be drawn as a picture", () => {
  it("draws an address over http or https", () => {
    expect(canDraw("http://127.0.0.1:8080/stream")).toBe(true);
    expect(canDraw("https://camera.example/picture.jpg")).toBe(true);
  });

  it("refuses what a picture element cannot render", () => {
    expect(canDraw("rtsp://127.0.0.1:8550/stream")).toBe(false);
    expect(canDraw("ws://127.0.0.1:8554")).toBe(false);
  });

  it("refuses an address that names no host", () => {
    // The camera protocol often gives a port rather than an address; a port is not a place to go
    // to, so it is said as one that cannot be drawn rather than guessed at.
    expect(canDraw("14550")).toBe(false);
    expect(canDraw("http://:8080/stream")).toBe(false);
  });
});

describe("what the relay can turn into a picture", () => {
  it("reads the schemes a camera publishes", () => {
    for (const address of [
      "rtsp://camera/live",
      "rtsps://camera/live",
      "rtmp://camera/live",
      "udp://127.0.0.1:5600",
      "srt://127.0.0.1:9000",
    ]) {
      expect(canRelay(address)).toBe(true);
    }
  });

  it("does not claim a picture element's own address", () => {
    expect(canRelay("http://camera/stream.mjpeg")).toBe(false);
    expect(canRelay("ws://camera/live")).toBe(false);
    expect(canRelay("camera")).toBe(false);
  });
});
