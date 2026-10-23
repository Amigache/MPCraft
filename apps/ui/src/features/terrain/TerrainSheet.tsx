import { useEffect, useState } from "react";

import {
  dtedElevation,
  dtedPath,
  normaliseError,
  openDtedFolder,
} from "../../lib/ipc/bridge";
import { pickFolder } from "../../lib/ipc/dialog";
import type { DtedLibraryInfo, TerrainPathPoint } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLiveValue } from "../../lib/link/liveTelemetry";
import { usePlanEditorStore } from "../../lib/plan/planEditorStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./TerrainSheet.module.css";

/**
 * The sheet of the terrain.
 *
 * There are two kinds of ground in MPCraft. The one that is always there is the service
 * of heights, which is fetched and kept on the disk. The other is the folder of DTED
 * cells that the operator brings: the heights of the ground in the military format,
 * which is what a survey of a field is planned with.
 *
 * The sheet opens the folder, says what is in it, reads the height of the ground under
 * the vehicle, and works out a survey that follows the ground: the strips run across
 * the line that joins the two places, and the vehicle flies at the same height above
 * the ground everywhere.
 */

/** How long a strip is, in metres. */
const DEFAULT_STRIP_LENGTH = 800;

/** How far apart the strips are, in metres. */
const DEFAULT_STRIP_SPACING = 100;

/** How high over the ground the vehicle flies, in metres. */
const DEFAULT_CLEARANCE = 30;

/** How far apart two points of a strip are, in metres. */
const DEFAULT_SAMPLE_STEP = 50;

