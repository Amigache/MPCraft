import { LiveValue } from "../../components/LiveValue";
import { formatDistance } from "../../lib/format";
import { useLinkStore } from "../../lib/link/linkStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import styles from "./StatusBar.module.css";

/**
 * The status bar.
 *
 * What the vehicle last said, what it is doing, and how the link is holding up. The
 * numbers are written straight into the bar, so a tick does not re-render it.
 */
export function StatusBar() {
  const messages = useLinkStore((state) => state.messages);
  const loss = useLinkStore((state) => state.loss);
  const error = useActivityStore((state) => state.error);

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
        <span className={styles.label}>Mode</span>
        <LiveValue select={(telemetry) => telemetry.mode} />
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>Armed</span>
        <LiveValue
          select={(telemetry) => telemetry.armed}
          format={(value) => (value ? "yes" : "no")}
        />
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>To home</span>
        <LiveValue
          select={(telemetry) => telemetry.distanceToHome ?? null}
          format={(value) => formatDistance(value as number)}
        />
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>Received</span>
        <span className={styles.value}>{loss.received.toLocaleString("en-US")}</span>
      </span>

      <span className={styles.cell}>
        <span className={styles.label}>Lost</span>
        <span className={styles.value}>{loss.lost.toLocaleString("en-US")}</span>
      </span>
    </footer>
  );
}
