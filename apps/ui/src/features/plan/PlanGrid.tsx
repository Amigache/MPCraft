import {
  flexRender,
  getCoreRowModel,
  useReactTable,
  type ColumnDef,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useMemo, useRef } from "react";

import type { MavCommand, PlanItem } from "../../lib/ipc/types";
import {
  carriesPosition,
  commandOf,
  slotsOfPlan,
} from "../../lib/plan/planItems";
import { FRAMES } from "./planFrames";
import { PlanNumberCell } from "./PlanNumberCell";
import styles from "./PlanGrid.module.css";

/**
 * The grid of the items of a plan.
 *
 * The columns are the ones an operator reads a plan by: the place of the item, what it
 * does, the frame it is expressed in, the parameters its command uses, where it is, how
 * high it is and whether the vehicle goes on by itself.
 *
 * A plan can be thousands of items long — a survey is a grid of them — so the rows are
 * virtualised: only the rows that are on the screen are in the document, and the rest
 * are counted rather than drawn.
 */

/** How tall a row is, in pixels. */
const ROW_HEIGHT = 30;

/** How many decimals a coordinate is shown with. */
const COORDINATE_DIGITS = 7;

/** How many decimals an altitude is shown with. */
const ALTITUDE_DIGITS = 1;

/** What a change to an item is. */
export type ItemChange = (seq: number, patch: Partial<Omit<PlanItem, "seq">>) => void;

export interface PlanGridProps {
  /** The items of the plan, in order. */
  readonly items: readonly PlanItem[];
  /** The commands a plan can hold, and what their parameters mean. */
  readonly commands: readonly MavCommand[];
  /** The item the operator has selected. */
  readonly selected: number | null;
  /** Selects an item. */
  readonly onSelect: (seq: number) => void;
  /** Changes an item. */
  readonly onChange: ItemChange;
  /** Moves an item up or down the plan. */
  readonly onMove: (seq: number, delta: number) => void;
  /** Removes an item from the plan. */
  readonly onRemove: (seq: number) => void;
}

/** A coordinate, as it is shown. */
function showCoordinate(value: number): string {
  return value.toFixed(COORDINATE_DIGITS);
}

/** An altitude, as it is shown. */
function showAltitude(value: number): string {
  return value.toFixed(ALTITUDE_DIGITS);
}

/** A parameter, as it is shown. */
function showParameter(value: number): string {
  return String(Math.round(value * 1000) / 1000);
}

