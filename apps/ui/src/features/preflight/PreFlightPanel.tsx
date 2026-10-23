import { useEffect, useState } from "react";

import { useLocaleStore } from "../../lib/i18n/localeStore";
import { preflightChecklist } from "../../lib/ipc/bridge";
import type { ChecklistEntry, CommandError } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import styles from "./PreFlightPanel.module.css";

/**
 * The pre-flight checklist.
 *
 * The rules are the vehicle's: the backend checks them and the panel shows what it says.
 * The first items watch a value of the vehicle — the fix, the satellites, the link, the
 * battery — and the rest are questions the pilot answers with their own hands.
 *
 * A value nobody measured is **unknown**, which is not the same as a failed check: a
 * checklist that says "no" about something that was never measured is lying.
 */
export function PreFlightPanel() {
  const [entries, setEntries] = useState<readonly ChecklistEntry[]>([]);
  const [answered, setAnswered] = useState<Readonly<Record<string, boolean>>>(
    {},
  );
  const [error, setError] = useState<CommandError | null>(null);

  const status = useLinkStore((state) => state.status);

  const t = useLocaleStore((state) => state.t);

  // The checks follow the vehicle, so they are asked for again and again.
  useEffect(() => {
    if (status !== "connected" && status !== "reconnecting") {
      setEntries([]);
      setError(null);
      return;
    }

    let cancelled = false;

    const refresh = async () => {
      try {
        const found = await preflightChecklist();
        if (!cancelled) {
          setEntries(found);
          setError(null);
        }
      } catch (cause) {
        if (!cancelled) {
          setError(cause as CommandError);
        }
      }
    };

    void refresh();
    const timer = window.setInterval(() => void refresh(), 1000);

    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [status]);

  const answer = (description: string) => {
    setAnswered((current) => ({
      ...current,
      [description]: !current[description],
    }));
  };

  if (error) {
    return (
      <div className={styles.panel}>
        <p className={styles.error} role="alert">
          {error.code} {error.message}
        </p>
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className={styles.panel}>
        <p className={styles.empty}>
          {status === "connected"
            ? t("The checklist is empty.")
            : t("There is no vehicle on the link.")}
        </p>
      </div>
    );
  }

  return (
    <div className={styles.panel}>
      <ul className={styles.list}>
        {entries.map((entry) => {
          if (entry.manual) {
            const checked = answered[entry.description] ?? false;

            return (
              <li key={entry.description} className={styles.row}>
                <label className={styles.manual}>
                  <input
                    type="checkbox"
                    checked={checked}
                    onChange={() => answer(entry.description)}
                  />
                  <span className={styles.description}>
                    {entry.description}
                  </span>
                </label>
              </li>
            );
          }

          // An unknown value is a dash, not a cross.
          const state = !entry.known
            ? "unknown"
            : entry.passed
              ? "passed"
              : "failed";

          return (
            <li
              key={entry.description}
              className={styles.row}
              data-state={state}
              title={entry.description}
            >
              <span className={styles.mark} aria-hidden="true">
                {state === "passed" ? "✓" : state === "failed" ? "✗" : "—"}
              </span>
              <span className={styles.description}>{entry.description}</span>
              <span className={styles.value}>{entry.text}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
