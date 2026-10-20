import { useEffect, useState } from "react";

import { listen } from "../../lib/ipc/bridge";

/**
 * The message console.
 *
 * It shows the notices the backend publishes, which is where a link problem is explained:
 * "no heartbeat received", "the board is printing at the wrong baud rate", and so on.
 */
export function MessageConsole() {
  const [notices, setNotices] = useState<string[]>([]);

  useEffect(() => {
    const unlisten = listen<{ message: string }>("notice", (event) => {
      setNotices((current) => [event.payload.message, ...current].slice(0, 50));
    });

    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return (
    <section className="panel panel--messages">
      <header className="panel__header">
        <h2>Messages</h2>
        <span className="panel__badge">{notices.length}</span>
      </header>

      {notices.length === 0 ? (
        <p className="messages__empty">No messages yet.</p>
      ) : (
        <ul className="messages__list">
          {notices.map((notice, index) => (
            <li key={`${index}-${notice}`} className="messages__item">
              {notice}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
