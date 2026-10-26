import { useCallback, useEffect, useRef, useState } from "react";

import {
  manualControl,
  motorTest,
  rcOverride,
  rcOverrideRelease,
} from "../../lib/ipc/bridge";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useActivityStore } from "../../lib/ui/activityStore";
import styles from "./FlightTestPanel.module.css";

/**
 * The tests of the vehicle.
 *
 * The motors are run one at a time, to see whether they turn the way they should; the
 * radio channels are taken over one at a time, to see whether the vehicle obeys the
 * sticks; and the vehicle can be flown with the sticks of this window.
 *
 * Every one of them moves the vehicle, and every one of them asks first.
 */

/** How often the sticks are sent while they are held. */
const MANUAL_INTERVAL = 200;

/** The channels that can be taken over. */
const CHANNELS = 8;

export function FlightTestPanel() {
  const report = useActivityStore((state) => state.report);

  const t = useLocaleStore((state) => state.t);

  const [motor, setMotor] = useState(1);
  const [throttle, setThrottle] = useState(50);
  const [seconds, setSeconds] = useState(3);

  const [channel, setChannel] = useState(4);
  const [value, setValue] = useState(1500);

  const [roll, setRoll] = useState(0);
  const [pitch, setPitch] = useState(0);
  const [thrust, setThrust] = useState(0);
  const [yaw, setYaw] = useState(0);
  const [holding, setHolding] = useState(false);

  const sticks = useRef({ roll: 0, pitch: 0, thrust: 0, yaw: 0 });

  sticks.current = { roll, pitch, thrust, yaw };

  // The sticks are sent again and again while they are held: a vehicle that is told
  // where to fly once expects to be told again, and goes back to the radio when it is
  // not.
  useEffect(() => {
    if (!holding) {
      return;
    }

    const timer = window.setInterval(() => {
      const current = sticks.current;
      void manualControl(
        current.roll,
        current.pitch,
        current.thrust,
        current.yaw,
        0,
      );
    }, MANUAL_INTERVAL);

    return () => {
      window.clearInterval(timer);
    };
  }, [holding]);

  const run = useCallback(
    async (action: () => Promise<void>, message: string) => {
      try {
        await action();
        report(null, message);
      } catch (cause) {
        report(
          {
            code: "failed",
            message: cause instanceof Error ? cause.message : String(cause),
          },
          null,
        );
      }
    },
    // `t` is read by the panel itself, not by this callback, so it is not part of what the
    // callback closes over.
    [report],
  );

  return (
    <div className={styles.panel}>
      <section className={styles.block}>
        <h3 className={styles.heading}>{t("Motor test")}</h3>
        <p className={styles.note}>
          {t(
            "One motor at a time, and only for a moment. The props are taken off, so the vehicle must not be flying.",
          )}
        </p>

        <div className={styles.row}>
          <label className={styles.field}>
            <span className={styles.label}>{t("Motor")}</span>
            <input
              className="input"
              type="number"
              min={1}
              max={8}
              value={motor}
              aria-label={t("The motor to test")}
              onChange={(event) => setMotor(Number(event.target.value))}
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t("Throttle %")}</span>
            <input
              className="input"
              type="number"
              min={-100}
              max={100}
              value={throttle}
              aria-label={t("How hard the motor is driven")}
              onChange={(event) => setThrottle(Number(event.target.value))}
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t("Seconds")}</span>
            <input
              className="input"
              type="number"
              min={1}
              max={30}
              value={seconds}
              aria-label={t("For how long the motor is driven")}
              onChange={(event) => setSeconds(Number(event.target.value))}
            />
          </label>
        </div>

        <button
          type="button"
          className="button button--primary"
          onClick={() => {
            void run(
              () => motorTest(motor, throttle / 100, seconds),
              t("motor {motor} was driven at {throttle}% for {seconds} s", {
                motor,
                throttle,
                seconds,
              }),
            );
          }}
        >
          {t("Test motor")}
        </button>
      </section>

      <section className={styles.block}>
        <h3 className={styles.heading}>{t("Radio override")}</h3>
        <p className={styles.note}>
          {t(
            "One channel at a time. The others are left to the radio, and the vehicle goes back to the radio when the channel is let go.",
          )}
        </p>

        <div className={styles.row}>
          <label className={styles.field}>
            <span className={styles.label}>{t("Channel")}</span>
            <select
              className="select"
              value={channel}
              aria-label={t("The channel to take over")}
              onChange={(event) => setChannel(Number(event.target.value))}
            >
              {Array.from({ length: CHANNELS }, (_, index) => (
                <option key={index} value={index + 1}>
                  {index + 1}
                </option>
              ))}
            </select>
          </label>

          <label className={styles.field}>
            <span className={styles.label}>{t("Value µs")}</span>
            <input
              className="input"
              type="number"
              min={800}
              max={2200}
              value={value}
              aria-label={t("The value of the channel")}
              onChange={(event) => setValue(Number(event.target.value))}
            />
          </label>
        </div>

        <div className={styles.row}>
          <button
            type="button"
            className="button"
            onClick={() => {
              const channels: (number | null)[] = Array(CHANNELS).fill(null);
              channels[channel - 1] = value;
              void run(
                () => rcOverride(channels),
                t("channel {channel} was taken over at {value} µs", {
                  channel,
                  value,
                }),
              );
            }}
          >
            {t("Override")}
          </button>

          <button
            type="button"
            className="button"
            onClick={() => {
              void run(
                () => rcOverrideRelease(),
                t("the radio has the channels again"),
              );
            }}
          >
            {t("Release")}
          </button>
        </div>
      </section>

      <section className={styles.block}>
        <h3 className={styles.heading}>{t("Manual control")}</h3>
        <p className={styles.note}>
          {t(
            "The vehicle is flown with the sticks of this window. The vehicle has to be in the manual mode, and the sticks are sent again and again while they are held.",
          )}
        </p>

        <div className={styles.sticks}>
          {(
            [
              ["Roll", roll, setRoll],
              ["Pitch", pitch, setPitch],
              ["Thrust", thrust, setThrust],
              ["Yaw", yaw, setYaw],
            ] as const
          ).map(([name, value, setValue]) => (
            <label key={name} className={styles.stick}>
              <span className={styles.label}>{t(name)}</span>
              <input
                type="range"
                min={-1}
                max={1}
                step={0.05}
                value={value}
                aria-label={t(name)}
                onChange={(event) => {
                  const next = Number(event.target.value);
                  setValue(next);

                  const current = sticks.current;
                  void manualControl(
                    name === "Roll" ? next : current.roll,
                    name === "Pitch" ? next : current.pitch,
                    name === "Thrust" ? next : current.thrust,
                    name === "Yaw" ? next : current.yaw,
                    0,
                  );
                }}
              />
              <span className={styles.value}>{value.toFixed(2)}</span>
            </label>
          ))}
        </div>

        <button
          type="button"
          className={`button ${holding ? styles.holding : ""}`}
          aria-pressed={holding}
          onPointerDown={() => {
            setHolding(true);
          }}
          onPointerUp={() => {
            setHolding(false);
          }}
          onPointerLeave={() => {
            setHolding(false);
          }}
          title={t("Hold to keep the sticks being sent")}
        >
          {holding ? t("Sending…") : t("Hold to send")}
        </button>
      </section>
    </div>
  );
}
