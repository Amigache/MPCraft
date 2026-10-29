import { useCallback, useEffect, useState } from "react";

import { fileNameOf } from "../../lib/files/name";
import { pickSavePath } from "../../lib/ipc/dialog";
import { formatCount } from "../../lib/format";
import { ftpList, ftpRead, normaliseError } from "../../lib/ipc/bridge";
import type { FtpEntry } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./FilesSheet.module.css";

/**
 * The sheet of the files of the vehicle.
 *
 * The vehicle holds a filesystem of its own — the logs it has not fetched yet, the
 * scripts it runs, the folders of its data — and the file transfer protocol is how a
 * program reaches it. This sheet is that browser: the folders are listed, the operator
 * walks into them, and a file is brought to the disk of the computer.
 *
 * Upstream has the same thing in `MAVFtp.cs`, and the same shape: a listing, a path, and
 * a download.
 */

/** The folder the listing starts in. */
const ROOT = "/";

/** How a size is said. */
function showSize(bytes: number): string {
  if (bytes < 0) {
    return "—";
  }

  if (bytes === 0) {
    return "0 B";
  }

  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} kB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** The name of the file as it is written to the disk. */
export function FilesSheet() {
  const open = useUiStore((state) => state.filesOpen);
  const hide = useUiStore((state) => state.hideFiles);

  const t = useLocaleStore((state) => state.t);

  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  const [folder, setFolder] = useState(ROOT);
  const [entries, setEntries] = useState<readonly FtpEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const list = useCallback(async (path: string) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const found = await ftpList(path);
      setEntries(found);
      setFolder(path);
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  }, []);

  // The listing is read when the sheet opens, and the folder is read again when the
  // link comes up.
  useEffect(() => {
    if (open && linked) {
      void list(ROOT);
    }
  }, [open, linked, list]);

  if (!open) {
    return null;
  }

  const goUp = () => {
    if (folder === ROOT) {
      return;
    }

    const parts = folder.split("/").filter((part) => part.length > 0);
    parts.pop();

    void list(parts.length === 0 ? ROOT : `/${parts.join("/")}`);
  };

  const download = async (entry: FtpEntry) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const name = fileNameOf(entry.path);
      const picked = await pickSavePath(name, [
        { name: t("Everything"), extensions: ["*"] },
      ]);

      if (!picked.ok) {
        setBusy(false);
        return;
      }

      const file = await ftpRead(entry.path, picked.path);

      // The numbers are made into strings here and nowhere else, with the separator the language the window is
      // read in uses.
      const given = formatCount(file.size);
      const said = formatCount(file.announcedSize);

      setNotice(
        file.announcedSize === file.size
          ? t(
              file.size === 1
                ? "{count} byte of {name} was written to {path}."
                : "{count} bytes of {name} were written to {path}.",
              { count: given, name, path: picked.path },
            )
          : t(
              "{given} bytes of {name} were written to {path}. The vehicle said the file has {said} bytes, which is not what it gave.",
              { given, name, path: picked.path, said },
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
        aria-label={t("The files of the vehicle")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The files of the vehicle")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "The vehicle holds a filesystem of its own. The folders are listed the way the vehicle says them, and a file is brought to the disk of the computer.",
            )}
          </p>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || folder === ROOT}
              onClick={() => {
                goUp();
              }}
            >
              {t("Up")}
            </button>

            <span
              className={styles.path}
              aria-label={t("The folder that is listed")}
            >
              {folder}
            </span>

            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              onClick={() => {
                void list(folder);
              }}
            >
              {t("Refresh")}
            </button>
          </div>

          {!linked ? (
            <p className={styles.text}>{t("There is no link to a vehicle.")}</p>
          ) : null}
        </section>

        <section className={styles.block}>
          {entries.length === 0 ? (
            <p className={styles.text}>
              {busy
                ? t("The vehicle is being asked.")
                : t("The folder is empty.")}
            </p>
          ) : (
            <ul className={styles.list}>
              {entries.map((entry) => (
                <li key={entry.path} className={styles.item}>
                  <span className={styles.name}>
                    {entry.isDirectory ? "📁" : "📄"} {entry.name}
                  </span>

                  <span className={styles.size}>{showSize(entry.size)}</span>

                  {entry.isDirectory ? (
                    <button
                      type="button"
                      className="button"
                      disabled={busy}
                      onClick={() => {
                        void list(entry.path);
                      }}
                    >
                      {t("Open")}
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="button"
                      disabled={busy}
                      onClick={() => {
                        void download(entry);
                      }}
                    >
                      {t("Download")}
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>

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