export function TerrainSheet() {
  const open = useUiStore((state) => state.terrainOpen);
  const hide = useUiStore((state) => state.hideTerrain);

  const t = useLocaleStore((state) => state.t);

  const addPath = usePlanEditorStore((state) => state.addPath);

  // Where the vehicle is: the height of the ground under it, and the start of the
  // survey.
  const position = useLiveValue((telemetry) => telemetry.position ?? null);

  const [library, setLibrary] = useState<DtedLibraryInfo | null>(null);
  const [height, setHeight] = useState<number | null>(null);
  const [path, setPath] = useState<readonly TerrainPathPoint[]>([]);

  const [fromLat, setFromLat] = useState("");
  const [fromLng, setFromLng] = useState("");
  const [toLat, setToLat] = useState("");
  const [toLng, setToLng] = useState("");
  const [stripLength, setStripLength] = useState(DEFAULT_STRIP_LENGTH);
  const [stripSpacing, setStripSpacing] = useState(DEFAULT_STRIP_SPACING);
  const [clearance, setClearance] = useState(DEFAULT_CLEARANCE);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // The survey starts where the vehicle is, and ends a hundredth of a degree further
  // on, so that the operator has something to change and not a blank form.
  useEffect(() => {
    if (!open || position === null) {
      return;
    }

    setFromLat((before) => (before === "" ? position.lat.toFixed(6) : before));
    setFromLng((before) => (before === "" ? position.lng.toFixed(6) : before));
    setToLat((before) => (before === "" ? position.lat.toFixed(6) : before));
    setToLng((before) =>
      before === "" ? (position.lng + 0.01).toFixed(6) : before,
    );
  }, [open, position]);

  if (!open) {
    return null;
  }

  const openFolder = async () => {
    const picked = await pickFolder();

    if (!picked.ok) {
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const info = await openDtedFolder(picked.path);
      setLibrary(info);
      setNotice(
        t(
          info.cells.length === 1
            ? "{count} cell was read."
            : "{count} cells were read.",
          { count: info.cells.length },
        ) +
          (info.failures > 0
            ? " " +
              t(
                info.failures === 1
                  ? "{count} file could not be read."
                  : "{count} files could not be read.",
                { count: info.failures },
              )
            : ""),
      );
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const readHeight = async () => {
    if (position === null) {
      setError(t("the vehicle has not said where it is."));
      return;
    }

    setBusy(true);
    setError(null);

    try {
      const metres = await dtedElevation(position.lat, position.lng);
      setHeight(metres);
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const workOutPath = async () => {
    const from = { lat: Number(fromLat), lng: Number(fromLng) };
    const to = { lat: Number(toLat), lng: Number(toLng) };

    if (
      !Number.isFinite(from.lat) ||
      !Number.isFinite(from.lng) ||
      !Number.isFinite(to.lat) ||
      !Number.isFinite(to.lng)
    ) {
      setError(t("the two places of the survey have to be told."));
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const points = await dtedPath(
        { ...from, alt: 0 },
        { ...to, alt: 0 },
        stripLength,
        stripSpacing,
        clearance,
        DEFAULT_SAMPLE_STEP,
      );

      setPath(points);
      setNotice(
        t(
          points.length === 1
            ? "The survey has {count} point."
            : "The survey has {count} points.",
          { count: points.length },
        ),
      );
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
        aria-label={t("Terrain")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("Terrain")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <h3 className={styles.heading}>{t("The cells of the ground")}</h3>
          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy}
              onClick={() => {
                void openFolder();
              }}
            >
              {t("Open a folder of cells…")}
            </button>
            <button
              type="button"
              className="button"
              disabled={busy || library === null}
              onClick={() => {
                void readHeight();
              }}
            >
              {t("The height under the vehicle")}
            </button>
          </div>

          {height !== null ? (
            <p className={styles.text}>
              {t(
                "The ground under the vehicle is at {metres} metres over the sea.",
                {
                  metres: Math.round(height),
                },
              )}
            </p>
          ) : null}

          {library !== null ? (
            <table className={styles.cells}>
              <thead>
                <tr>
                  <th>{t("File")}</th>
                  <th>{t("Level")}</th>
                  <th>{t("South-west corner")}</th>
                  <th>{t("Posts")}</th>
                </tr>
              </thead>
              <tbody>
                {library.cells.map((cell) => (
                  <tr key={cell.name}>
                    <td>{cell.name}</td>
                    <td>{cell.level}</td>
                    <td>
                      {cell.originLat.toFixed(3)}, {cell.originLng.toFixed(3)}
                    </td>
                    <td>
                      {cell.columns} × {cell.rows}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className={styles.text}>
              {t(
                "No folder of cells has been opened. The heights of the service are used until one is.",
              )}
            </p>
          )}
        </section>

        <section className={styles.block}>
          <h3 className={styles.heading}>
            {t("A survey that follows the ground")}
          </h3>
          <p className={styles.text}>
            {t(
              "The strips run across the line that joins the two places, one after the other, and the vehicle flies at the same height over the ground everywhere.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("From latitude")}</span>
              <input
                className="input"
                type="number"
                step="0.000001"
                value={fromLat}
                placeholder={position ? position.lat.toFixed(6) : ""}
                aria-label={t("The latitude the survey starts at")}
                onChange={(event) => {
                  setFromLat(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("From longitude")}</span>
              <input
                className="input"
                type="number"
                step="0.000001"
                value={fromLng}
                placeholder={position ? position.lng.toFixed(6) : ""}
                aria-label={t("The longitude the survey starts at")}
                onChange={(event) => {
                  setFromLng(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("To latitude")}</span>
              <input
                className="input"
                type="number"
                step="0.000001"
                value={toLat}
                aria-label={t("The latitude the survey ends at")}
                onChange={(event) => {
                  setToLat(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("To longitude")}</span>
              <input
                className="input"
                type="number"
                step="0.000001"
                value={toLng}
                aria-label={t("The longitude the survey ends at")}
                onChange={(event) => {
                  setToLng(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Strip m")}</span>
              <input
                className="input"
                type="number"
                min={1}
                value={stripLength}
                aria-label={t("How long the strips are")}
                onChange={(event) => {
                  setStripLength(Number(event.target.value));
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Apart m")}</span>
              <input
                className="input"
                type="number"
                min={1}
                value={stripSpacing}
                aria-label={t("How far apart the strips are")}
                onChange={(event) => {
                  setStripSpacing(Number(event.target.value));
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Over the ground m")}</span>
              <input
                className="input"
                type="number"
                min={0}
                value={clearance}
                aria-label={t("How high over the ground the vehicle flies")}
                onChange={(event) => {
                  setClearance(Number(event.target.value));
                }}
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || library === null}
              onClick={() => {
                void workOutPath();
              }}
            >
              {t("Work out the survey")}
            </button>

            <button
              type="button"
              className="button button--primary"
              disabled={path.length === 0}
              onClick={() => {
                addPath(
                  path.map((point) => ({
                    lat: point.lat,
                    lng: point.lng,
                    alt: point.altM,
                  })),
                );
                setPath([]);
                setNotice(t("The survey was added to the plan."));
              }}
            >
              {t("Add it to the plan")}
            </button>
          </div>
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
