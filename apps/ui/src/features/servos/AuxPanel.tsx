import { useEffect, useState } from "react";

import { useLocaleStore } from "../../lib/i18n/localeStore";
import { outputFunctions } from "../../lib/ipc/bridge";
import type { CommandError, OutputFunction } from "../../lib/ipc/types";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./AuxPanel.module.css";

/**
 * What the outputs of the vehicle are for.
 *
 * The autopilot says what each output drives with a parameter: the first one may be a
 * motor, the fifth one a light and the eighth one nothing at all. The number is the
 * parameter's, and the name is the one the specification gives it.
 *
 * An output whose function the vehicle has not said is not here: the vehicle that does
 * not report it does not have it.
 */
export function AuxPanel() {
  const [functions, setFunctions] = useState<readonly OutputFunction[]>([]);
  const [error, setError] = useState<CommandError | null>(null);

  const t = useLocaleStore((state) => state.t);

  // The functions are read when the tab is opened: the vehicle says what its
  // outputs are for with the parameters, and those are only known once they have
  // been read.
  const active = useUiStore((state) => state.activeTab === "aux");

  useEffect(() => {
    if (!active) {
      return;
    }

    let cancelled = false;

    const read = async () => {
      try {
        const found = await outputFunctions();
        if (!cancelled) {
          setFunctions(found);
          setError(null);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause as CommandError);
          setFunctions([]);
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, [active]);

  return (
    <div className={styles.panel}>
      <h3 className={styles.heading}>{t("Auxiliary functions")}</h3>

      {error !== null ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message}
        </p>
      ) : null}

      {!error && functions.length === 0 ? (
        <p className={styles.empty}>
          {t("The vehicle has not said what its outputs are for.")}{" "}
          {t("It tells it with the parameters")} <code>SERVO1_FUNCTION</code>{" "}
          {t("and its brothers.")}
        </p>
      ) : null}

      <ul className={styles.list}>
        {functions.map((entry) => (
          <li key={entry.output} className={styles.row}>
            <span className={styles.output}>
              {t("Output {number}", { number: entry.output })}
            </span>
            <span className={styles.name}>
              {entry.name ?? t("Function {number}", { number: entry.value })}
            </span>
            {entry.motor !== null ? (
              <span className={styles.motor}>
                {t("Motor {number}", { number: entry.motor })}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
    </div>
  );
}
