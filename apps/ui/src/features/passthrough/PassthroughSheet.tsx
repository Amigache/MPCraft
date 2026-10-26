import { useState } from "react";

import { normaliseError } from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLiveValue } from "../../lib/link/liveTelemetry";
import { useUiStore } from "../../lib/ui/uiStore";
import { serialControl } from "../../lib/ipc/bridge";
import styles from "./PassthroughSheet.module.css";

/**
 * The sheet of the serial passthrough.
 *
 * The vehicle has serial ports of its own — the telemetry ones, the ones of the GPS — and
 * a program that needs to talk to what is on them sends the bytes inside a MAVLink
 * message. This sheet is that program: the port is chosen, the text is written, and the
 * bytes go.
 *
 * Upstream has the same thing (`MAVLinkInterface.SendSerialControl`), and the same
 * shape: the port, the timeout, the speed, and the bytes.
 */

/** The ports the vehicle has. */
const PORTS = [
  { id: 0, label: "Telemetry 1" },
  { id: 1, label: "Telemetry 2" },
  { id: 4, label: "Telemetry 3" },
  { id: 2, label: "GPS 1" },
  { id: 3, label: "GPS 2" },
] as const;

export function PassthroughSheet() {
  const open = useUiStore((state) => state.passthroughOpen);
  const hide = useUiStore((state) => state.hidePassthrough);

  const t = useLocaleStore((state) => state.t);

  const [port, setPort] = useState(0);
  const [text, setText] = useState("");
  const [baudrate, setBaudrate] = useState(0);
  const [timeout, setTimeout] = useState(200);

  // What the vehicle answered with: the bytes its port had.
  const passthrough = useLiveValue(
    (telemetry) => telemetry.passthrough ?? null,
  );

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  if (!open) {
    return null;
  }

  const send = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await serialControl(port, text, baudrate, timeout);
      setNotice(
        t(
          text.length === 1
            ? "{count} byte went to the port."
            : "{count} bytes went to the port.",
          { count: text.length },
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
        aria-label={t("Serial passthrough")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("Serial passthrough")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The bytes travel inside a MAVLink message and the vehicle hands them over to the port. The speed of the port is left alone when it is zero.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Port")}</span>
              <select
                className="select"
                value={port}
                aria-label={t("The port of the vehicle")}
                onChange={(event) => {
                  setPort(Number(event.target.value));
                }}
              >
                {PORTS.map((entry) => (
                  <option key={entry.id} value={entry.id}>
                    {t(entry.label)}
                  </option>
                ))}
              </select>
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Speed")}</span>
              <input
                className="input"
                type="number"
                min={0}
                value={baudrate}
                aria-label={t("The speed of the port")}
                onChange={(event) => {
                  setBaudrate(Number(event.target.value));
                }}
              />
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Timeout ms")}</span>
              <input
                className="input"
                type="number"
                min={0}
                value={timeout}
                aria-label={t("How long the vehicle waits for the answer")}
                onChange={(event) => {
                  setTimeout(Number(event.target.value));
                }}
              />
            </label>
          </div>

          <label className={styles.wide}>
            <span className={styles.label}>{t("What is sent")}</span>
            <textarea
              className={styles.textarea}
              value={text}
              spellCheck={false}
              rows={6}
              aria-label={t("The text that goes to the port")}
              onChange={(event) => {
                setText(event.target.value);
              }}
            />
          </label>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || text.length === 0}
              onClick={() => {
                void send();
              }}
            >
              {t("Send")}
            </button>
          </div>
        </section>

        {passthrough !== null ? (
          <section className={styles.block}>
            <h3 className={styles.heading}>{t("What came back")}</h3>
            <pre className={styles.answer}>
              {new TextDecoder()
                .decode(new Uint8Array(passthrough.data))
                // The control characters are the point of this line, not an accident: the bytes a
                // port can hand over include them, and only the ones that cannot be drawn are
                // replaced with a dot.
                // eslint-disable-next-line no-control-regex
                .replace(/[\u0000-\u0008\u000e-\u001f]/g, ".")}
            </pre>
            <p className={styles.text}>
              {t(
                passthrough.data.length === 1
                  ? "{count} byte came from the port {device}."
                  : "{count} bytes came from the port {device}.",
                { count: passthrough.data.length, device: passthrough.device },
              )}
            </p>
          </section>
        ) : null}

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
