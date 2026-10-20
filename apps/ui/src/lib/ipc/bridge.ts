import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import type {
  CommandError,
  ConnectRequest,
  ConnectResponse,
  ConnectionStatus,
  Telemetry,
  VehicleKey,
} from "./types";

export { listen };

/**
 * The single place where the frontend talks to the Rust backend.
 *
 * Every command goes through {@link call}, which normalises the error shape so components
 * handle a `code` rather than parsing strings.
 */
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (cause) {
    throw normaliseError(cause);
  }
}

/** Converts an unknown thrown value into the typed error shape. */
export function normaliseError(cause: unknown): CommandError {
  if (
    typeof cause === "object" &&
    cause !== null &&
    "code" in cause &&
    "message" in cause
  ) {
    const candidate = cause as { code: unknown; message: unknown };
    if (typeof candidate.code === "string" && typeof candidate.message === "string") {
      return { code: candidate.code, message: candidate.message };
    }
  }

  return {
    code: "internal",
    message: cause instanceof Error ? cause.message : String(cause),
  };
}

/** The connection state exposed to the UI. */
export interface ConnectionState {
  readonly status: ConnectionStatus;
  readonly error: CommandError | null;
  readonly connect: (target: string) => Promise<void>;
  readonly disconnect: () => Promise<void>;
}

/** Subscribes to the connection state and provides connect / disconnect actions. */
export function useConnection(): ConnectionState {
  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [error, setError] = useState<CommandError | null>(null);

  const connect = useCallback(async (target: string) => {
    setStatus("connecting");
    setError(null);
    try {
      const request: ConnectRequest = { target };
      const response = await call<ConnectResponse>("connect", { req: request });
      setStatus(response.status);
    } catch (cause) {
      setError(cause as CommandError);
      setStatus("failed");
    }
  }, []);

  const disconnect = useCallback(async () => {
    try {
      await call<ConnectResponse>("disconnect");
      setStatus("disconnected");
    } catch (cause) {
      setError(cause as CommandError);
    }
  }, []);

  useEffect(() => {
    const unlisten = listen<{ status: ConnectionStatus }>("link-state", (event) => {
      setStatus(event.payload.status);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  return { status, error, connect, disconnect };
}

/**
 * Subscribes to the batched telemetry stream.
 *
 * Telemetry arrives as one snapshot per tick, not one event per field. At 20 Hz with forty
 * fields, per-field events would mean eight hundred IPC messages per second.
 */
export function useTelemetry(
  vehicle: VehicleKey | null,
  onTelemetry: (telemetry: Telemetry) => void,
): void {
  useEffect(() => {
    if (!vehicle) {
      return;
    }

    const unlisten = listen<{ vehicle: VehicleKey; t: Telemetry }>(
      "telemetry",
      (event) => {
        if (
          event.payload.vehicle.systemId === vehicle.systemId &&
          event.payload.vehicle.componentId === vehicle.componentId
        ) {
          onTelemetry(event.payload.t);
        }
      },
    );

    return () => {
      void unlisten.then((off) => off());
    };
  }, [vehicle, onTelemetry]);
}
