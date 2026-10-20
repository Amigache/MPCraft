import { useEffect, useRef, useState } from "react";

import { useLinkStore } from "../../lib/link/linkStore";
import { messageTone } from "../../lib/messages/severity";
import type { VehicleMessage } from "../../lib/ipc/types";
import styles from "./MessageConsole.module.css";

/**
 * The console of the messages the vehicle sends.
 *
 * The vehicle speaks through `STATUSTEXT`, and this is where it is read: the newest
 * message at the bottom, coloured by how serious it is, with the name of the severity
 * beside it.
 */

/** How many messages are kept. */
const LIMIT = 200;

/** Whether two messages are the same one. */
function sameMessage(left: VehicleMessage, right: VehicleMessage): boolean {
  return left.severity === right.severity && left.text === right.text;
}

export function MessageConsole() {
  const messages = useLinkStore((state) => state.messages);
  const [shown, setShown] = useState<VehicleMessage[]>([]);
  const previous = useRef<readonly VehicleMessage[]>([]);
  const list = useRef<HTMLOListElement | null>(null);

  // The backend sends the whole list every time. The console keeps its own copy, so
  // that clearing it does not fill up again with the same messages, and so that the
  // list it shows is not rebuilt for every tick of the link.
  useEffect(() => {
    const before = previous.current;
    previous.current = messages;

    if (messages.length === 0) {
      setShown([]);
      return;
    }

    // Everything after the last message that was already shown is new.
    const last = before[before.length - 1];
    let start = 0;
    if (last) {
      for (let index = messages.length - 1; index >= 0; index -= 1) {
        const message = messages[index];
        if (message && sameMessage(message, last)) {
          start = index + 1;
          break;
        }
      }
    }

    const fresh = messages.slice(start);
    if (fresh.length === 0) {
      return;
    }

    setShown((current) => [...current, ...fresh].slice(-LIMIT));
  }, [messages]);

  useEffect(() => {
    const element = list.current;
    if (element) {
      element.scrollTop = element.scrollHeight;
    }
  }, [shown.length]);

  return (
    <div className={styles.console}>
      <div className={styles.bar}>
        <span className={styles.count}>{shown.length}</span>
        <button
          type="button"
          className="button"
          disabled={shown.length === 0}
          onClick={() => setShown([])}
        >
          Clear
        </button>
      </div>

      {shown.length === 0 ? (
        <p className={styles.empty}>The vehicle has not said anything yet.</p>
      ) : (
        <ol className={styles.list} ref={list}>
          {shown.map((message, index) => {
            const tone = messageTone(message.severity);
            return (
              <li
                key={`${index}-${message.severity}-${message.text}`}
                className={`${styles.item} ${styles[tone] ?? ""}`}
              >
                <span className={styles.severity}>{message.severityName}</span>
                <span className={styles.text}>{message.text}</span>
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
}
