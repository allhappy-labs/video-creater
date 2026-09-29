import { useEffect, useState } from "react";
import { backendListen } from "@/lib/runtime/backend-client";
import type { BackendUnlisten } from "@/lib/runtime/backend-transport";
import {
  listSettingsOperations,
  settingsOperationEvent,
  type SettingsOperation,
} from "./operations";

const settingsOperationsRetryDelayMs = 1_000;

export interface SettingsOperationsSyncError {
  code:
    | "settings.operations.listenerFailed"
    | "settings.operations.pollFailed";
  message: string;
  detail: string;
}

export interface SettingsOperationsSnapshot {
  operations: SettingsOperation[];
  error: SettingsOperationsSyncError | null;
  hydrated: boolean;
  baselineOperationIds: ReadonlySet<string>;
  baselineTerminalOperationIds: ReadonlySet<string>;
}

const terminalOperationStates = new Set([
  "succeeded",
  "failed",
  "cancelled",
]);

function operationVersion(updatedAt: string): number {
  const parsed = Date.parse(updatedAt);
  return Number.isNaN(parsed) ? Number.NEGATIVE_INFINITY : parsed;
}

function replaceById(
  operations: SettingsOperation[],
  incoming: SettingsOperation,
): SettingsOperation[] {
  const index = operations.findIndex(({ id }) => id === incoming.id);
  if (index === -1) {
    return [...operations, incoming];
  }
  if (
    operationVersion(incoming.updatedAt) <=
    operationVersion(operations[index]?.updatedAt ?? "")
  ) {
    return operations;
  }

  const next = [...operations];
  next[index] = incoming;
  return next;
}

function mergeOperationSnapshots(
  current: SettingsOperation[],
  recovered: SettingsOperation[],
): SettingsOperation[] {
  return recovered.reduce<SettingsOperation[]>(
    (operations, operation) => replaceById(operations, operation),
    current,
  );
}

function errorDetail(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  if (
    typeof error === "object" &&
    error !== null &&
    "detail" in error &&
    typeof error.detail === "string"
  ) {
    return error.detail;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function pollSyncError(error: unknown): SettingsOperationsSyncError {
  return {
    code: "settings.operations.pollFailed",
    message: "Settings operation recovery is temporarily unavailable.",
    detail: errorDetail(error),
  };
}

function listenerSyncError(error: unknown): SettingsOperationsSyncError {
  return {
    code: "settings.operations.listenerFailed",
    message: "Live settings operation updates are temporarily unavailable.",
    detail: errorDetail(error),
  };
}

export function useSettingsOperations(): SettingsOperationsSnapshot {
  const [operations, setOperations] = useState<SettingsOperation[]>([]);
  const [error, setError] = useState<SettingsOperationsSyncError | null>(null);
  const [hydrated, setHydrated] = useState(false);
  const [baselineOperationIds, setBaselineOperationIds] =
    useState<ReadonlySet<string>>(() => new Set());
  const [baselineTerminalOperationIds, setBaselineTerminalOperationIds] =
    useState<ReadonlySet<string>>(() => new Set());

  useEffect(() => {
    let active = true;
    let unlisten: BackendUnlisten | undefined;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;
    let listenerReady = false;
    let baselineCaptured = false;
    const liveOperationIdsBeforeBaseline = new Set<string>();
    let pollInFlight = false;
    let listenerInFlight = false;
    let pollError: SettingsOperationsSyncError | null = null;
    let listenerError: SettingsOperationsSyncError | null = null;

    const publishError = () => {
      if (active) {
        setError(listenerError ?? pollError);
      }
    };

    const poll = async (): Promise<boolean> => {
      if (!active || pollInFlight) {
        return false;
      }
      pollInFlight = true;
      try {
        const recovered = await listSettingsOperations();
        if (!active) {
          return false;
        }
        setOperations((current) =>
          mergeOperationSnapshots(current, recovered),
        );
        if (!baselineCaptured) {
          baselineCaptured = true;
          const retained = recovered.filter(
            ({ id }) => !liveOperationIdsBeforeBaseline.has(id),
          );
          setBaselineOperationIds(
            new Set(retained.map(({ id }) => id)),
          );
          setBaselineTerminalOperationIds(
            new Set(
              retained
                .filter(({ state }) => terminalOperationStates.has(state))
                .map(({ id }) => id),
            ),
          );
        }
        pollError = null;
        publishError();
        if (listenerReady) {
          setHydrated(true);
        }
        return true;
      } catch (pollFailure) {
        if (!active) {
          return false;
        }
        pollError = pollSyncError(pollFailure);
        publishError();
        return false;
      } finally {
        pollInFlight = false;
      }
    };

    const registerListener = async () => {
      if (!active || listenerReady || listenerInFlight) {
        return;
      }
      listenerInFlight = true;
      try {
        const removeListener = await backendListen<SettingsOperation>(
          settingsOperationEvent,
          (payload) => {
            if (!active) {
              return;
            }
            if (!baselineCaptured) {
              liveOperationIdsBeforeBaseline.add(payload.id);
            }
            setOperations((current) => replaceById(current, payload));
          },
        );
        if (!active) {
          removeListener();
          return;
        }
        unlisten = removeListener;
        listenerReady = true;
        listenerError = null;
        publishError();
        await poll();
      } catch (listenerFailure) {
        if (!active) {
          return;
        }
        listenerError = listenerSyncError(listenerFailure);
        publishError();
      } finally {
        listenerInFlight = false;
      }
    };

    const scheduleRetry = () => {
      if (!active || retryTimer !== undefined) {
        return;
      }
      retryTimer = setTimeout(() => {
        retryTimer = undefined;
        void recover();
      }, settingsOperationsRetryDelayMs);
    };

    const recover = async () => {
      await poll();
      await registerListener();
      if (active && (!listenerReady || pollError !== null)) {
        scheduleRetry();
      }
    };

    void recover();

    return () => {
      active = false;
      if (retryTimer !== undefined) {
        clearTimeout(retryTimer);
      }
      unlisten?.();
    };
  }, []);

  return {
    operations,
    error,
    hydrated,
    baselineOperationIds,
    baselineTerminalOperationIds,
  };
}
