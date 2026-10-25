import { useCallback, useState } from "react";

import {
  estimatePictureOffset,
  matchPictures,
  normaliseError,
  readPictures,
  writeGeotagged,
} from "../../lib/ipc/bridge";
import { pickFolder, pickOpenPath } from "../../lib/ipc/dialog";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import {
  DASH,
  formatCoordinate,
  formatInstant,
  formatNumber,
  present,
} from "../../lib/format";
import { usePictureStore } from "../../lib/georef/pictureStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./PictureSheet.module.css";

/**
 * The sheet of the pictures and where they were taken.
 *
 * A picture carries the place it was taken inside itself, but the clock of a camera is not the
 * clock of the ground station: the two are apart by whatever the camera's clock was showing when
 * the shutter opened. The difference is worked out from the pictures at both ends of a folder and
 * the samples of the log at the same positions, which is what upstream does in `EstimateOffset`.
 *
 * A picture whose instant is found in the log gets the place the log says the vehicle was at that
 * instant; one that is not, keeps only the place its own file carries. The two places are shown
 * next to each other rather than merged, because the ground station should not say that it knows
 * something it does not.
 *
 * The sheet needs no link: it reads files, so it is open whether or not a vehicle is on the wire.
 */
export function PictureSheet() {
  const open = useUiStore((state) => state.picturesOpen);
  const hide = useUiStore((state) => state.hidePictures);
  const t = useLocaleStore((state) => state.t);

  const folder = usePictureStore((state) => state.folder);
  const log = usePictureStore((state) => state.log);
  const offset = usePictureStore((state) => state.offset);
  const lag = usePictureStore((state) => state.lag);
  const pictures = usePictureStore((state) => state.pictures);

  const setFolder = usePictureStore((state) => state.setFolder);
  const setLog = usePictureStore((state) => state.setLog);
  const setOffset = usePictureStore((state) => state.setOffset);
  const setLag = usePictureStore((state) => state.setLag);
  const setPictures = usePictureStore((state) => state.setPictures);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const run = useCallback(async (what: () => Promise<string>) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      setNotice(await what());
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  }, []);

  const chooseFolder = () => {
    void pickFolder().then((picked) => {
      if (picked.ok) {
        setFolder(picked.path);
      }
    });
  };

  const chooseLog = () => {
    void pickOpenPath([
      { name: t("Telemetry log"), extensions: ["tlog"] },
    ]).then((picked) => {
      if (picked.ok) {
        setLog(picked.path);
      }
    });
  };

  const readThePictures = () =>
    run(async () => {
      const found = await readPictures(folder);
      setPictures(found);

      return found.length === 1
        ? t("The folder holds {count} picture.", { count: found.length })
        : t("The folder holds {count} pictures.", { count: found.length });
    });

  const workTheClockOut = () =>
    run(async () => {
      const difference = await estimatePictureOffset(folder, log);

      if (difference === null) {
        return t(
          "The instants of the folder cannot be lined up with the log, so nothing was worked out.",
        );
      }

      setOffset(difference);

      return t(
        "The clock of the camera is {seconds} seconds from the clock of the log.",
        {
          seconds: formatNumber(difference, 1),
        },
      );
    });

  const matchWithTheLog = () =>
    run(async () => {
      const matched = await matchPictures(folder, log, offset, lag);
      setPictures(matched);

      const matchedCount = matched.filter(
        (picture) => picture.matchedPlace !== null,
      ).length;

      return t(
        "{count} of {total} pictures were matched with a sample of the log.",
        { count: matchedCount, total: matched.length },
      );
    });

  const writeTheCopies = () =>
    run(async () => {
      const written = await writeGeotagged(
        folder,
        `${folder}/geotagged`,
        log,
        offset,
        lag,
      );

      return t("{written} copies were written to {folder}/geotagged.", {
        written,
        folder,
      });
    });

  // The sheet is only drawn when it is the sheet that is open, but what it read is kept in the
  // store, so the map still draws the places of a folder that was read before the sheet was
  // closed.
  if (!open) {
    return null;
  }

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The pictures and where they were taken")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>
            {t("The pictures and where they were taken")}
          </h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The place a picture says is what its own file carries. The place the log says is where the vehicle was at the instant the camera made it, once the difference between the two clocks is worked out from the pictures at both ends of the folder. A picture that is more than two seconds from any sample of the log keeps only its own place.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>
                {t("The folder of the pictures")}
              </span>
              <input
                className="input"
                type="text"
                value={folder}
                onChange={(event) => setFolder(event.target.value)}
                placeholder={t("the folder the pictures are in")}
              />
            </label>

            <button type="button" className="button" onClick={chooseFolder}>
              {t("Choose the folder")}
            </button>
          </div>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("The log of the flight")}</span>
              <input
                className="input"
                type="text"
                value={log}
                onChange={(event) => setLog(event.target.value)}
                placeholder={t("the telemetry log to match with")}
              />
            </label>

            <button type="button" className="button" onClick={chooseLog}>
              {t("Choose the log")}
            </button>
          </div>

          <div className={styles.fields}>
            <label className={`${styles.field} ${styles.seconds}`}>
              <span className={styles.label}>
                {t("The two clocks, in seconds")}
              </span>
              <input
                className="input"
                type="number"
                step="0.1"
                value={offset}
                onChange={(event) =>
                  setOffset(Number.parseFloat(event.target.value))
                }
              />
            </label>

            <label className={`${styles.field} ${styles.seconds}`}>
              <span className={styles.label}>
                {t("What the shutter took, in seconds")}
              </span>
              <input
                className="input"
                type="number"
                step="0.1"
                value={lag}
                onChange={(event) =>
                  setLag(Number.parseFloat(event.target.value))
                }
              />
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              onClick={readThePictures}
              disabled={busy || folder === ""}
            >
              {t("Read the pictures")}
            </button>
            <button
              type="button"
              className="button"
              onClick={workTheClockOut}
              disabled={busy || folder === "" || log === ""}
            >
              {t("Work the clock out")}
            </button>
            <button
              type="button"
              className="button"
              onClick={matchWithTheLog}
              disabled={busy || folder === "" || log === ""}
            >
              {t("Match them with the log")}
            </button>
            <button
              type="button"
              className="button"
              onClick={writeTheCopies}
              disabled={busy || folder === "" || log === ""}
            >
              {t("Write the copies")}
            </button>
          </div>
        </section>

        <section className={styles.block}>
          {pictures.length === 0 ? (
            <p className={styles.text}>
              {t(
                "Nothing is read yet. A folder that holds no picture with a block inside it says nothing, and a picture whose instant the log does not hold has no second place.",
              )}
            </p>
          ) : (
            <table className={styles.table}>
              <thead>
                <tr>
                  <th>{t("The file")}</th>
                  <th>{t("When the camera says it was made")}</th>
                  <th>{t("The place the file says")}</th>
                  <th>{t("The place the log says")}</th>
                  <th>{t("Seconds apart")}</th>
                </tr>
              </thead>
              <tbody>
                {pictures.map((picture) => (
                  <tr key={picture.name}>
                    <td>{picture.name}</td>
                    <td>{formatInstant(picture.takenAt)}</td>
                    <td>
                      {picture.place === null
                        ? DASH
                        : formatCoordinate(
                            picture.place.lat,
                            picture.place.lng,
                          )}
                    </td>
                    <td>
                      {picture.matchedPlace === null
                        ? DASH
                        : formatCoordinate(
                            picture.matchedPlace.lat,
                            picture.matchedPlace.lng,
                          )}
                    </td>
                    <td>
                      {present(picture.awaySeconds)
                        ? formatNumber(picture.awaySeconds, 1)
                        : DASH}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>

        {notice !== null && <p className={styles.notice}>{notice}</p>}
        {error !== null && <p className={styles.error}>{error}</p>}
      </div>
    </div>
  );
}
