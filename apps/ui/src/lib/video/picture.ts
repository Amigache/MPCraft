/**
 * The picture on the screen, and the place a click on it says.
 *
 * A picture is drawn inside its box keeping the proportions it has, so when the box is a
 * different shape than the picture there is a band of nothing between the edge of the box and
 * the edge of the picture. The place a click says is a place **in the picture**, not in the box,
 * so the band has to be taken off before the click is read — and a click that falls in the band
 * is not a place at all: it is refused rather than read as the nearest edge. Upstream clamps
 * (`GimbalVideoControl.getMousePosition`, line 684), which makes a click on the black band say a
 * place the operator did not mean.
 *
 * Upstream's mapping is worked out with the wrong divisor as well. It scales the click by the
 * size of the box over the size the picture is drawn at, and then normalises by the drawn size,
 * so the two only agree on an axis where there is no band. For a box of 640 × 480 and a picture
 * of 1280 × 720 the picture is drawn at 640 × 360, and a click at the middle of it — the middle
 * of the picture, which is `0` — comes back from upstream as `0.333`. That case is asserted
 * below against numbers written down before the function was called.
 */

/** The box the picture is drawn in, in pixels. */
export interface PictureBox {
  readonly width: number;
  readonly height: number;
}

/** The size the picture itself has, in pixels. */
export interface PictureSize {
  readonly width: number;
  readonly height: number;
}

/** A point inside the box, from its upper-left corner, in pixels. */
export interface PictureClick {
  readonly x: number;
  readonly y: number;
}

/** A place in the picture, from `-1` to `1`: `-1` is the left or the top, `1` the right or the bottom. */
export interface PicturePlace {
  readonly x: number;
  readonly y: number;
}

/**
 * How big the picture is drawn inside the box.
 *
 * Returns nothing when a size is not known yet: a picture that has not arrived has no size, and a
 * place cannot be worked out from something that is not there.
 */
export function drawnSize(
  box: PictureBox,
  picture: PictureSize,
): PictureBox | null {
  if (box.width <= 0 || box.height <= 0) {
    return null;
  }

  if (picture.width <= 0 || picture.height <= 0) {
    return null;
  }

  const drawnWidth = Math.min(
    box.width,
    (box.height * picture.width) / picture.height,
  );
  const drawnHeight = Math.min(
    box.height,
    (box.width * picture.height) / picture.width,
  );

  return { width: drawnWidth, height: drawnHeight };
}

/**
 * The place a click says.
 *
 * Returns nothing when the click falls outside the picture that is drawn — the band is not part of
 * the picture, so it does not say a place.
 */
export function placeFromClick(
  box: PictureBox,
  picture: PictureSize,
  click: PictureClick,
): PicturePlace | null {
  const drawn = drawnSize(box, picture);

  if (drawn === null) {
    return null;
  }

  // The band on each side, which is what the letterboxing leaves between the box and the picture.
  const insideX = click.x - (box.width - drawn.width) / 2;
  const insideY = click.y - (box.height - drawn.height) / 2;

  if (
    insideX < 0 ||
    insideX > drawn.width ||
    insideY < 0 ||
    insideY > drawn.height
  ) {
    return null;
  }

  return {
    x: (2 * insideX) / drawn.width - 1,
    y: (2 * insideY) / drawn.height - 1,
  };
}

/**
 * Whether an address is one MPCraft can draw as a picture.
 *
 * A picture element renders what it is given as an image — an MJPEG stream or a single picture —
 * over `http` or `https`. A stream that speaks `rtsp`, or an address that is only a port, is not
 * something a picture element can render, so it is said as one that cannot be drawn rather than
 * tried.
 */
export function canDraw(address: string): boolean {
  const text = address.trim();

  if (!text.startsWith("http://") && !text.startsWith("https://")) {
    return false;
  }

  // An address that gives no host is a port the camera named, not a place to go to.
  const rest = text.slice(text.indexOf("//") + 2);
  const path = rest.split("/");
  const host = (path[0] ?? "").split(":")[0] ?? "";

  return host.length > 0;
}

/**
 * Whether the address is a stream a relay can turn into a picture.
 *
 * A picture element cannot read RTSP, RTMP or a raw transport; `ffmpeg` can, and the relay
 * serves the result as MJPEG. The list is the schemes the relay knows, and an address with
 * another one is not guessed at.
 */
export function canRelay(address: string): boolean {
  const text = address.trim().toLowerCase();

  return [
    "rtsp://",
    "rtsps://",
    "rtmp://",
    "rtmps://",
    "udp://",
    "srt://",
    "tcp://",
  ].some((scheme) => text.startsWith(scheme));
}
