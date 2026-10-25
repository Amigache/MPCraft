import { useCallback, useEffect, useState } from "react";

import {
  normaliseError,
  setSigningKeys,
  signingStatus,
} from "../../lib/ipc/bridge";
import type { SigningKeyEntry } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./SigningSheet.module.css";

/**
 * The sheet of the keys of the signed link.
 *
 * The file is meant to be read by hand, and this is the part that says what the file means once it
 * has been read: a secret is only a key when it can be read as thirty-two bytes, which is what the
 * tool that makes keys prints. A secret that cannot be read is not repaired and not guessed at — it
 * stays where the operator wrote it, and the sheet names it, because a link signed with a key that
 * is not the one the vehicle has is a link that does not work at all.
 *
 * The decision about a secret is not made here: it is made by the keyring, once, and what comes back
 * is what the keyring says. A sheet that decided for itself would disagree with the link.
 */

export function SigningSheet() {
  const open = useUiStore((state) => state.signingOpen);
  const hide = useUiStore((state) => state.hideSigning);
  const t = useLocaleStore((state) => state.t);

  const [keys, setKeys] = useState<SigningKeyEntry[]>([]);
  const [refused, setRefused] = useState<string[]>([]);
  const [signedWith, setSignedWith] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const show = useCallback(
    (status: {
      keys: readonly SigningKeyEntry[];
      refused: readonly string[];
      signedWith: string | null;
    }) => {
      setKeys([...status.keys]);
      setRefused([...status.refused]);
      setSignedWith(status.signedWith);
    },
    [],
  );

  const read = useCallback(async () => {
    try {
      show(await signingStatus());
    } catch {
      // The state is only read to be shown.
    }
  }, [show]);

  useEffect(() => {
    if (!open) {
      return;
    }

    void read();
  }, [open, read]);

  if (!open) {
    return null;
  }

  const save = async () => {
    setBusy(true);
    setNotice(null);

    try {
      show(await setSigningKeys(keys));
      setNotice(t("The keys were saved."));
    } catch (cause) {
      setNotice(normaliseError(cause).message);
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
        aria-label={t("The keys of the signed link")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The keys of the signed link")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "A key is thirty-two bytes, written as sixty-four hexadecimal characters. A secret that cannot be read as bytes is kept as it was written and said, and the first secret that can be read is the one the frames are signed with.",
            )}
          </p>

          <table className={styles.table}>
            <thead>
              <tr>
                <th>{t("The name")}</th>
                <th>{t("The secret")}</th>
                <th>{t("What the keyring makes of it")}</th>
                <th>{t("Take out")}</th>
              </tr>
            </thead>
            <tbody>
              {keys.length === 0 ? (
                <tr>
                  <td colSpan={4}>{t("Nothing has been written yet.")}</td>
                </tr>
              ) : (
                keys.map((key, index) => (
                  <tr key={`${key.name}-${index}`}>
                    <td>
                      <input
                        className="input"
                        type="text"
                        value={key.name}
                        spellCheck={false}
                        aria-label={t("The name of the key")}
                        onChange={(event) => {
                          setKeys((current) =>
                            current.map((entry, position) =>
                              position === index
                                ? { ...entry, name: event.target.value }
                                : entry,
                            ),
                          );
                        }}
                      />
                    </td>
                    <td>
                      <input
                        className="input"
                        type="text"
                        value={key.secret}
                        spellCheck={false}
                        aria-label={t("The secret, in hexadecimal")}
                        onChange={(event) => {
                          setKeys((current) =>
                            current.map((entry, position) =>
                              position === index
                                ? { ...entry, secret: event.target.value }
                                : entry,
                            ),
                          );
                        }}
                      />
                    </td>
                    <td>
                      {refused.includes(key.name)
                        ? t("cannot be read")
                        : signedWith === key.name
                          ? t("signs")
                          : t("checked against")}
                    </td>
                    <td>
                      <button
                        type="button"
                        className="button"
                        disabled={busy}
                        onClick={() => {
                          setKeys((current) =>
                            current.filter((_, position) => position !== index),
                          );
                        }}
                      >
                        {t("Take out")}
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy}
              onClick={() => {
                setKeys((current) => [...current, { name: "", secret: "" }]);
              }}
            >
              {t("Add a key")}
            </button>

            <button
              type="button"
              className="button button--primary"
              disabled={busy}
              onClick={() => {
                void save();
              }}
            >
              {t("Save")}
            </button>
          </div>
        </section>

        <section className={styles.block}>
          <dl className={styles.facts}>
            <dt>{t("The link that is opened next")}</dt>
            <dd>
              {signedWith === null
                ? t("is not signed, because nothing can be read")
                : `“${signedWith}” ${t("signs")}`}
            </dd>
            <dt>{t("The secrets that could not be read")}</dt>
            <dd>{refused.length === 0 ? t("none") : refused.join(", ")}</dd>
          </dl>
        </section>

        {notice !== null ? (
          <p className={styles.notice} role="status">
            {notice}
          </p>
        ) : null}
      </div>
    </div>
  );
}
