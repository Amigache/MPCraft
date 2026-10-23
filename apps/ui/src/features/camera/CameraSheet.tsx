import { useCallback, useEffect, useState } from "react";

import {
  cameraZoomOptions,
  capturePhoto,
  clearGimbalRoi,
  gimbalDevices,
  neutralGimbal,
  normaliseError,
  pointGimbalAtLocation,
  pointGimbalAtVehicle,
  readCamera,
  retractGimbal,
  setCameraZoom,
  setGimbalAngles,
  setGimbalYawLock,
  slewGimbal,
  startIntervalCapture,
  startRecording,
  stopIntervalCapture,
  stopRecording,
  trackPoint,
  trackRectangle,
} from "../../lib/ipc/bridge";
import type {
  CameraDevice,
  GimbalDevice,
  ZoomOption,
} from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./CameraSheet.module.css";

/**
 * The sheet of the camera and the gimbal.
 *
 * What a device says about itself decides what can be asked of it: a camera that cannot
 * record video, or a gimbal without a pitch axis, is refused before anything is sent. The
 * controls here are therefore only offered for what the vehicle has said it can do, and an
 * angle outside the range of the device is not offered either.
 *
 * The attitude of the gimbal and the state of the camera come from the link as the vehicle
 * reports them, so the sheet shows what the vehicle says rather than what was last asked for.
 *
 * Upstream has the same protocols (`CameraProtocol.cs`, `GimbalManagerProtocol.cs`). What it
 * does with a picture on the screen — picking a place to follow from the picture itself —
 * waits on drawing the video, which MPCraft does not do yet; the place is typed instead.
 */

/** What a string means in the language the window is read in. */
type Say = (source: string, values?: Record<string, string | number>) => string;

/** How many seconds between pictures when the operator has not chosen. */
const DEFAULT_INTERVAL = 5;

/** A place in the picture, from `-1` to `1`, at the middle. */
const DEFAULT_PLACE = "0";

/** The share of the whole zoom range, when the operator has not chosen. */
const DEFAULT_ZOOM_LEVEL = "50";

