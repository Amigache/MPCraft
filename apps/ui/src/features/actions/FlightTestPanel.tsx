import { useCallback, useEffect, useRef, useState } from "react";

import {
  manualControl,
  motorTest,
  rcOverride,
  rcOverrideRelease,
} from "../../lib/ipc/bridge";
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
      void manualControl(current.roll, current.pitch, current.thrust, current.yaw, 0);
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
    [report],
  );

  return (
    <div className={styles.panel}>
      <section className={styles.block}>
        <h3 className={styles.heading}>Motor test</h3>
        <p className={styles.note}>
          One motor at a time, and only for a moment. The props are taken off, so the
          vehicle must not be flying.
        </p>

        <div className={styles.row}>
          <label className={styles.field}>
            <span className={styles.label}>Motor</span>
            <input
              className="input"
              type="number"
              min={1}
              max={8}
              value={motor}
              aria-label="The motor to test"
              onChange={(event) => setMotor(Number(event.target.value))}
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>Throttle %</span>
            <input
              className="input"
              type="number"
              min={-100}
              max={100}
              value={throttle}
              aria-label="How hard the motor is driven"
              onChange={(event) => setThrottle(Number(event.target.value))}
            />
          </label>

          <label className={styles.field}>
            <span className={styles.label}>Seconds</span>
            <input
              className="input"
              type="number"
              min={1}
              max={30}
              value={seconds}
              aria-label="For how long the motor is driven"
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
              `motor ${motor} was driven at ${throttle}% for ${seconds} s`,
            );
          }}
        >
          Test motor
        </button>
      </section>

      <section className={styles.block}>
        <h3 className={styles.heading}>Radio override</h3>
        <p className={styles.note}>
          One channel at a time. The others are left to the radio, and the vehicle goes
          back to the radio when the channel is let go.
        </p>

        <div className={styles.row}>
          <label className={styles.field}>
            <span className={styles.label}>Channel</span>
            <select
              className="select"
              value={channel}
              aria-label="The channel to take over"
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
            <span className={styles.label}>Value µs</span>
            <input
              className="input"
              type="number"
              min={800}
              max={2200}
              value={value}
              aria-label="The value of the channel"
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
                `channel ${channel} was taken over at ${value} µs`,
              );
            }}
          >
            Override
          </button>

          <button
            type="button"
            className="button"
            onClick={() => {
              void run(
                () => rcOverrideRelease(),
                "the radio has the channels again",
              );
            }}
          >
            Release
          </button>
        </div>
      </section>

      <section className={styles.block}>
        <h3 className={styles.heading}>Manual control</h3>
        <p className={styles.note}>
          The vehicle is flown with the sticks of this window. The vehicle has to be in
          the manual mode, and the sticks are sent again and again while they are held.
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
              <span className={styles.label}>{name}</span>
              <input
                type="range"
                min={-1}
                max={1}
                step={0.05}
                value={value}
                aria-label={name}
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
          title="Hold to keep the sticks being sent"
        >
          {holding ? "Sending…" : "Hold to send"}
        </button>
      </section>
    </div>
  );
}
