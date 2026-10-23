import { useEffect, useRef, useState } from "react";

import {
  normaliseError,
  rcOverride,
  rcOverrideRelease,
} from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./JoystickSheet.module.css";

/**
 * The sheet of the joystick.
 *
 * A joystick can take the radio channels over, so that the vehicle is flown from the
 * desk with the hands on the sticks instead of on the keyboard. The browser knows what
 * the joystick is doing — the Gamepad API says where every axis is — and the ground
 * station sends the channels to the vehicle.
 *
 * Upstream has a screen for this (`Joystick/JoystickSetup.cs`) with the same two ideas:
 * which axis drives which channel, and whether the channels are being sent.
 */

/** The channels of the radio that the joystick can drive. */
const CHANNELS = [
  { index: 0, name: "Channel 1 — roll" },
  { index: 1, name: "Channel 2 — pitch" },
  { index: 2, name: "Channel 3 — throttle" },
  { index: 3, name: "Channel 4 — yaw" },
] as const;

/** The lowest and the highest value a radio channel takes. */
const CHANNEL_MINIMUM = 1000;
const CHANNEL_MAXIMUM = 2000;

/** How often the channels are sent, in milliseconds. */
const SEND_INTERVAL = 50;

/** How often the joystick is read, in milliseconds. */
const READ_INTERVAL = 40;

/** The value of an axis, as a radio channel. */
function toChannel(value: number, inverted: boolean): number {
  const shaped = inverted ? -value : value;
  const half = (Math.max(-1, Math.min(1, shaped)) + 1) / 2;
  return Math.round(
    CHANNEL_MINIMUM + half * (CHANNEL_MAXIMUM - CHANNEL_MINIMUM),
  );
}

/** One of the axes of the joystick, and the channel it drives. */
interface Binding {
  /** The axis of the joystick, or nothing when the channel is not driven. */
  readonly axis: number | null;
  /** Whether the axis is read the other way round. */
  readonly inverted: boolean;
}

