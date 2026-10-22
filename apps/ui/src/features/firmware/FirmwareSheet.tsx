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
} from "../../lib/ipc/bridge";
import type {
  BoardInfo,
  FirmwareBoard,
  FirmwareManifest,
  SerialPort,
} from "../../lib/ipc/types";
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

  const [manifest, setManifest] = useState<FirmwareManifest | null>(null);
  const [board, setBoard] = useState("");
  const [version, setVersion] = useState("");
  const [platform, setPlatform] = useState("");

  const [ports, setPorts] = useState<readonly SerialPort[]>([]);
  const [port, setPort] = useState("");
  const [file, setFile] = useState<string | null>(null);
  const [boardInfo, setBoardInfo] = useState<BoardInfo | null>(null);

  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  // The bootloader is reached by asking the vehicle to restart into it, and that needs
  // a link to a vehicle.
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
      setError("there is no file chosen.");
      return;
    }

    const name = `${chosen?.name ?? "firmware"}-${chosenVersion.version}-${chosenFile.platform}.${chosenFile.format || "bin"}`;

    const picked = await pickSavePath(name, [
      {
        name: "The file of the firmware",
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
        `The file was written to ${picked.path} (${showSize(length)}).`,
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
      setNotice("The flash of the board was erased.");
    } catch (cause) {
      setError(normaliseError(cause).message);
    } finally {
      setBusy(false);
    }
  };

  const writeToBoard = async () => {
    if (file === null) {
      setError("there is no file to write.");
      return;
    }

    setBusy(true);
    setError(null);
    setNotice(null);

    try {
      const info = await firmwareWriteToBoard(port, file);
      setBoardInfo(info);
      setNotice("The board was written and told to start again.");
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
      await flashBootloader();
      setNotice(
        "The vehicle was told to restart into its bootloader. The link is closed " +
          "when the board stops answering.",
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
        aria-label="Firmware"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className={styles.header}>
          <h2 className={styles.title}>Firmware</h2>
          <button type="button" className="button" onClick={hide}>
            Close
          </button>
        </div>

        <section className={styles.block}>
          <h3 className={styles.heading}>The vehicle</h3>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>Vehicle</span>
              <select
                className="select"
                value={board}
                aria-label="The vehicle the firmware is for"
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
              <span className={styles.label}>Version</span>
              <select
                className="select"
                value={version}
                aria-label="The version of the firmware"
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
              <span className={styles.label}>Board</span>
              <select
                className="select"
                value={platform}
                aria-label="The board the firmware is for"
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
              <dt>The file</dt>
              <dd>{chosenFile.url}</dd>
              <dt>How big</dt>
              <dd>{showSize(chosenFile.size)}</dd>
              <dt>Its hash</dt>
              <dd className={styles.hash}>{chosenFile.sha256}</dd>
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
              Fetch the file
            </button>
          </div>

          <p className={styles.text}>
            The file is fetched to the disk. It can then be written to a board
            that is in its bootloader: the board answers the questions of the
            bootloader instead of the MAVLink of a flight, and the file is
            written to it block by block.
          </p>
        </section>

        <section className={styles.block}>
          <h3 className={styles.heading}>Writing it to a board</h3>

          <div className={styles.row}>
            <button
              type="button"
              className="button"
              disabled={busy || !linked}
              title="Ask the vehicle to restart into its bootloader"
              onClick={() => {
                void restartIntoBootloader();
              }}
            >
              Restart the vehicle into its bootloader
            </button>
          </div>

          <p className={styles.text}>
            The vehicle is asked to restart into its bootloader. It stops speaking
            MAVLink when it is there, and the port is then opened in the manner of
            the bootloader.
          </p>

          <div className={styles.fields}>
            <label className={styles.field}>
              <span className={styles.label}>Port</span>
              <select
                className="select"
                value={port}
                aria-label="The port the board is on"
                onChange={(event) => {
                  setPort(event.target.value);
                }}
              >
                {ports.length === 0 ? <option value="">no ports</option> : null}
                {ports.map((entry) => (
                  <option key={entry.device} value={entry.device}>
                    {entry.device}
                    {entry.description ? ` — ${entry.description}` : ""}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className={styles.row}>
            <button
              type="button"
              className="button button--primary"
              disabled={busy || port === "" || file === null}
              onClick={() => {
                void writeToBoard();
              }}
            >
              Write it to the board
            </button>

            <button
              type="button"
              className="button"
              disabled={busy || port === ""}
              title="Leave the board without a firmware"
              onClick={() => {
                void eraseBoard();
              }}
            >
              Erase the board
            </button>
          </div>

          <p className={styles.text}>
            The board has to be in its bootloader for this to work: the link has
            to be closed, the board has to be restarted into the bootloader, and
            the file is written over the port. The board is asked what it is
            before anything is erased, and the file is not written if it does
            not fit in the flash.
          </p>

          {boardInfo !== null ? (
            <p className={styles.text}>
              The board is the {boardInfo.boardId} (revision{" "}
              {boardInfo.boardRev}), with{" "}
              {Math.round(boardInfo.flashSize / 1024)} kB of flash, speaking the
              protocol {boardInfo.blRev}.
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
