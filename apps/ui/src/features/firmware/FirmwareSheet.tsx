import { useEffect, useState } from "react";

import { pickSavePath } from "../../lib/ipc/dialog";
import {
  firmwareDownload,
  firmwareEraseBoard,
  firmwareManifest,
  firmwareWriteToBoard,
  flashBootloader,
  listSerialPorts,
  normaliseError,
  rebootIntoBootloader,
} from "../../lib/ipc/bridge";
import type {
  BoardInfo,
  FirmwareBoard,
  FirmwareManifest,
  SerialPort,
} from "../../lib/ipc/types";
import { formatCount, formatText } from "../../lib/format";
import { useLocaleStore } from "../../lib/i18n/localeStore";
import { useLinkStore } from "../../lib/link/linkStore";
import { useUiStore } from "../../lib/ui/uiStore";
import styles from "./FirmwareSheet.module.css";

/**
 * The sheet of the firmware.
 *
 * The firmware of the vehicles is published in a manifest: a document that says, for
 * every vehicle and every version, where the file is and what its hash is. The sheet
 * reads that manifest and lets the operator choose the vehicle, the version and the
 * platform, and fetch the file.
 *
 * Upstream has a screen for this (`ConfigFirmware.cs`) and it does the same, with the
 * download of the file and the writing of it to the vehicle on top.
 */

