import { useCallback, useEffect, useState } from "react";

import {
  normaliseError,
  videoStreamStart,
  videoStreams,
  videoStreamStop,
} from "../../lib/ipc/bridge";
import type { VideoStream } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./VideoSheet.module.css";

/**
 * The sheet of video streams the vehicle's camera describes.
 *
 * The camera says which streams it has, where they can be reached, and how big and how
 * fast they are. The ground station can ask the camera to start or stop one. The stream
 * itself travels outside MAVLink; MPCraft does not claim to decode it or draw it.
 *
 * Upstream has the same protocol in `CameraProtocol` and `Camera.cs`.
 */
export function VideoSheet() {
  const open = useUiStore((state) => state.videoOpen);
  const hide = useUiStore((state) => state.hideVideo);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [streams, setStreams] = useState<readonly VideoStream[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

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

  if (!open) {
    return null;
  }

  const toggle = async (stream: VideoStream) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      if (stream.running) {
        await videoStreamStop(stream.componentId, stream.streamId);
        setNotice(`The stream ${stream.name || stream.streamId} has stopped.`);
      } else {
        await videoStreamStart(stream.componentId, stream.streamId);
        setNotice(`The stream ${stream.name || stream.streamId} has started.`);
      }

      await read();
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label="The video streams of the vehicle"
        onClick={(event) => event.stopPropagation()}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>The video streams of the vehicle</h2>
          <button type="button" className="button" onClick={hide}>
            Close
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            The camera describes its streams with a URI, their resolution and frame
            rate. The stream travels outside MAVLink: use the URI with a video player.
          </p>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              onClick={() => void read()}
            >
              Ask the camera
            </button>
            {!linked ? <span className={styles.text}>There is no link.</span> : null}
          </div>
        </section>

        <section className={styles.block}>
          {streams.length === 0 ? (
            <p className={styles.text}>
              {busy
                ? "The camera is being asked."
                : "The camera did not describe any video streams."}
            </p>
          ) : (
            <ul className={styles.list}>
              {streams.map((stream) => (
                <li
                  className={styles.stream}
                  key={`${stream.componentId}-${stream.streamId}`}
                >
                  <div className={styles.streamHead}>
                    <h3 className={styles.streamName}>
                      {stream.name || `Stream ${stream.streamId}`}
                    </h3>
                    <span className={styles.state}>
                      {stream.running ? "Running" : "Stopped"}
                    </span>
                  </div>

                  <dl className={styles.facts}>
                    <dt>Address</dt>
                    <dd className={styles.uri}>{stream.uri || "Not provided"}</dd>
                    <dt>Picture</dt>
                    <dd>
                      {stream.width} × {stream.height}
                    </dd>
                    <dt>Frame rate</dt>
                    <dd>{stream.framesPerSecond.toFixed(1)} fps</dd>
                    <dt>Bit rate</dt>
                    <dd>{stream.bitrate} bit/s</dd>
                  </dl>

                  <button
                    type="button"
                    className="button"
                    disabled={busy}
                    onClick={() => void toggle(stream)}
                  >
                    {stream.running ? "Stop the stream" : "Start the stream"}
                  </button>
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
