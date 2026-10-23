import { useEffect, useMemo, useState } from "react";

import { listSerialPorts, useConnection } from "../../lib/ipc/bridge";
import type { CommandError, SerialPort } from "../../lib/ipc/types";
import { useProfilesStore } from "../../lib/connection/profilesStore";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import {
  AUTO_BAUD,
  BAUD_RATES,
  DEFAULT_BAUD,
  buildSerialTarget,
  isAutoBaud,
  parseSerialTarget,
  withAutoBaud,
  withBaud,
} from "./serialTarget";
import styles from "./ConnectionPanel.module.css";

/**
 * The connection controls.
 *
 * The target is a single string, the way the backend takes it: `serial:COM3:57600`,
 * `udp:127.0.0.1:14550`, `tcp:192.168.1.10:5760`. The panel is opened from the File
 * menu; the state of the link is always on the menu bar and in the status bar.
 *
 * The ports of the machine and the rate an autopilot uses are offered, because writing
 * `serial:COM3:115200` by hand every time is a nuisance — but the target stays a text
 * box, because a machine may have no serial ports at all, and a link may be a UDP or a
 * TCP address.
 *
 * The **links the operator has named** are below: the vehicle on the bench, the
 * simulator, the drone in the field. They are kept between two runs, and the ones
 * marked are opened when the application starts.
 */

/** The state of the list of ports. */
type PortsState = "loading" | "ready" | "failed";

