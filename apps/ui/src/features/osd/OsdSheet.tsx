import { useCallback, useEffect, useMemo, useState } from "react";

import { normaliseError, osdSlotConfig, osdSlots } from "../../lib/ipc/bridge";
import type { OsdSlot } from "../../lib/ipc/types";
import { useLinkStore } from "../../lib/link/linkStore";
import { useMetadata } from "../../lib/params/metadataStore";
import { useParamsStore } from "../../lib/params/paramsStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./OsdSheet.module.css";

/**
 * The sheet of the on-screen display.
 *
 * A vehicle draws an on-screen display over the picture of the camera. What it draws
 * is a set of **slots**, and each slot holds one thing. The screens five and six are
 * the ones the operator chooses: the vehicle is asked what a slot holds, and it is
 * told what to hold.
 *
 * Upstream has the same thing (`ConfigOSD.cs`, and the library behind it), with the
 * same two messages and the same two screens.
 */

/** The screens whose slots the operator chooses. */
const SCREENS = [5, 6];

/**
 * The range a parameter is shown with.
 *
 * The vehicle is told the range of the parameter that goes in the slot, so that it
 * knows how to draw it. The documentation knows the range of most of them.
 */
function rangeOf(
  minimum: string,
  maximum: string,
): { min: number; max: number; increment: number } {
  const low = Number(minimum);
  const high = Number(maximum);

  const min = Number.isFinite(low) ? low : 0;
  const max = Number.isFinite(high) ? high : 1;

  // A wide range is shown in steps of one, and a narrow one in steps of a hundredth.
  const increment = max - min > 10 ? 1 : 0.01;

  return { min, max, increment };
}

export function OsdSheet() {
  const open = useUiStore((state) => state.osdOpen);
  const hide = useUiStore((state) => state.hideOsd);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const parameters = useParamsStore((state) => state.parameters);

  const names = useMemo(
    () => parameters.map((parameter) => parameter.name),
    [parameters],
  );

  const { index: metadata } = useMetadata(names);

  const [slots, setSlots] = useState<readonly OsdSlot[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      setSlots(await osdSlots());
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (open && linked) {
      void read();
    }
  }, [open, linked, read]);

  if (!open) {
    return null;
  }

  const assign = async (slot: OsdSlot, name: string) => {
    if (name === "") {
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);

    const known = metadata.get(name);
    const range = rangeOf(known?.minimum ?? "", known?.maximum ?? "");

    try {
      await osdSlotConfig(
        slot.screen,
        slot.index,
        name,
        range.min,
        range.max,
        range.increment,
      );

      setSlots((before) =>
        before.map((entry) =>
          entry.screen === slot.screen && entry.index === slot.index
            ? { ...entry, name, known: true }
            : entry,
        ),
      );

      setNotice(`The slot ${slot.index} of the screen ${slot.screen} holds ${name}.`);
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
        aria-label="The on-screen display"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>The on-screen display</h2>
          <button type="button" className="button" onClick={hide}>
            Close
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            The vehicle draws the display over the picture of the camera. Each slot
            holds one thing, and the screens five and six are the ones that are
            configured here.
          </p>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              onClick={() => {
                void read();
              }}
            >
              Read the slots
            </button>

            {!linked ? (
              <span className={styles.text}>There is no link to a vehicle.</span>
            ) : null}
          </div>
        </section>

        {SCREENS.map((screen) => (
          <section key={screen} className={styles.block}>
            <h3 className={styles.heading}>Screen {screen}</h3>

            <ul className={styles.list}>
              {slots
                .filter((slot) => slot.screen === screen)
                .map((slot) => (
                  <li key={`${slot.screen}-${slot.index}`} className={styles.item}>
                    <span className={styles.number}>{slot.index}</span>

                    <select
                      className="select"
                      value={slot.name ?? ""}
                      aria-label={`What the slot ${slot.index} of the screen ${screen} holds`}
                      disabled={busy}
                      onChange={(event) => {
                        void assign(slot, event.target.value);
                      }}
                    >
                      <option value="">— nothing —</option>
                      {slot.name !== null &&
                      !parameters.some(
                        (parameter) => parameter.name === slot.name,
                      ) ? (
                        <option value={slot.name}>{slot.name}</option>
                      ) : null}
                      {parameters.map((parameter) => (
                        <option key={parameter.name} value={parameter.name}>
                          {parameter.name}
                        </option>
                      ))}
                    </select>
                  </li>
                ))}
            </ul>

            {slots.length === 0 && !busy ? (
              <p className={styles.text}>
                The vehicle did not say what its slots hold. It may not know the
                questions.
              </p>
            ) : null}
          </section>
        ))}

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
