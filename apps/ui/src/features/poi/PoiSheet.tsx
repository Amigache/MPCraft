import { useState } from "react";

import { formatNumber } from "../../lib/format";
import { normaliseError } from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { usePoiStore } from "../../lib/poi/poiStore";
import { useUiStore } from "../../lib/ui/uiStore";
import type { Poi } from "../../lib/ipc/types";
import styles from "./PoiSheet.module.css";

/**
 * The sheet of the places the operator marked on the map.
 *
 * A place is a name and a position, kept between two runs and drawn on the map of the flight and
 * of the plan. Upstream keeps the same list in `Utilities/POI.cs`; the rule that a place with the
 * same name at the same position is one place is the one it applies when its file is loaded more
 * than once, and it is applied here when the place is added.
 *
 * A place is picked on the map: the sheet arms the map and gets out of the way, and the next
 * click puts the point where the operator clicked. The position can also be typed, which is how
 * a place is added when the window is not on a screen with a map.
 */
export function PoiSheet() {
  const open = useUiStore((state) => state.poisOpen);
  const hide = useUiStore((state) => state.hidePois);
  const name = useUiStore((state) => state.poiName);
  const setPoiName = useUiStore((state) => state.setPoiName);
  const startPoiPlacement = useUiStore((state) => state.startPoiPlacement);
  const screen = useUiStore((state) => state.screen);

  const t = useLocaleStore((state) => state.t);

  const points = usePoiStore((state) => state.points);
  const add = usePoiStore((state) => state.add);
  const remove = usePoiStore((state) => state.remove);
  const clear = usePoiStore((state) => state.clear);

  const [latitude, setLatitude] = useState("");
  const [longitude, setLongitude] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onMap = screen === "flightdata" || screen === "plan";

  const place = async () => {
    setBusy(true);
    setError(null);

    try {
      await add(name, Number(latitude), Number(longitude));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const take = async (poi: Poi) => {
    setBusy(true);
    setError(null);

    try {
      await remove(poi);
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const takeAll = async () => {
    setBusy(true);
    setError(null);

    try {
      await clear();
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  if (!open) {
    return null;
  }

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("The places of interest")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The places of interest")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "A place is drawn on the map of the flight and of the plan, and it is kept between two runs.",
            )}
          </p>

          {error !== null ? <p className={styles.error}>{error}</p> : null}

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Name")}</span>
              <input
                className="input"
                type="text"
                value={name}
                aria-label={t("The name of the place")}
                onChange={(event) => {
                  setPoiName(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Latitude")}</span>
              <input
                className="input"
                type="text"
                inputMode="decimal"
                value={latitude}
                aria-label={t("The latitude of the place")}
                onChange={(event) => {
                  setLatitude(event.target.value);
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Longitude")}</span>
              <input
                className="input"
                type="text"
                inputMode="decimal"
                value={longitude}
                aria-label={t("The longitude of the place")}
                onChange={(event) => {
                  setLongitude(event.target.value);
                }}
              />
            </label>
          </div>

          <div className={styles.actions}>
            <button
              type="button"
              className="button"
              disabled={busy || !onMap}
              onClick={startPoiPlacement}
            >
              {t("Pick on the map")}
            </button>
            <button
              type="button"
              className="button"
              disabled={
                busy || latitude.trim() === "" || longitude.trim() === ""
              }
              onClick={() => {
                void place();
              }}
            >
              {t("Add the place")}
            </button>
            <button
              type="button"
              className="button"
              disabled={busy || points.length === 0}
              onClick={() => {
                void takeAll();
              }}
            >
              {t("Clear the places")}
            </button>
          </div>

          {!onMap ? (
            <p className={styles.text}>
              {t(
                "Open the flight data or the plan to pick a place on the map.",
              )}
            </p>
          ) : null}
        </section>

        <section className={styles.block}>
          {points.length === 0 ? (
            <p className={styles.text}>{t("No places yet.")}</p>
          ) : (
            <ul className={styles.list}>
              {points.map((poi) => (
                <li
                  className={styles.row}
                  key={`${poi.name}|${poi.latitude}|${poi.longitude}`}
                >
                  <span>{poi.name}</span>
                  <span className={styles.coordinates}>
                    {formatNumber(poi.latitude, 5)},{" "}
                    {formatNumber(poi.longitude, 5)}
                  </span>
                  <button
                    type="button"
                    className="button"
                    disabled={busy}
                    onClick={() => {
                      void take(poi);
                    }}
                  >
                    {t("Remove")}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
