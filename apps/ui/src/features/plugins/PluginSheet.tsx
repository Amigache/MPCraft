import { useCallback, useEffect, useState } from "react";

import {
  listPlugins,
  normaliseError,
  pluginFolder,
  pluginNotes,
  setPluginEnabled,
} from "../../lib/ipc/bridge";
import type { PluginInfo } from "../../lib/ipc/types";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./PluginSheet.module.css";

/**
 * The sheet of the plugins.
 *
 * A plugin is somebody else's code, so the sheet says what it asks for before anybody decides to run
 * it, and what the driver says about it afterwards. Nothing runs because its file happens to be in the
 * folder: the operator has to say so here, and the decision is kept for the next run.
 *
 * Upstream has the same thing (`Plugin/PluginUI.cs`), with the same four stages — read, link open,
 * tick, close — and the same list, but it runs whatever it finds.
 */

/** What the state of a plugin is called, which is what the sheet says beside its name. */
function stateName(state: string): string {
  switch (state) {
    case "not_run":
      return "Not run";
    case "ready":
      return "Read";
    case "running":
      return "Being called";
    default:
      return "Could not be read";
  }
}

export function PluginSheet() {
  const open = useUiStore((state) => state.pluginsOpen);
  const hide = useUiStore((state) => state.hidePlugins);
  const t = useLocaleStore((state) => state.t);

  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [folder, setFolder] = useState("");
  const [notes, setNotes] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const read = useCallback(async () => {
    try {
      const readPlugins = await listPlugins();

      setPlugins(readPlugins);
      setFolder(await pluginFolder());
      setNotes(await pluginNotes());
    } catch (cause) {
      setError(normaliseError(cause).message);
    }
  }, []);

  useEffect(() => {
    if (!open) {
      return;
    }

    void read();

    const timer = window.setInterval(() => {
      void read();
    }, 1000);

    return () => {
      window.clearInterval(timer);
    };
  }, [open, read]);

  if (!open) {
    return null;
  }

  const choose = async (name: string, enabled: boolean) => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await setPluginEnabled(name, enabled);
      setNotice(
        enabled
          ? t("{name} is being run.", { name })
          : t("{name} has been left alone.", { name }),
      );
      await read();
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
        aria-label={t("The plugins")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("The plugins")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <p className={styles.text}>
            {t(
              "A plugin is somebody else's code. It is not run because its file happens to be in the folder: the operator has to say so, and the decision is kept for the next run.",
            )}
          </p>

          <p className={styles.folder}>
            <span className={styles.label}>
              {t("The folder the plugins are read from")}
            </span>
            <span className={styles.path}>{folder}</span>
          </p>
        </section>

        <section className={styles.block}>
          {plugins.length === 0 ? (
            <p className={styles.text}>{t("The folder holds nothing.")}</p>
          ) : (
            <ul className={styles.list}>
              {plugins.map((plugin) => (
                <li key={plugin.name} className={styles.item}>
                  <div className={styles.headline}>
                    <span className={styles.name}>{plugin.name}</span>
                    <span className={styles.version}>{plugin.version}</span>
                    <button
                      type="button"
                      className="button"
                      disabled={busy}
                      onClick={() => {
                        void choose(plugin.name, !plugin.enabled);
                      }}
                    >
                      {plugin.enabled ? t("Leave alone") : t("Run")}
                    </button>
                  </div>

                  <dl className={styles.facts}>
                    <dt>{t("Who wrote it")}</dt>
                    <dd>
                      {plugin.author === "" ? t("nothing said") : plugin.author}
                    </dd>
                    <dt>{t("What it asks for")}</dt>
                    <dd>
                      {plugin.capabilities.length === 0
                        ? t("nothing")
                        : plugin.capabilities.join(", ")}
                    </dd>
                    <dt>{t("How often it is called")}</dt>
                    <dd>{plugin.loopHz}</dd>
                    <dt>{t("Where it stands")}</dt>
                    <dd>{t(stateName(plugin.state))}</dd>
                    {plugin.errorCode !== null ? (
                      <>
                        <dt>{t("What went wrong")}</dt>
                        <dd className={styles.failure}>
                          {plugin.errorCode}: {plugin.errorMessage}
                        </dd>
                      </>
                    ) : null}
                  </dl>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className={styles.block}>
          <span className={styles.label}>{t("What the plugins said")}</span>

          {notes.length === 0 ? (
            <p className={styles.text}>{t("Nothing yet.")}</p>
          ) : (
            <ul className={styles.notes}>
              {notes.map((note, index) => (
                <li key={`${index}-${note}`} className={styles.note}>
                  {note}
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