/** How a size is said. */
function showSize(bytes: number): string {
  if (bytes <= 0) {
    return "—";
  }

  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} kB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function FirmwareSheet() {
  const open = useUiStore((state) => state.firmwareOpen);
  const hide = useUiStore((state) => state.hideFirmware);

  const t = useLocaleStore((state) => state.t);

  const [manifest, setManifest] = useState<FirmwareManifest | null>(null);
  const [board, setBoard] = useState("");
  const [version, setVersion] = useState("");
  const [platform, setPlatform] = useState("");

  const [ports, setPorts] = useState<readonly SerialPort[]>([]);
  const [port, setPort] = useState("");
  const [file, setFile] = useState<string | null>(null);
  const [boardInfo, setBoardInfo] = useState<BoardInfo | null>(null);

  const [busy, setBusy] = useState(false);
  const [allowSame, setAllowSame] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // Both of these need a link to a vehicle: one writes a sector of the board's flash, the other asks
  // the board to restart and stay in its loader.
  const status = useLinkStore((state) => state.status);
  const linked = status === "connected" || status === "reconnecting";

  // The manifest is read when the sheet is opened: it changes with every release, and
  // the operator is here to see what there is now.
  useEffect(() => {
    if (!open) {
      return;
    }

    let cancelled = false;

    setBusy(true);
    setError(null);

    void listSerialPorts()
      .then((found) => {
        if (cancelled) {
          return;
        }

        setPorts(found);
        setPort((before) =>
          before === "" ? (found[0]?.device ?? "") : before,
        );
      })
      .catch(() => {
        if (!cancelled) {
          setPorts([]);
        }
      });

    void firmwareManifest()
      .then((read) => {
        if (cancelled) {
          return;
        }

        setManifest(read);
        setBoard(read.boards[0]?.name ?? "");
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(normaliseError(cause).message);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setBusy(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [open]);

  const chosen: FirmwareBoard | undefined = manifest?.boards.find(
    (entry) => entry.name === board,
  );

  const chosenVersion = chosen?.versions.find(
    (entry) => entry.version === version,
  );

  const chosenFile = chosenVersion?.files.find(
    (entry) => entry.platform === platform,
  );

  // The version and the platform follow the vehicle, and the first of each is the one
  // that is wanted.
  useEffect(() => {
    if (chosen === undefined) {
      return;
    }

    setVersion((before) =>
      chosen.versions.some((entry) => entry.version === before)
        ? before
        : (chosen.versions[0]?.version ?? ""),
    );
  }, [chosen]);

  useEffect(() => {
    if (chosenVersion === undefined) {
      return;
    }

    setPlatform((before) =>
      chosenVersion.files.some((entry) => entry.platform === before)
        ? before
        : (chosenVersion.files[0]?.platform ?? ""),
    );
  }, [chosenVersion]);

  if (!open) {
    return null;
  }

  const fetchFile = async () => {
    if (chosenVersion === undefined || chosenFile === undefined) {
      setError(t("there is no file chosen."));
      return;
    }

    const name = `${chosen?.name ?? "firmware"}-${chosenVersion.version}-${chosenFile.platform}.${chosenFile.format || "bin"}`;

    const picked = await pickSavePath(name, [
      {
        name: t("The file of the firmware"),
        extensions: [chosenFile.format || "bin"],
      },
    ]);

    if (!picked.ok) {
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const length = await firmwareDownload(chosenFile.url, picked.path);
      setFile(picked.path);
      setNotice(
        t("The file was written to {path} ({size}).", {
          path: picked.path,
          size: showSize(length),
        }),
      );
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const eraseBoard = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const info = await firmwareEraseBoard(port);
      setBoardInfo(info);
      setNotice(t("The flash of the board was erased."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const writeToBoard = async () => {
    if (file === null) {
      setError(t("there is no file to write."));
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const info = await firmwareWriteToBoard(port, file, allowSame);
      setBoardInfo(info);
      setNotice(t("The board was written and told to start again."));
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const writeBootloaderSector = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      await flashBootloader();
      setNotice(
        t(
          "The vehicle wrote the bootloader it carries in itself into the sector of its flash. It did not restart, and it is still speaking as a vehicle.",
        ),
      );
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const restartIntoBootloader = async () => {
    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      // What is read is what the vehicle answered, as the vehicle said it. The answer may not arrive:
      // the device goes away at the moment of the restart, and the port then carries the protocol of the
      // loader rather than the MAVLink of a flight.
      const answer = await rebootIntoBootloader();
      setNotice(
        t("the vehicle answered {result} to {command}", {
          result: answer.result,
          command: answer.command,
        }),
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
        aria-label={t("Firmware")}
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>{t("Firmware")}</h2>
          <button type="button" className="button" onClick={hide}>
            {t("Close")}
          </button>
        </div>

        <section className={styles.block}>
          <h3 className={styles.heading}>{t("The vehicle")}</h3>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Vehicle")}</span>
              <select
                className="select"
                value={board}
                aria-label={t("The vehicle the firmware is for")}
                onChange={(event) => {
                  setBoard(event.target.value);
                }}
              >
                {(manifest?.boards ?? []).map((entry) => (
                  <option key={entry.name} value={entry.name}>
                    {entry.name}
                  </option>
                ))}
              </select>
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Version")}</span>
              <select
                className="select"
                value={version}
                aria-label={t("The version of the firmware")}
                onChange={(event) => {
                  setVersion(event.target.value);
                }}
              >
                {(chosen?.versions ?? []).map((entry) => (
                  <option key={entry.version} value={entry.version}>
                    {entry.version}
                  </option>
                ))}
              </select>
            </label>

            <label className={styles.field}>
              <span className={styles.label}>{t("Board")}</span>
              <select
                className="select"
                value={platform}
                aria-label={t("The board the firmware is for")}
                onChange={(event) => {
                  setPlatform(event.target.value);
                }}
              >
                {(chosenVersion?.files ?? []).map((entry) => (
                  <option key={entry.platform} value={entry.platform}>
                    {entry.platform}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {chosenFile !== undefined ? (
            <dl className={styles.facts}>
              <dt>{t("The file")}</dt>
              <dd>{chosenFile.url}</dd>
              <dt>{t("The board the document says it is for")}</dt>
              <dd>{formatCount(chosenFile.boardId)}</dd>
              <dt>{t("How big")}</dt>
              <dd>{showSize(chosenFile.size)}</dd>
              <dt>{t("Its hash")}</dt>
              <dd className={styles.hash}>{formatText(chosenFile.sha256)}</dd>
            </dl>
          ) : null}

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || chosenFile === undefined}
              onClick={() => {
                void fetchFile();
              }}
            >
              {t("Fetch the file")}
            </button>
          </div>

          <p className={styles.text}>
            {t(
              "The file is fetched to the disk. It can then be written to a board that is in its bootloader: the board answers the questions of the bootloader instead of the MAVLink of a flight, and the file is written to it block by block.",
            )}
          </p>
        </section>

        <section className={styles.block}>
          <h3 className={styles.heading}>{t("Writing it to a board")}</h3>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              title={t(
                "Write the bootloader the vehicle carries in itself into the sector of its flash",
              )}
              onClick={() => {
                void writeBootloaderSector();
              }}
            >
              {t("Write the bootloader to the vehicle")}
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              title={t("Ask the vehicle to restart into its bootloader")}
              onClick={() => {
                void restartIntoBootloader();
              }}
            >
              {t("Restart the vehicle into its bootloader")}
            </button>
          </div>

          <p className={styles.text}>
            {t(
              "These are two different things. The first writes the sector of the loader that the vehicle carries in itself, and the board goes on speaking as a vehicle after answering. The second asks the board to restart and stay in its loader, and from there the port carries the questions of the loader rather than the MAVLink of a flight, which is what writing a file needs.",
            )}
          </p>

          <p className={styles.text}>
            {t(
              "The vehicle is asked to restart into its bootloader. It stops speaking MAVLink when it is there, and the port is then opened in the manner of the bootloader.",
            )}
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>{t("Port")}</span>
              <select
                className="select"
                value={port}
                aria-label={t("The port the board is on")}
                onChange={(event) => {
                  setPort(event.target.value);
                }}
              >
                {ports.length === 0 ? (
                  <option value="">{t("no ports")}</option>
                ) : null}
                {ports.map((entry) => (
                  <option key={entry.device} value={entry.device}>
                    {entry.device}
                    {entry.description ? ` — ${entry.description}` : ""}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <label className={styles.check}>
            <input
              type="checkbox"
              checked={allowSame}
              aria-label={t(
                "Write it again when the board already holds this image",
              )}
              onChange={(event) => {
                setAllowSame(event.target.checked);
              }}
            />
            <span>
              {t("Write it again when the board already holds this image")}
            </span>
          </label>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || port === "" || file === null}
              onClick={() => {
                void writeToBoard();
              }}
            >
              {t("Write it to the board")}
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || port === ""}
              title={t("Leave the board without a firmware")}
              onClick={() => {
                void eraseBoard();
              }}
            >
              {t("Erase the board")}
            </button>
          </div>

          <p className={styles.text}>
            {t(
              "The board has to be in its bootloader for this to work: the link has to be closed, the board has to be restarted into the bootloader, and the file is written over the port. The board is asked what it is before anything is erased, and the file is not written if it does not fit in the flash.",
            )}
          </p>

          <p className={styles.text}>
            {t(
              "A board that says its flash already holds this image is not erased for nothing, and what was written is read back from the board afterwards rather than assumed from the send.",
            )}
          </p>

          {boardInfo !== null ? (
            <p className={styles.text}>
              {t(
                "The board is the {board} (revision {revision}), with {kilobytes} kB of flash, speaking the protocol {protocol}.",
                {
                  board: boardInfo.boardId,
                  revision: boardInfo.boardRev,
                  kilobytes: Math.round(boardInfo.flashSize / 1024),
                  protocol: boardInfo.blRev,
                },
              )}
            </p>
          ) : null}
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
