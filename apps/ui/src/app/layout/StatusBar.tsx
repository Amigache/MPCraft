import { LiveValue } from "../../components/LiveValue";
import { formatDistance } from "../../lib/format";
import { useLinkStore } from "../../lib/link/linkStore";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import styles from "./StatusBar.module.css";

/**
 * The status bar.
 *
 * What the vehicle last said, what it is doing, and how the link is holding up. The
 * numbers are written straight into the bar, so a tick does not re-render it.
 *
 * The names are said through the bundles, and the counts are written the way the language the
 * window is read in writes them, because the separator of the thousands is not the same in both.
 */
export function StatusBar() {
  const messages = useLinkStore((state) => state.messages);
  const loss = useLinkStore((state) => state.loss);
  const error = useActivityStore((state) => state.error);
  const t = useLocaleStore((state) => state.t);
  const tag = useLocaleStore((state) => state.tag);

  const last = messages.length > 0 ? messages[messages.length - 1] : null;

  return (
    <footer className={styles.bar}>
      <span
        className={error ? styles.fault : styles.message}
        role={error ? "alert" : "status"}
        title={error ? `${error.code}: ${error.message}` : undefined}
      >
        {error ? `${error.code}: ${error.message}` : (last?.text ?? "")}
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>{t("Mode")}</span>
        <LiveValue select={(telemetry) => telemetry.mode} />
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>{t("Armed")}</span>
        <LiveValue
          select={(telemetry) => telemetry.armed}
          format={(value) => (value ? t("yes") : t("no"))}
        />
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>{t("To home")}</span>
        <LiveValue
          select={(telemetry) => telemetry.distanceToHome ?? null}
          format={(value) => formatDistance(value as number)}
        />
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>{t("Received")}</span>
        <span className={styles.value}>
          {loss.received.toLocaleString(tag)}
        </span>
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>{t("Lost")}</span>
        <span className={styles.value}>{loss.lost.toLocaleString(tag)}</span>
      </span>
    </footer>
  );
}
