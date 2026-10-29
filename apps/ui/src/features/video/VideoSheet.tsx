import { useCallback, useEffect, useRef, useState } from "react";

import {
  camerasDiscover,
  normaliseError,
  trackPoint,
  videoRelayStart,
  videoRelayStop,
  videoStreamStart,
  videoStreams,
  videoStreamStop,
} from "../../lib/ipc/bridge";
import type {
  CameraHost,
  CameraStream,
  VideoStream,
} from "../../lib/ipc/types";
import { DASH, formatCount, formatNumber } from "../../lib/format";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import {
  canDraw,
  canRelay,
  placeFromClick,
  type PicturePlace,
} from "../../lib/video/picture";
import styles from "./VideoSheet.module.css";

/**
 * The sheet of video streams the vehicle's camera describes.
 *
 * The camera says which streams it has, where they can be reached, and how big and how fast they
 * are. The ground station can ask the camera to start or stop one, and it can draw the picture of
 * a stream that is drawn as an image — what a picture element renders, which is an MJPEG stream or
 * a single picture over `http` or `https`. A stream that speaks another way is said as one that
 * cannot be drawn, because MPCraft does not carry a player for it.
 *
 * Drawing is something the operator decides: the application makes no network call that nobody
 * chose, so a stream is not fetched until the operator says to draw it. Upstream draws as soon as a
 * stream is picked (`GimbalVideoControl`), which is the difference recorded here.
 *
 * A click on the drawn picture says a place in the picture, and that place is sent to the camera
 * that described the stream. For an address typed by hand there is no camera behind it, so the
 * place is shown as what the click says and nothing is sent — a place cannot be asked of a device
 * that was never described.
 *
 * Upstream has the same protocols (`CameraProtocol.cs`, `GimbalVideoControl.cs`). What it also does
 * — finding cameras by service discovery on the local network, and drawing an RTSP stream through a
 * media runtime — is not ported here.
 */

