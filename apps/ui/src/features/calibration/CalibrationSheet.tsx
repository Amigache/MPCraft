import { useState } from "react";

import { ConfirmButton } from "../../components/ConfirmButton";
import {
  calibrate,
  normaliseError,
  writeParameter,
} from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./CalibrationSheet.module.css";

/**
 * The sheet of the calibrations.
 *
 * The vehicle calibrates itself: the order says which sensor, and the vehicle counts the
 * positions and works the offsets out. The interface only sends the order and says what
 * the operator has to do while it happens.
 *
 * Upstream has a screen for the accelerometer and another for the compass, and the menu
 * of the setup carries both. MPCraft has one sheet with the two of them, because the
 * difference between them is the sensor and not the shape of the screen.
 */

/** What the calibrations are called in the order that is sent. */
type Calibration = "accelerometer" | "level" | "compass";

export function CalibrationSheet() {
  const open = useUiStore((state) => state.calibrationOpen);
  const hide = useUiStore((state) => state.hideCalibration);
  const showParameterPrefix = useUiStore((state) => state.showParameterPrefix);
  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);

  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!open) {
    return null;
  }

  const linked = status === "connected" || status === "reconnecting";

  const run = async (kind: Calibration, said: string) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await calibrate(kind);
      setNotice(said);
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  /**
   * Asks the vehicle to calibrate the electronic speed controllers.
   *
   * The number is the one the autopilot understands: three tells the controllers to
   * listen and learn. It is the same number upstream writes.
   */
  const runEsc = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await writeParameter("ESC_CALIBRATION", 3);
      setNotice(
        t(
          "The vehicle was told to calibrate the electronic speed controllers.",
        ),
      );
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className={styles.backdrop}
      role="presentation"
      onClick={() => {
        hide();
      }}
    >
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby="calibration-title"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <header className={styles.header}>
          <h2 className={styles.title} id="calibration-title">
            {t("Calibration")}
          </h2>
          <div className={styles.headerActions}>
            <button
              type="button"
              className="button"
              onClick={() => {
                hide();
              }}
            >
              {t("Close")}
            </button>
          </div>
        </header>

        <div className={styles.body}>
          <section className={styles.block}>
            <h3 className={styles.heading}>{t("Accelerometer")}</h3>

            <p className={styles.text}>
              {t(
                "The vehicle measures the accelerometer in six positions and works the offsets out. Put it on a level table, on its side, on its nose, on its back and on each of the other two corners, and tell it when it is in each one.",
              )}
            </p>

            <div className={styles.actions}>
              <ConfirmButton
                label={t("Full calibration")}
                confirmLabel={t("Start the six positions")}
                variant="primary"
                disabled={busy || !linked}
                onConfirm={() => {
                  void run(
                    "accelerometer",
                    t("The vehicle was told to calibrate the accelerometer."),
                  );
                }}
              />

              <ConfirmButton
                label={t("Level calibration")}
                confirmLabel={t("Start the level calibration")}
                disabled={busy || !linked}
                onConfirm={() => {
                  void run(
                    "level",
                    t(
                      "The vehicle was told to calibrate the accelerometer level.",
                    ),
                  );
                }}
              />
            </div>
          </section>

          <section className={styles.block}>
            <h3 className={styles.heading}>{t("Compass")}</h3>

            <p className={styles.text}>
              {t(
                "The compass has to be away from the metal of the vehicle and of what is around it: take the motors off, or at least do not let them run, and keep the vehicle away from the bench and from the computer. Turn it slowly in every direction while the calibration runs.",
              )}
            </p>

            <div className={styles.actions}>
              <ConfirmButton
                label={t("Start the calibration")}
                confirmLabel={t("Start the compass calibration")}
                variant="primary"
                disabled={busy || !linked}
                onConfirm={() => {
                  void run(
                    "compass",
                    t("The vehicle was told to calibrate the compass."),
                  );
                }}
              />

              <button
                type="button"
                className="button"
                onClick={() => {
                  hide();
                  showParameterPrefix("COMPASS_MOT");
                }}
              >
                {t("The compensation of the motors")}
              </button>
            </div>

            <p className={styles.note}>
              {t(
                "The compensation of the motors is a parameter of the vehicle — the compass of a vehicle with motors is never quite right without it.",
              )}
            </p>
          </section>

          <section className={styles.block}>
            <h3 className={styles.heading}>{t("ESC")}</h3>

            <p className={styles.text}>
              {t(
                "The electronic speed controllers have a calibration of their own: the vehicle tells them to listen, they learn the position of the throttle and the timing of the signal, and then they are ready. The width of the pulse the vehicle sends them is a parameter of the vehicle.",
              )}
            </p>

            <div className={styles.actions}>
              <ConfirmButton
                label={t("Calibrate the ESC")}
                confirmLabel={t("Start the calibration of the ESC")}
                variant="primary"
                disabled={busy || !linked}
                onConfirm={() => {
                  void runEsc();
                }}
              />

              <button
                type="button"
                className="button"
                onClick={() => {
                  hide();
                  showParameterPrefix("MOT_PWM");
                }}
              >
                {t("The pulse of the motors")}
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

          {!linked ? (
            <p className={styles.note}>
              {t("There is no vehicle on the link.")}
            </p>
          ) : null}
        </div>
      </div>
    </div>
  );
}