export function CameraSheet() {
  const open = useUiStore((state) => state.cameraOpen);
  const hide = useUiStore((state) => state.hideCamera);

  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const cameras = useLinkStore((state) => state.cameras);
  const gimbals = useLinkStore((state) => state.gimbals);

  const [zoomOptions, setZoomOptions] = useState<ZoomOption[]>([]);
  const [deviceId, setDeviceId] = useState<number>(0);

  const [interval, setIntervalSeconds] = useState(String(DEFAULT_INTERVAL));
  const [zoomLevel, setZoomLevel] = useState(DEFAULT_ZOOM_LEVEL);
  const [zoomType, setZoomType] = useState<number>(2);

  const [pointX, setPointX] = useState(DEFAULT_PLACE);
  const [pointY, setPointY] = useState(DEFAULT_PLACE);
  const [corner1X, setCorner1X] = useState("-0.5");
  const [corner1Y, setCorner1Y] = useState("-0.5");
  const [corner2X, setCorner2X] = useState("0.5");
  const [corner2Y, setCorner2Y] = useState("0.5");

  const [pitch, setPitch] = useState("0");
  const [yaw, setYaw] = useState("0");
  const [pitchRate, setPitchRate] = useState("0");
  const [yawRate, setYawRate] = useState("0");

  const [roiLatitude, setRoiLatitude] = useState("");
  const [roiLongitude, setRoiLongitude] = useState("");
  const [roiAltitude, setRoiAltitude] = useState("100");
  const [roiSystemId, setRoiSystemId] = useState("1");

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const camera: CameraDevice | null = cameras[0] ?? null;
  // A camera answers on its own component, so a command for it names that component rather
  // than the autopilot; the index says which camera within it.
  const cameraComponent = camera?.componentId ?? 0;
  const gimbal: GimbalDevice | null =
    gimbals.find((reported) => reported.deviceId === deviceId) ??
    gimbals[0] ??
    null;

  const read = useCallback(async () => {
    if (!linked) {
      return;
    }

    // The kinds of zoom are what the program that knows the protocol says they are, so the one
    // shown is taken from the list rather than assumed. This does not depend on the vehicle, so
    // it is done even when a vehicle that cannot answer is on the link.
    try {
      const options = await cameraZoomOptions();
      setZoomOptions(options);

      const chosen =
        options.find((option) => option.value === zoomType) ?? options[0];
      if (chosen !== undefined) {
        setZoomType(chosen.value);
      }
    } catch (cause) {
      setError(normaliseError(cause).message);
    }

    // What the vehicle says about its devices is asked for separately: a camera that does not
    // answer does not stop the gimbal from being asked, and neither is pretended.
    try {
      await readCamera();
    } catch (cause) {
      setError(normaliseError(cause).message);
    }

    try {
      const devices = await gimbalDevices();
      const lowest = devices[0];
      if (lowest !== undefined) {
        setDeviceId(lowest.deviceId);
      }
    } catch (cause) {
      setError(normaliseError(cause).message);
    }
  }, [linked, zoomType]);

  useEffect(() => {
    if (!open) {
      return;
    }

    void read();
  }, [open, read]);

  if (!open) {
    return null;
  }

  const run = async (action: () => Promise<void>, said: string) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await action();
      setNotice(said);
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const number = (value: string): number => Number.parseFloat(value);

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The camera and the gimbal")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The camera and the gimbal")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "What the camera says about itself is what can be asked of it. A camera that cannot record video in the mode it is in is refused before anything is sent.",
            )}
          </p>

          {!camera ? (
            <p className={styles.text}>
              {t("The vehicle has not said what its camera can do.")}
            </p>
          ) : (
            <dl className={styles.facts}>
              <dt>{t("Which camera it is")}</dt>
              <dd>
                {camera.vendor || t("not said")} {camera.model || ""}
              </dd>
              <dt>{t("The mode it is in")}</dt>
              <dd>{camera.mode}</dd>
              <dt>{t("The size of the picture")}</dt>
              <dd>
                {t("{width} by {height} pixels", {
                  width: camera.width,
                  height: camera.height,
                })}
              </dd>
              <dt>{t("What it says about its pictures")}</dt>
              <dd>{camera.imageStatus}</dd>
              <dt>{t("What it says about its video")}</dt>
              <dd>{camera.videoStatus}</dd>
            </dl>
          )}

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || !linked || !(camera?.takesPictureNow ?? false)}
              onClick={() => {
                void run(
                  () => capturePhoto(cameraComponent, 0),
                  t("The camera was asked for a picture."),
                );
              }}
            >
              {t("Take one picture")}
            </button>
          </div>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>
                {t("Seconds between pictures")}
              </span>
              <input
                className="input"
                type="number"
                min={1}
                step={1}
                value={interval}
                aria-label={t("The seconds between pictures")}
                onChange={(event) => {
                  setIntervalSeconds(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={
                busy ||
                !linked ||
                !(camera?.capabilities.takesPictures ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    startIntervalCapture(cameraComponent, 0, number(interval)),
                  t("The camera was asked to take pictures at that interval."),
                );
              }}
            >
              {t("Take them at an interval")}
            </button>

            <button
              type="button"
              className="button"
              disabled={
                busy ||
                !linked ||
                !(camera?.capabilities.takesPictures ?? false)
              }
              onClick={() => {
                void run(
                  () => stopIntervalCapture(cameraComponent, 0),
                  t("The interval was taken off."),
                );
              }}
            >
              {t("Stop the interval")}
            </button>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked || !(camera?.recordsVideoNow ?? false)}
              onClick={() => {
                void run(
                  () => startRecording(cameraComponent, 0),
                  t("The camera was asked to record."),
                );
              }}
            >
              {t("Record")}
            </button>

            <button
              type="button"
              className="button"
              disabled={
                busy || !linked || !(camera?.capabilities.recordsVideo ?? false)
              }
              onClick={() => {
                void run(
                  () => stopRecording(cameraComponent, 0),
                  t("The recording was stopped."),
                );
              }}
            >
              {t("Stop recording")}
            </button>
          </div>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Zoom level")}</span>
              <input
                className="input"
                type="number"
                step="any"
                value={zoomLevel}
                aria-label={t("The zoom level")}
                onChange={(event) => {
                  setZoomLevel(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("What the level means")}</span>
              <select
                className="input"
                value={zoomType}
                aria-label={t("The kind of zoom")}
                onChange={(event) => {
                  setZoomType(Number.parseInt(event.target.value, 10));
                }}
              >
                {zoomOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={
                busy || !linked || !(camera?.capabilities.hasZoom ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    setCameraZoom(cameraComponent, number(zoomLevel), zoomType),
                  t("The zoom was set."),
                );
              }}
            >
              {t("Set the zoom")}
            </button>
          </div>

          <p className={styles.text}>
            {t(
              "A place in the picture is read from `-1` to `1`: `-1` is the left or the top, `1` is the right or the bottom. Picking it from the picture itself waits on drawing the video.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Across the picture")}</span>
              <input
                className="input"
                type="number"
                min={-1}
                max={1}
                step="any"
                value={pointX}
                aria-label={t("The place across the picture")}
                onChange={(event) => {
                  setPointX(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Down the picture")}</span>
              <input
                className="input"
                type="number"
                min={-1}
                max={1}
                step="any"
                value={pointY}
                aria-label={t("The place down the picture")}
                onChange={(event) => {
                  setPointY(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={
                busy ||
                !linked ||
                !(camera?.capabilities.canTrackPoint ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    trackPoint(cameraComponent, number(pointX), number(pointY)),
                  t("The camera was told what to follow."),
                );
              }}
            >
              {t("Follow that point")}
            </button>
          </div>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("First corner, across")}</span>
              <input
                className="input"
                type="number"
                min={-1}
                max={1}
                step="any"
                value={corner1X}
                aria-label={t(
                  "The first corner of the area, across the picture",
                )}
                onChange={(event) => {
                  setCorner1X(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("First corner, down")}</span>
              <input
                className="input"
                type="number"
                min={-1}
                max={1}
                step="any"
                value={corner1Y}
                aria-label={t("The first corner of the area, down the picture")}
                onChange={(event) => {
                  setCorner1Y(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Second corner, across")}</span>
              <input
                className="input"
                type="number"
                min={-1}
                max={1}
                step="any"
                value={corner2X}
                aria-label={t(
                  "The second corner of the area, across the picture",
                )}
                onChange={(event) => {
                  setCorner2X(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Second corner, down")}</span>
              <input
                className="input"
                type="number"
                min={-1}
                max={1}
                step="any"
                value={corner2Y}
                aria-label={t(
                  "The second corner of the area, down the picture",
                )}
                onChange={(event) => {
                  setCorner2Y(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={
                busy ||
                !linked ||
                !(camera?.capabilities.canTrackRectangle ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    trackRectangle(
                      cameraComponent,
                      number(corner1X),
                      number(corner1Y),
                      number(corner2X),
                      number(corner2Y),
                    ),
                  t("The camera was told what area to follow."),
                );
              }}
            >
              {t("Follow that area")}
            </button>
          </div>
        </section>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The gimbal is pointed at an angle, or at a rate. An angle the device cannot reach is refused before anything is sent, and the yaw is read in the frame the device says it uses.",
            )}
          </p>

          <label className={styles.field}>
            <span className={styles.label}>{t("Which gimbal")}</span>
            <select
              className="input"
              value={deviceId}
              aria-label={t("Which gimbal the command names")}
              onChange={(event) => {
                setDeviceId(Number.parseInt(event.target.value, 10));
              }}
            >
              <option value={0}>{t("All gimbals")}</option>
              {gimbals.map((reported) => (
                <option key={reported.deviceId} value={reported.deviceId}>
                  {t("The gimbal {id}", { id: reported.deviceId })}
                </option>
              ))}
            </select>
          </label>

          {!gimbal ? (
            <p className={styles.text}>
              {t("The vehicle has not said what its gimbals can do.")}
            </p>
          ) : (
            <dl className={styles.facts}>
              <dt>{t("Where it points")}</dt>
              <dd>
                {gimbal.attitude
                  ? t("roll {roll}°, pitch {pitch}°, yaw {yaw}°", {
                      roll: gimbal.attitude.roll.toFixed(1),
                      pitch: gimbal.attitude.pitch.toFixed(1),
                      yaw: gimbal.attitude.yaw.toFixed(1),
                    })
                  : t("not said")}
              </dd>
              <dt>{t("How far its axes go")}</dt>
              <dd>
                {t(
                  "pitch {pitchMin}° to {pitchMax}°, yaw {yawMin}° to {yawMax}°",
                  {
                    pitchMin: gimbal.limits.pitchMin.toFixed(1),
                    pitchMax: gimbal.limits.pitchMax.toFixed(1),
                    yawMin: gimbal.limits.yawMin.toFixed(1),
                    yawMax: gimbal.limits.yawMax.toFixed(1),
                  },
                )}
              </dd>
              <dt>{t("What it can do")}</dt>
              <dd>{describeGimbal(gimbal, t)}</dd>
              <dt>{t("What it is doing")}</dt>
              <dd>{describeFlags(gimbal, t)}</dd>
              <dt>{t("Whether something is wrong with it")}</dt>
              <dd>
                {gimbal.failed ? t("It says it is failing") : t("Nothing")}
              </dd>
            </dl>
          )}

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Pitch, in degrees")}</span>
              <input
                className="input"
                type="number"
                min={-180}
                max={180}
                step="any"
                value={pitch}
                aria-label={t("The pitch angle of the gimbal, in degrees")}
                onChange={(event) => {
                  setPitch(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Yaw, in degrees")}</span>
              <input
                className="input"
                type="number"
                min={-180}
                max={180}
                step="any"
                value={yaw}
                aria-label={t("The yaw angle of the gimbal, in degrees")}
                onChange={(event) => {
                  setYaw(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={
                busy ||
                !linked ||
                !(gimbal?.capabilities.canPointLocal ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    setGimbalAngles(
                      gimbal?.deviceId ?? 0,
                      number(pitch),
                      number(yaw),
                      false,
                    ),
                  t("The gimbal was pointed at that angle."),
                );
              }}
            >
              {t("Point it there")}
            </button>

            <button
              type="button"
              className="button"
              disabled={
                busy || !linked || !(gimbal?.capabilities.hasYawLock ?? false)
              }
              onClick={() => {
                void run(
                  () => setGimbalYawLock(gimbal?.deviceId ?? 0, true),
                  t("The yaw is held against the ground."),
                );
              }}
            >
              {t("Hold the yaw")}
            </button>

            <button
              type="button"
              className="button"
              disabled={
                busy || !linked || !(gimbal?.capabilities.hasYawFollow ?? false)
              }
              onClick={() => {
                void run(
                  () => setGimbalYawLock(gimbal?.deviceId ?? 0, false),
                  t("The yaw follows the vehicle."),
                );
              }}
            >
              {t("Follow the vehicle")}
            </button>

            <button
              type="button"
              className="button"
              disabled={
                busy || !linked || !(gimbal?.capabilities.hasRetract ?? false)
              }
              onClick={() => {
                void run(
                  () => retractGimbal(gimbal?.deviceId ?? 0),
                  t("The gimbal was put in its safe position."),
                );
              }}
            >
              {t("Retract")}
            </button>

            <button
              type="button"
              className="button"
              disabled={
                busy || !linked || !(gimbal?.capabilities.hasNeutral ?? false)
              }
              onClick={() => {
                void run(
                  () => neutralGimbal(gimbal?.deviceId ?? 0),
                  t("The gimbal was put in its default position."),
                );
              }}
            >
              {t("Neutral")}
            </button>
          </div>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Pitch rate")}</span>
              <input
                className="input"
                type="number"
                step="any"
                value={pitchRate}
                aria-label={t("The rate of the pitch, in degrees a second")}
                onChange={(event) => {
                  setPitchRate(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Yaw rate")}</span>
              <input
                className="input"
                type="number"
                step="any"
                value={yawRate}
                aria-label={t("The rate of the yaw, in degrees a second")}
                onChange={(event) => {
                  setYawRate(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked || gimbal === null}
              onClick={() => {
                void run(
                  () =>
                    slewGimbal(
                      gimbal?.deviceId ?? 0,
                      number(pitch),
                      number(yaw),
                      number(pitchRate),
                      number(yawRate),
                    ),
                  t("The gimbal was told to move at those rates."),
                );
              }}
            >
              {t("Move it at those rates")}
            </button>
          </div>

          <p className={styles.text}>
            {t(
              "A gimbal can also be pointed at a place on the ground, or at another vehicle. The point of interest is held until it is taken off.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Latitude")}</span>
              <input
                className="input"
                type="number"
                min={-90}
                max={90}
                step="any"
                value={roiLatitude}
                aria-label={t("The latitude of the point of interest")}
                onChange={(event) => {
                  setRoiLatitude(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Longitude")}</span>
              <input
                className="input"
                type="number"
                min={-180}
                max={180}
                step="any"
                value={roiLongitude}
                aria-label={t("The longitude of the point of interest")}
                onChange={(event) => {
                  setRoiLongitude(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>
                {t("Height above the ground")}
              </span>
              <input
                className="input"
                type="number"
                step="any"
                value={roiAltitude}
                aria-label={t(
                  "The height of the point of interest above the ground, in metres",
                )}
                onChange={(event) => {
                  setRoiAltitude(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={
                busy ||
                !linked ||
                !(gimbal?.capabilities.canPointGlobal ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    pointGimbalAtLocation(
                      gimbal?.deviceId ?? 0,
                      number(roiLatitude),
                      number(roiLongitude),
                      number(roiAltitude),
                    ),
                  t("The gimbal was pointed at that place."),
                );
              }}
            >
              {t("Point it at that place")}
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || !linked || gimbal === null}
              onClick={() => {
                void run(
                  () => clearGimbalRoi(gimbal?.deviceId ?? 0),
                  t("The point of interest was taken off."),
                );
              }}
            >
              {t("Take the point off")}
            </button>

            <label className={styles.field}>
              <span className={styles.label}>
                {t("System id of the other vehicle")}
              </span>
              <input
                className="input"
                type="number"
                min={1}
                step={1}
                value={roiSystemId}
                aria-label={t("The system id of the vehicle to point at")}
                onChange={(event) => {
                  setRoiSystemId(event.target.value);
                }}
              />
            </label>

            <button
              type="button"
              className="button"
              disabled={
                busy ||
                !linked ||
                !(gimbal?.capabilities.canPointGlobal ?? false)
              }
              onClick={() => {
                void run(
                  () =>
                    pointGimbalAtVehicle(
                      gimbal?.deviceId ?? 0,
                      Number.parseInt(roiSystemId, 10),
                    ),
                  "The gimbal was pointed at that vehicle.",
                );
              }}
            >
              {t("Point it at that vehicle")}
            </button>
          </div>
        </section>

        {error ? <p className={styles.error}>{error}</p> : null}
        {notice ? <p className={styles.notice}>{notice}</p> : null}
      </div>
    </div>
  );
}

/** What a gimbal can do, said in one line. */
function describeGimbal(gimbal: GimbalDevice, say: Say): string {
  const capabilities = gimbal.capabilities;
  const parts: string[] = [];

  if (capabilities.hasRollAxis) {
    parts.push(say("a roll axis"));
  }
  if (capabilities.hasPitchAxis) {
    parts.push(say("a pitch axis"));
  }
  if (capabilities.hasYawAxis) {
    parts.push(say("a yaw axis"));
  }
  if (capabilities.hasYawLock) {
    parts.push(say("holds its yaw against the ground"));
  }
  if (capabilities.hasYawFollow) {
    parts.push(say("follows the vehicle"));
  }
  if (capabilities.hasRetract) {
    parts.push(say("can be retracted"));
  }
  if (capabilities.hasNeutral) {
    parts.push(say("can go to neutral"));
  }
  if (capabilities.canPointGlobal) {
    parts.push(say("points at a place on the ground"));
  }

  return parts.length > 0 ? parts.join(", ") : say("nothing it has said");
}

/** What a gimbal is running with, said in one line. */
function describeFlags(gimbal: GimbalDevice, say: Say): string {
  const flags = gimbal.flags;
  const parts: string[] = [];

  if (flags.retracted) {
    parts.push(say("retracted"));
  }
  if (flags.neutral) {
    parts.push(say("at neutral"));
  }
  if (flags.yawLock) {
    parts.push(say("yaw held against the ground"));
  } else if (flags.yawInVehicleFrame) {
    parts.push(say("yaw read from the way the vehicle points"));
  } else if (flags.yawInEarthFrame) {
    parts.push(say("yaw read from the ground"));
  }

  return parts.length > 0 ? parts.join(", ") : say("nothing it has said");
}