/** What a string means in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

export function VideoSheet() {
  const open = useUiStore((state) => state.videoOpen);
  const hide = useUiStore((state) => state.hideVideo);

  const t: Say = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [streams, setStreams] = useState<readonly VideoStream[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // The address the picture is drawn from, and the stream that described it, when there is one.
  const [address, setAddress] = useState("");
  const [drawingKey, setDrawingKey] = useState<string | null>(null);
  const [drawing, setDrawing] = useState(false);
  const [relayUri, setRelayUri] = useState<string | null>(null);
  const [relayBusy, setRelayBusy] = useState(false);
  const [cameras, setCameras] = useState<readonly CameraHost[]>([]);
  const [cameraBusy, setCameraBusy] = useState(false);
  const [searched, setSearched] = useState(false);

  // The size that arrived is what the bytes say, and it is kept apart from the size the camera
  // said: for a stream the two are not the same thing.
  const [arrived, setArrived] = useState<{
    width: number;
    height: number;
  } | null>(null);
  // The size a camera on the network advertised for the address being drawn, when one did.
  const [advertised, setAdvertised] = useState<{
    width: number;
    height: number;
  } | null>(null);
  const [place, setPlace] = useState<PicturePlace | null>(null);
  const [report, setReport] = useState<string | null>(null);

  const areaRef = useRef<HTMLDivElement>(null);

  const read = useCallback(async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      setStreams(await videoStreams());
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (open && linked) {
      void read();
    }
  }, [linked, open, read]);

  useEffect(() => {
    if (open || relayUri === null) {
      return;
    }

    // The sheet is closed, and a process that nobody is watching has no reason to run.
    void videoRelayStop(relayUri);
    setRelayUri(null);
    setDrawing(false);
  }, [open, relayUri]);

  if (!open) {
    return null;
  }

  const drawnStream =
    streams.find((stream) => keyOf(stream) === drawingKey) ?? null;

  // What the address being drawn said about its size, from either half: a stream the vehicle's
  // camera described, or one a camera on the network advertised. A discovered camera has no
  // descriptor, so its text record is the only thing it said about itself.
  const sized =
    drawnStream !== null
      ? { width: drawnStream.width, height: drawnStream.height }
      : advertised;

  const toggle = async (stream: VideoStream) => {
    const name = stream.name || String(stream.streamId);

    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      if (stream.running) {
        await videoStreamStop(stream.componentId, stream.streamId);
        setNotice(t("The stream {name} has stopped.", { name }));
      } else {
        await videoStreamStart(stream.componentId, stream.streamId);
        setNotice(t("The stream {name} has started.", { name }));
      }

      await read();
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  /**
   * Starts the relay for one address and draws what it serves.
   *
   * Every way of drawing a picture that is not already MJPEG goes through here — a stream the camera
   * described, an address typed by hand, a camera found on the network — so the relay is stopped
   * before another one starts, and the sheet never shows two processes running for one picture.
   */
  const startRelay = async (
    uri: string,
    key: string | null,
    said: { width: number; height: number } | null = null,
  ) => {
    setRelayBusy(true);
    setError(null);
    setNotice(null);

    try {
      if (relayUri !== null) {
        await videoRelayStop(relayUri);
      }

      const relay = await videoRelayStart(uri);

      setRelayUri(uri);
      setAddress(relay.url);
      setDrawingKey(key);
      setAdvertised(said);
      setArrived(null);
      setPlace(null);
      setReport(null);
      setDrawing(true);
      setNotice(t("The stream is drawn through the relay."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setRelayBusy(false);
    }
  };

  const playFrom = (stream: VideoStream) =>
    startRelay(stream.uri, keyOf(stream));

  const playAddress = () => startRelay(address, null);

  const searchCameras = async () => {
    setCameraBusy(true);
    setError(null);
    setNotice(null);

    try {
      const found = await camerasDiscover();

      setCameras(found);
      setSearched(true);

      if (found.length === 0) {
        setNotice(t("No camera answered the search."));
      }
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setCameraBusy(false);
    }
  };

  const stopRelay = async () => {
    if (relayUri === null) {
      return;
    }

    setRelayBusy(true);

    try {
      await videoRelayStop(relayUri);
      setRelayUri(null);
      setDrawing(false);
      setArrived(null);
      setAdvertised(null);
      setNotice(t("The relay has stopped."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setRelayBusy(false);
    }
  };

  const drawFrom = (stream: VideoStream) => {
    if (canRelay(stream.uri)) {
      void playFrom(stream);
      return;
    }

    setAddress(stream.uri);
    setDrawingKey(keyOf(stream));
    setArrived(null);
    setPlace(null);
    setReport(null);
    setDrawing(true);
  };

  const clickPicture = async (event: React.MouseEvent<HTMLDivElement>) => {
    const area = areaRef.current;

    if (area === null || arrived === null) {
      return;
    }

    // The picture is fitted inside the content box, so the click is read against that box and not
    // against the border box the event is measured from: with a border the two are a pixel apart on
    // each side, which is enough to move a place by a hundredth.
    const style = getComputedStyle(area);
    const border = {
      x: Number.parseFloat(style.borderLeftWidth) || 0,
      y: Number.parseFloat(style.borderTopWidth) || 0,
    };
    const rect = area.getBoundingClientRect();
    const box = { width: area.clientWidth, height: area.clientHeight };

    const said = placeFromClick(box, arrived, {
      x: event.clientX - rect.left - border.x,
      y: event.clientY - rect.top - border.y,
    });

    if (said === null) {
      // The band is not part of the picture, so a click there does not say a place. Upstream
      // clamps it to the nearest edge and sends that; here it is refused before anything is sent.
      setReport(
        t(
          "The click fell outside the picture, so it says no place and nothing was sent.",
        ),
      );
      return;
    }

    setPlace(said);

    if (!linked || drawnStream === null) {
      setReport(
        drawnStream === null
          ? t(
              "The place is shown as the click says it, but there is no camera behind an address typed by hand, so nothing was sent.",
            )
          : t(
              "The place is shown as the click says it, and nothing was sent because there is no link.",
            ),
      );
      return;
    }

    try {
      await trackPoint(drawnStream.componentId, said.x, said.y);
      setReport(t("The camera was asked to follow that place."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    }
  };

  const drawable = canDraw(address);
  const relayable = canRelay(address);

  /** What a discovered stream is called: its codec and the size it advertised. */
  const streamLabel = (stream: CameraStream): string => {
    const size =
      stream.width !== null && stream.height !== null
        ? `${stream.width}×${stream.height}`
        : "";

    return (
      [stream.codec, size].filter((part) => part !== "").join(" ") ||
      t("The stream")
    );
  };

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The video streams of the vehicle")}
        onClick={(event) => event.stopPropagation()}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>
            {t("The video streams of the vehicle")}
          </h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The camera describes its streams with a URI, their resolution and frame rate. The stream itself travels outside MAVLink; MPCraft does not claim to decode it or draw it.",
            )}
          </p>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              onClick={() => void read()}
            >
              {t("Ask the camera")}
            </button>
            {!linked ? (
              <span className={styles.text}>{t("There is no link.")}</span>
            ) : null}
          </div>
        </section>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "A camera that announces itself on this network can be found without typing its address. The search is asked on the local network and does not leave it, so a camera on another network is not found.",
            )}
          </p>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={cameraBusy}
              onClick={() => {
                void searchCameras();
              }}
            >
              {t("Search the network")}
            </button>
            {searched && cameras.length === 0 ? (
              <span className={styles.text}>
                {t("No camera answered the search.")}
              </span>
            ) : null}
          </div>

          {cameras.map((camera) => (
            <div key={camera.id} className={styles.camera}>
              <p className={styles.text}>
                {camera.name} — {camera.address}:{camera.port}
              </p>

              <div className={styles.row}>
                {camera.streams.map((stream) => (
                  <button
                    key={stream.url}
                    type="button"
                    className="button"
                    disabled={relayBusy}
                    onClick={() => {
                      void startRelay(
                        stream.url,
                        `camera:${stream.url}`,
                        stream.width !== null && stream.height !== null
                          ? { width: stream.width, height: stream.height }
                          : null,
                      );
                    }}
                  >
                    {streamLabel(stream)}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </section>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "A picture element renders what it is given as an image — an MJPEG stream or a single picture over http or https. What speaks another way is said as one that cannot be drawn rather than tried, and the picture is fetched only when the operator says so, because the application makes no network call that nobody chose.",
            )}
          </p>

          <div className="field">
            <label htmlFor="video-address">
              {t("The address of the picture")}
            </label>
            <input
              id="video-address"
              type="text"
              value={address}
              onChange={(event) => {
                setAddress(event.target.value);
              }}
            />
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={!drawable}
              onClick={() => {
                setArrived(null);
                setPlace(null);
                setReport(null);
                setDrawing(true);
              }}
            >
              {t("Draw the picture")}
            </button>
            {relayable ? (
              <button
                type="button"
                className="button"
                disabled={relayBusy}
                onClick={() => {
                  void playAddress();
                }}
              >
                {t("Play through the relay")}
              </button>
            ) : null}
            {relayUri !== null ? (
              <button
                type="button"
                className="button"
                disabled={relayBusy}
                onClick={() => {
                  void stopRelay();
                }}
              >
                {t("Stop the relay")}
              </button>
            ) : null}
            {!drawable && !relayable ? (
              <span className={styles.text}>
                {t("This address is not one a picture element can render.")}
              </span>
            ) : null}
          </div>

          {drawing && drawable ? (
            <div
              className={styles.picture}
              role="button"
              tabIndex={0}
              aria-label={t("the picture")}
              ref={areaRef}
              onClick={(event) => {
                void clickPicture(event);
              }}
            >
              <img
                src={address}
                alt=""
                onLoad={(event) => {
                  const picture = event.currentTarget;
                  setArrived({
                    width: picture.naturalWidth,
                    height: picture.naturalHeight,
                  });
                }}
                onError={() => {
                  setReport(t("Nothing arrived from this address."));
                }}
              />
            </div>
          ) : null}

          <dl className={styles.facts}>
            <dt>{t("The size the camera said")}</dt>
            <dd>
              {sized === null
                ? t("nothing was said for this address")
                : `${formatCount(sized.width)} × ${formatCount(sized.height)}`}
            </dd>
            <dt>{t("The size that arrived")}</dt>
            <dd>
              {arrived === null
                ? DASH
                : `${formatCount(arrived.width)} × ${formatCount(arrived.height)}`}
            </dd>
            <dt>{t("The place the last click said")}</dt>
            <dd>
              {place === null
                ? DASH
                : `${formatNumber(place.x, 2)}, ${formatNumber(place.y, 2)}`}
            </dd>
            <dt>{t("What the last click did")}</dt>
            <dd>{report ?? DASH}</dd>
          </dl>

          {sized !== null && arrived !== null && differs(sized, arrived) ? (
            <p className={styles.text}>
              {t(
                "The size the camera said is not the size that arrived, and both are shown rather than one of them.",
              )}
            </p>
          ) : null}
        </section>

        <section className={styles.block}>
          {streams.length === 0 ? (
            <p className={styles.text}>
              {busy
                ? t("The camera is being asked.")
                : t("The camera did not describe any video streams.")}
            </p>
          ) : (
            <ul className={styles.list}>
              {streams.map((stream) => (
                <li className={styles.stream} key={keyOf(stream)}>
                  <div className={styles.streamHead}>
                    <h3 className={styles.streamName}>
                      {stream.name || t("Stream {id}", { id: stream.streamId })}
                    </h3>
                    <span className={styles.state}>
                      {stream.running ? t("Running") : t("Stopped")}
                    </span>
                  </div>

                  <dl className={styles.facts}>
                    <dt>{t("Address")}</dt>
                    <dd className={styles.uri}>
                      {stream.uri || t("Not provided")}
                    </dd>
                    <dt>{t("Picture")}</dt>
                    <dd>
                      {formatCount(stream.width)} × {formatCount(stream.height)}
                    </dd>
                    <dt>{t("Frame rate")}</dt>
                    <dd>{formatNumber(stream.framesPerSecond, 1)} fps</dd>
                    <dt>{t("Bit rate")}</dt>
                    <dd>{formatCount(stream.bitrate)} bit/s</dd>
                  </dl>

                  <div className={styles.row}>
                    <button
                      type="button"
                      className="button"
                      disabled={busy}
                      onClick={() => void toggle(stream)}
                    >
                      {stream.running
                        ? t("Stop the stream")
                        : t("Start the stream")}
                    </button>

                    {canDraw(stream.uri) || canRelay(stream.uri) ? (
                      <button
                        type="button"
                        className="button"
                        onClick={() => {
                          drawFrom(stream);
                        }}
                      >
                        {t("Draw this picture")}
                      </button>
                    ) : null}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>

        {notice !== null ? (
          <p className={styles.notice} role="status">
            {notice}
          </p>
        ) : null}

        {error !== null ? (
          <p className={styles.error} role="alert">
            {error}
          </p>
        ) : null}
      </div>
    </div>
  );
}

/** What a stream is kept under, which is the pair the camera and its id make. */
function keyOf(stream: VideoStream): string {
  return `${stream.componentId}-${stream.streamId}`;
}

/** Whether the size that arrived is a different size than the one the camera said. */
function differs(
  said: { width: number; height: number },
  arrived: { width: number; height: number },
): boolean {
  return said.width !== arrived.width || said.height !== arrived.height;
}