export function JoystickSheet() {
  const open = useUiStore((state) => state.joystickOpen);
  const hide = useUiStore((state) => state.hideJoystick);

  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);

  const [gamepads, setGamepads] = useState<string[]>([]);
  const [chosen, setChosen] = useState(0);
  const [axes, setAxes] = useState<readonly number[]>([]);
  const [bindings, setBindings] = useState<readonly Binding[]>(
    CHANNELS.map(() => ({ axis: null, inverted: false })),
  );
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const sendingRef = useRef(sending);
  sendingRef.current = sending;

  // The joystick is read from the browser, and the browser only tells about it when it
  // is asked: the axes of a joystick that is at rest are still axes.
  useEffect(() => {
    if (!open) {
      return;
    }

    const read = () => {
      const pads = navigator.getGamepads?.() ?? [];
      const names: string[] = [];

      for (const pad of pads) {
        if (pad !== null) {
          names.push(pad.id);
        }
      }

      setGamepads(names);

      const pad = pads[chosen];
      if (pad !== null && pad !== undefined) {
        setAxes([...pad.axes]);
      } else {
        setAxes([]);
      }
    };

    read();
    const timer = window.setInterval(read, READ_INTERVAL);

    return () => {
      window.clearInterval(timer);
    };
  }, [chosen, open]);

  // The channels are sent while the operator asks for it, and they are sent at the rate
  // the radio would: twenty times a second is what a transmitter does.
  useEffect(() => {
    if (!sending) {
      return;
    }

    const timer = window.setInterval(() => {
      const values: (number | null)[] = CHANNELS.map((_, index) => {
        const binding = bindings[index];
        const axis = binding?.axis ?? null;

        if (axis === null) {
          return null;
        }

        const value = axes[axis];
        if (value === undefined) {
          return null;
        }

        return toChannel(value, binding?.inverted ?? false);
      });

      void rcOverride(values).catch((cause: unknown) => {
        setError(normaliseError(cause).message);
      });
    }, SEND_INTERVAL);

    return () => {
      window.clearInterval(timer);
    };
  }, [axes, bindings, sending]);

  if (!open) {
    return null;
  }

  const linked = status === "connected" || status === "reconnecting";

  const bind = (index: number, binding: Binding) => {
    setBindings((before) =>
      before.map((entry, position) => (position === index ? binding : entry)),
    );
  };

  const stop = () => {
    setSending(false);
    void rcOverrideRelease().catch((cause: unknown) => {
      setError(normaliseError(cause).message);
    });
  };

  return (
    <div className={styles.backdrop} role="presentation" onClick={hide}>
      <div
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-label={t("Joystick")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("Joystick")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        {gamepads.length === 0 ? (
          <p className={styles.text}>
            {t(
              "No joystick has been seen. Plug one in and press a button on it: the operating system only tells the application about a joystick that has been touched.",
            )}
          </p>
        ) : (
          <label className={styles.block}>
            <span className={styles.heading}>{t("The joystick")}</span>
            <select
              className="select"
              value={chosen}
              aria-label={t("The joystick that drives the vehicle")}
              onChange={(event) => {
                setChosen(Number(event.target.value));
              }}
            >
              {gamepads.map((name, index) => (
                <option key={name} value={index}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        )}

        <ul className={styles.channels}>
          {CHANNELS.map((channel) => {
            const binding = bindings[channel.index];
            const axis = binding?.axis ?? null;
            const value = axis === null ? undefined : axes[axis];
            const name = t(channel.name);

            return (
              <li className={styles.channel} key={channel.index}>
                <span className={styles.channelName}>{name}</span>

                <select
                  className={`select ${styles.axisSelect}`}
                  value={axis === null ? "none" : String(axis)}
                  aria-label={t("The axis that drives {name}", { name })}
                  onChange={(event) => {
                    const chosenAxis = event.target.value;
                    bind(channel.index, {
                      axis: chosenAxis === "none" ? null : Number(chosenAxis),
                      inverted: binding?.inverted ?? false,
                    });
                  }}
                >
                  <option value="none">{t("Not driven")}</option>
                  {axes.map((_, index) => (
                    <option key={index} value={index}>
                      {t("Axis {number}", { number: index + 1 })}
                    </option>
                  ))}
                </select>

                <label className={styles.invert}>
                  <input
                    type="checkbox"
                    checked={binding?.inverted ?? false}
                    aria-label={t(
                      "Whether {name} is read the other way round",
                      {
                        name,
                      },
                    )}
                    onChange={(event) => {
                      bind(channel.index, {
                        axis: binding?.axis ?? null,
                        inverted: event.target.checked,
                      });
                    }}
                  />
                  <span>{t("Inverted")}</span>
                </label>

                <span className={styles.bar} aria-hidden="true">
                  <span
                    className={styles.fill}
                    style={{
                      width: `${value === undefined ? 0 : ((value + 1) / 2) * 100}%`,
                    }}
                  />
                </span>

                <span className={styles.value}>
                  {value === undefined
                    ? "—"
                    : toChannel(value, binding?.inverted ?? false)}
                </span>
              </li>
            );
          })}
        </ul>

        <div className={styles.actions}>
          <button
            type="button"
            className={`button ${sending ? styles.on : ""}`}
            disabled={!linked || gamepads.length === 0}
            aria-pressed={sending}
            title={
              linked
                ? t("Take the radio channels over")
                : t("there is no link to the vehicle")
            }
            onClick={() => {
              setError(null);
              setNotice(null);
              setSending(!sending);
            }}
          >
            {sending ? t("Stop sending") : t("Send to the vehicle")}
          </button>

          <button
            type="button"
            className="button"
            disabled={!linked}
            onClick={stop}
            title={t("Let the radio channels go back to the radio")}
          >
            {t("Release the channels")}
          </button>
        </div>

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