export function ConnectionPanel() {
  const { status, error, connect, disconnect } = useConnection();
  const t = useLocaleStore((state) => state.t);
  const [target, setTarget] = useState("udp:127.0.0.1:14550");
  const [baud, setBaud] = useState(DEFAULT_BAUD);
  const [ports, setPorts] = useState<readonly SerialPort[]>([]);
  const [portsState, setPortsState] = useState<PortsState>("loading");
  const [portsError, setPortsError] = useState<CommandError | null>(null);
  const [busy, setBusy] = useState(false);

  const profiles = useProfilesStore((state) => state.profiles);
  const loadProfiles = useProfilesStore((state) => state.load);
  const saveProfile = useProfilesStore((state) => state.save);
  const removeProfile = useProfilesStore((state) => state.remove);
  const setAutoConnect = useProfilesStore((state) => state.setAutoConnect);
  const profileError = useProfilesStore((state) => state.error);
  const [name, setName] = useState("");

  const open = status === "disconnected" || status === "failed";

  // The ports of the machine. They are read when the panel opens.
  useEffect(() => {
    let cancelled = false;

    const read = async () => {
      try {
        const found = await listSerialPorts();
        if (cancelled) {
          return;
        }
        setPorts(found);
        setPortsState("ready");
      } catch (cause) {
        if (!cancelled) {
          setPortsError(cause as CommandError);
          setPortsState("failed");
        }
      }
    };

    void read();

    return () => {
      cancelled = true;
    };
  }, []);

  // The links the operator has named. They are read when the panel opens.
  useEffect(() => {
    void loadProfiles();
  }, [loadProfiles]);

  // What the target says about the port and the rate. A target that is not a serial
  // one says neither, and the panel then offers the rate it has been given.
  const parsed = parseSerialTarget(target);
  const device = parsed?.device ?? "";
  const rate = parsed?.baud ?? baud;

  // The rate may be left to the program, which finds it by trying the ones there are.
  const autoBaud = isAutoBaud(target);

  // The ports that are offered: the ones of the machine, and the one the target names,
  // which may be a port of another machine or one that is not there any more.
  const devices = useMemo(() => {
    const list = ports.map((port) => port.device);

    if (device.length > 0 && !list.includes(device)) {
      list.push(device);
    }

    return list;
  }, [device, ports]);

  const labelOf = (name: string): string => {
    const port = ports.find((entry) => entry.device === name);
    return port?.description ? `${name} — ${port.description}` : name;
  };

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className={styles.panel}>
      <label className={styles.field}>
        <span className={styles.label}>{t("Target")}</span>
        <input
          className="input"
          value={target}
          spellCheck={false}
          autoComplete="off"
          aria-label={t("Connection target")}
          onChange={(event) => setTarget(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && open) {
              void run(() => connect(target));
            }
          }}
        />
      </label>

      <div className={styles.pick}>
        <label className={styles.field}>
          <span className={styles.label}>{t("Port")}</span>
          <select
            className="select"
            value={device}
            aria-label={t("Serial port")}
            disabled={devices.length === 0}
            onChange={(event) => {
              const chosen = event.target.value;
              if (chosen.length > 0) {
                setTarget(buildSerialTarget(chosen, rate));
              }
            }}
          >
            {devices.length === 0 ? (
              <option value="">{t("No serial ports")}</option>
            ) : device.length === 0 ? (
              <option value="">{t("Not a serial port")}</option>
            ) : null}
            {devices.map((name) => (
              <option key={name} value={name}>
                {labelOf(name)}
              </option>
            ))}
          </select>
        </label>

        <label className={styles.field}>
          <span className={styles.label}>{t("Baud")}</span>
          <select
            className="select"
            value={autoBaud ? AUTO_BAUD : rate}
            aria-label={t("Baud rate")}
            onChange={(event) => {
              const value = event.target.value;

              if (value === AUTO_BAUD) {
                setTarget(withAutoBaud(target));
                return;
              }

              const chosen = Number.parseInt(value, 10);
              if (Number.isFinite(chosen)) {
                setBaud(chosen);
                setTarget(withBaud(target, chosen));
              }
            }}
          >
            <option value={AUTO_BAUD}>{t("Automatic")}</option>
            {BAUD_RATES.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </label>
      </div>

      {portsState === "loading" ? (
        <p className={styles.note}>{t("Looking for the serial ports…")}</p>
      ) : null}

      {portsState === "ready" && ports.length === 0 ? (
        <p className={styles.note}>
          {t(
            "This machine has no serial ports. Write the target by hand — a UDP or a TCP address works too.",
          )}
        </p>
      ) : null}

      {portsError ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{portsError.code}</span>{" "}
          {portsError.message}
        </p>
      ) : null}

      <div className={styles.row}>
        <button
          type="button"
          className="button button--primary"
          disabled={busy || !open || target.trim().length === 0}
          onClick={() => void run(() => connect(target))}
        >
          {t("Connect")}
        </button>
        <button
          type="button"
          className="button"
          disabled={busy || open}
          onClick={() => void run(() => disconnect())}
        >
          {t("Disconnect")}
        </button>
        <span className={`${styles.status} ${styles[status] ?? ""}`}>
          {t(status)}
        </span>
      </div>

      {error ? (
        <p className={styles.error} role="alert">
          <span className={styles.code}>{error.code}</span> {error.message}
        </p>
      ) : null}

      <section className={styles.saved}>
        <h3 className={styles.savedHeading}>{t("Saved links")}</h3>

        <div className={styles.saveRow}>
          <input
            className="input"
            value={name}
            spellCheck={false}
            autoComplete="off"
            placeholder={t("Name")}
            aria-label={t("The name of the link")}
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
          <button
            type="button"
            className="button"
            disabled={
              busy || name.trim().length === 0 || target.trim().length === 0
            }
            title={t("Save the target under this name")}
            onClick={() => {
              void run(async () => {
                await saveProfile({
                  name: name.trim(),
                  target: target.trim(),
                  autoConnect: false,
                });
                setName("");
              });
            }}
          >
            {t("Save")}
          </button>
        </div>

        {profiles.length === 0 ? (
          <p className={styles.note}>
            {t(
              "No link has been saved. The ones that are saved are here the next time, and the ones marked are opened when the application starts.",
            )}
          </p>
        ) : (
          <ul className={styles.savedList}>
            {profiles.map((profile) => (
              <li key={profile.name} className={styles.savedItem}>
                <input
                  type="checkbox"
                  checked={profile.autoConnect}
                  aria-label={t("Open {name} when the application starts", {
                    name: profile.name,
                  })}
                  title={t("Open this link when the application starts")}
                  disabled={busy}
                  onChange={(event) => {
                    void setAutoConnect(profile.name, event.target.checked);
                  }}
                />
                <button
                  type="button"
                  className={styles.savedName}
                  title={t("Use this link")}
                  onClick={() => {
                    setTarget(profile.target);
                    setName(profile.name);
                  }}
                >
                  {profile.name}
                </button>
                <span className={styles.savedTarget}>{profile.target}</span>
                <button
                  type="button"
                  className={styles.savedDelete}
                  aria-label={t("Forget {name}", { name: profile.name })}
                  title={t("Forget this link")}
                  disabled={busy}
                  onClick={() => {
                    void run(() => removeProfile(profile.name));
                  }}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}

        {profileError ? (
          <p className={styles.error} role="alert">
            <span className={styles.code}>{profileError.code}</span>{" "}
            {profileError.message}
          </p>
        ) : null}
      </section>
    </div>
  );
}