export function PlanGrid({
  items,
  commands,
  selected,
  onSelect,
  onChange,
  onMove,
  onRemove,
}: PlanGridProps) {
  const scroll = useRef<HTMLDivElement | null>(null);

  // The columns of the grid follow the plan: a plan of delays has one parameter column
  // and a plan of waypoints has another. Four empty boxes for a command that has no
  // parameters are noise.
  const slots = useMemo(
    () => slotsOfPlan(items, commands),
    [commands, items],
  );

  const columns = useMemo<ColumnDef<PlanItem, unknown>[]>(
    () => [
      {
        id: "seq",
        header: "#",
        size: 44,
        cell: ({ row }) => (
          <span
            className={row.original.current ? styles.current : styles.number}
            title={
              row.original.current
                ? "The item the vehicle is executing"
                : undefined
            }
          >
            {row.original.seq + 1}
          </span>
        ),
      },
      {
        id: "command",
        header: "Command",
        size: 152,
        cell: ({ row }) => (
          <CommandCell item={row.original} commands={commands} onChange={onChange} />
        ),
      },
      {
        id: "frame",
        header: "Frame",
        size: 116,
        cell: ({ row }) => (
          <select
            className={`select ${styles.select}`}
            value={row.original.frame}
            aria-label={`The frame of item ${row.original.seq + 1}`}
            onChange={(event) => {
              onChange(row.original.seq, { frame: Number(event.target.value) });
            }}
          >
            {FRAMES.map((frame) => (
              <option key={frame.value} value={frame.value}>
                {frame.label}
              </option>
            ))}
          </select>
        ),
      },
      ...slots.map<ColumnDef<PlanItem, unknown>>((slot) => ({
        id: `p${slot + 1}`,
        header: `P${slot + 1}`,
        size: 84,
        cell: ({ row }) => (
          <ParameterCell
            item={row.original}
            slot={slot}
            commands={commands}
            onChange={onChange}
          />
        ),
      })),
      {
        id: "lat",
        header: "Latitude",
        size: 100,
        cell: ({ row }) => (
          <PositionCell
            item={row.original}
            commands={commands}
            axis="lat"
            onChange={onChange}
          />
        ),
      },
      {
        id: "lon",
        header: "Longitude",
        size: 100,
        cell: ({ row }) => (
          <PositionCell
            item={row.original}
            commands={commands}
            axis="lon"
            onChange={onChange}
          />
        ),
      },
      {
        id: "alt",
        header: "Alt",
        size: 72,
        cell: ({ row }) => (
          <PositionCell
            item={row.original}
            commands={commands}
            axis="alt"
            onChange={onChange}
          />
        ),
      },
      {
        id: "autocontinue",
        header: "Auto",
        size: 48,
        cell: ({ row }) => (
          <input
            className={styles.check}
            type="checkbox"
            checked={row.original.autocontinue}
            aria-label={`Whether the vehicle goes on by itself after item ${
              row.original.seq + 1
            }`}
            onChange={(event) => {
              onChange(row.original.seq, { autocontinue: event.target.checked });
            }}
          />
        ),
      },
      {
        id: "actions",
        header: "",
        size: 92,
        cell: ({ row }) => (
          <div className={styles.actions}>
            <button
              type="button"
              className={styles.iconButton}
              title="Move the item up"
              aria-label={`Move item ${row.original.seq + 1} up`}
              disabled={row.original.seq === 0}
              onClick={() => {
                onMove(row.original.seq, -1);
              }}
            >
              ▲
            </button>
            <button
              type="button"
              className={styles.iconButton}
              title="Move the item down"
              aria-label={`Move item ${row.original.seq + 1} down`}
              disabled={row.original.seq === items.length - 1}
              onClick={() => {
                onMove(row.original.seq, 1);
              }}
            >
              ▼
            </button>
            <button
              type="button"
              className={`${styles.iconButton} ${styles.remove}`}
              title="Delete the item"
              aria-label={`Delete item ${row.original.seq + 1}`}
              onClick={() => {
                onRemove(row.original.seq);
              }}
            >
              ×
            </button>
          </div>
        ),
      },
    ],
    [commands, items.length, onChange, onMove, onRemove, slots],
  );

  const table = useReactTable({
    data: items as PlanItem[],
    columns,
    getCoreRowModel: getCoreRowModel(),
  });

  const rows = table.getRowModel().rows;

  const virtualiser = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  const total = table.getTotalSize();

  return (
    <div className={styles.scroll} ref={scroll}>
      <div
        className={styles.canvas}
        role="grid"
        aria-label="The items of the plan"
        aria-rowcount={rows.length + 1}
        aria-colcount={columns.length}
        style={{ width: `${total}px`, minWidth: "100%" }}
      >
        <div className={styles.head} role="row">
          {table.getHeaderGroups().map((group) =>
            group.headers.map((header) => (
              <div
                key={header.id}
                className={styles.headCell}
                role="columnheader"
                style={{ width: `${header.getSize()}px` }}
              >
                {flexRender(
                  header.column.columnDef.header,
                  header.getContext(),
                )}
              </div>
            )),
          )}
        </div>

        <div
          className={styles.body}
          role="rowgroup"
          style={{ height: `${virtualiser.getTotalSize()}px` }}
        >
          {virtualiser.getVirtualItems().map((virtual) => {
            const row = rows[virtual.index];
            if (row === undefined) {
              return null;
            }

            const isSelected = row.original.seq === selected;

            return (
              <div
                key={row.id}
                className={`${styles.row} ${isSelected ? styles.selected : ""} ${
                  row.original.current ? styles.executing : ""
                }`}
                role="row"
                aria-rowindex={virtual.index + 2}
                aria-selected={isSelected}
                style={{
                  height: `${virtual.size}px`,
                  transform: `translateY(${virtual.start}px)`,
                }}
                onClick={() => {
                  onSelect(row.original.seq);
                }}
                onFocusCapture={() => {
                  onSelect(row.original.seq);
                }}
              >
                {row.getVisibleCells().map((cell) => (
                  <div
                    key={cell.id}
                    className={styles.cell}
                    role="gridcell"
                    style={{ width: `${cell.column.getSize()}px` }}
                  >
                    {flexRender(cell.column.columnDef.cell, cell.getContext())}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/** The command of an item, as a list of the ones the ground station knows. */
function CommandCell({
  item,
  commands,
  onChange,
}: {
  readonly item: PlanItem;
  readonly commands: readonly MavCommand[];
  readonly onChange: ItemChange;
}) {
  const known = commands.some((command) => command.value === item.commandValue);

  return (
    <select
      className={`select ${styles.select}`}
      value={item.commandValue}
      aria-label={`The command of item ${item.seq + 1}`}
      onChange={(event) => {
        const value = Number(event.target.value);
        const command = commandOf(commands, value);
        if (command === undefined) {
          return;
        }
        onChange(item.seq, {
          command: command.canonicalName,
          commandValue: command.value,
        });
      }}
    >
      {known ? null : <option value={item.commandValue}>{item.command}</option>}
      {commands.map((command) => (
        <option key={command.value} value={command.value}>
          {command.name}
        </option>
      ))}
    </select>
  );
}

/** One of the generic parameters of an item. */
function ParameterCell({
  item,
  slot,
  commands,
  onChange,
}: {
  readonly item: PlanItem;
  readonly slot: number;
  readonly commands: readonly MavCommand[];
  readonly onChange: ItemChange;
}) {
  const command = commandOf(commands, item.commandValue);
  const role = command?.roles[slot] ?? "empty";

  // A slot the command does not use is left empty: a box for a value the command has no
  // use for is noise in a grid of a thousand rows.
  if (role === "empty") {
    return null;
  }

  const unit = command?.units[slot] ?? "";

  return (
    <>
      <PlanNumberCell
        value={item.params[slot] ?? 0}
        format={showParameter}
        label={`Parameter ${slot + 1} of item ${item.seq + 1}`}
        onCommit={(value) => {
          const params = [...item.params] as [number, number, number, number];
          params[slot] = value;
          onChange(item.seq, { params });
        }}
      />
      {unit ? <span className={styles.unit}>{unit}</span> : null}
    </>
  );
}

/** The position of an item: the latitude, the longitude or the altitude. */
function PositionCell({
  item,
  commands,
  axis,
  onChange,
}: {
  readonly item: PlanItem;
  readonly commands: readonly MavCommand[];
  readonly axis: "lat" | "lon" | "alt";
  readonly onChange: ItemChange;
}) {
  const command = commandOf(commands, item.commandValue);

  // A command that carries no position has no place to show one.
  if (!carriesPosition(command)) {
    return <span className={styles.absent}>—</span>;
  }

  if (axis === "alt") {
    return (
      <PlanNumberCell
        value={item.z}
        format={showAltitude}
        label={`The altitude of item ${item.seq + 1}`}
        onCommit={(value) => {
          onChange(item.seq, { z: value });
        }}
      />
    );
  }

  const isLatitude = axis === "lat";

  return (
    <PlanNumberCell
      value={isLatitude ? item.x : item.y}
      format={showCoordinate}
      label={`The ${isLatitude ? "latitude" : "longitude"} of item ${item.seq + 1}`}
      onCommit={(value) => {
        onChange(item.seq, isLatitude ? { x: value } : { y: value });
      }}
    />
  );
}
