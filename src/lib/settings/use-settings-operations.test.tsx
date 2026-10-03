import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { requiredAt } from "../../test-utils/required";
import type { SettingsOperation } from "./operations";
import { useSettingsOperations } from "./use-settings-operations";

const { listSettingsOperationsMock, listenMock } = vi.hoisted(() => ({
  listSettingsOperationsMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("./operations", async () => {
  const actual = await vi.importActual<typeof import("./operations")>("./operations");
  return {
    ...actual,
    listSettingsOperations: listSettingsOperationsMock,
  };
});

vi.mock("@/lib/runtime/backend-client", () => ({
  backendListen: listenMock,
}));

function operation(
  id: string,
  updatedAt: string,
  overrides: Partial<SettingsOperation> = {},
): SettingsOperation {
  return {
    id,
    kind: "modelDownload",
    targetId: "nvidia/parakeet-tdt-0.6b-v3",
    phase: "queued",
    state: "queued",
    completedUnits: 0,
    totalUnits: 485_000_000,
    unit: "bytes",
    cancellable: true,
    message: "Queued.",
    error: null,
    startedAt: "2026-07-16T10:00:00Z",
    updatedAt,
    ...overrides,
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

describe("useSettingsOperations", () => {
  beforeEach(() => {
    listSettingsOperationsMock.mockReset();
    listenMock.mockReset();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("recovers by polling first, then replaces event snapshots by ID without reordering", async () => {
    const initial = [
      operation("operation-1", "2026-07-16T10:00:00Z"),
      operation("operation-2", "2026-07-16T10:00:01Z"),
    ];
    const unlisten = vi.fn();
    let onEvent: (payload: SettingsOperation) => void = () => {};
    listSettingsOperationsMock.mockResolvedValue(initial);
    listenMock.mockImplementation(
      async (
        _eventName: string,
        next: (payload: SettingsOperation) => void,
      ) => {
        onEvent = next;
        return unlisten;
      },
    );

    const { result } = renderHook(() => useSettingsOperations());

    expect(result.current.hydrated).toBe(false);
    await waitFor(() => expect(result.current.operations).toEqual(initial));
    await waitFor(() => expect(result.current.hydrated).toBe(true));
    expect(requiredAt(
      listSettingsOperationsMock.mock.invocationCallOrder,
      0,
      "settings list invocation order",
    )).toBeLessThan(
      requiredAt(listenMock.mock.invocationCallOrder, 0, "settings listener invocation order"),
    );

    act(() => {
      onEvent(
        operation("operation-1", "2026-07-16T10:00:02Z", {
          phase: "downloading",
          state: "running",
          completedUnits: 1024,
          message: "Downloading model files.",
        }),
      );
      onEvent(operation("operation-3", "2026-07-16T10:00:03Z"));
    });

    expect(result.current.operations.map(({ id }) => id)).toEqual([
      "operation-1",
      "operation-2",
      "operation-3",
    ]);
    expect(result.current.operations[0]).toMatchObject({
      state: "running",
      completedUnits: 1024,
    });
  });

  it("closes the poll-listen gap with a recovery poll after listener registration", async () => {
    const listener = deferred<() => void>();
    const stale = operation("operation-1", "2026-07-16T10:00:00Z");
    const current = operation("operation-1", "2026-07-16T10:00:02Z", {
      phase: "downloading",
      state: "running",
      completedUnits: 2048,
    });
    listSettingsOperationsMock
      .mockResolvedValueOnce([stale])
      .mockResolvedValueOnce([current]);
    listenMock.mockImplementation(
      () => listener.promise,
    );

    const { result } = renderHook(() => useSettingsOperations());
    await waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    await act(async () => {
      listener.resolve(vi.fn<() => void>());
      await listener.promise;
    });

    await waitFor(() => expect(listSettingsOperationsMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(result.current.operations).toEqual([current]));
  });

  it("captures baseline IDs only from the initial retained-history poll", async () => {
    const listener = deferred<() => void>();
    const gapPoll = deferred<SettingsOperation[]>();
    const retained = operation("operation-retained", "2026-07-16T10:00:00Z", {
      state: "succeeded",
    });
    const live = operation("operation-live", "2026-07-16T10:00:01Z", {
      state: "succeeded",
    });
    let onEvent: (payload: SettingsOperation) => void = () => {};
    listSettingsOperationsMock
      .mockResolvedValueOnce([retained])
      .mockReturnValueOnce(gapPoll.promise);
    listenMock.mockImplementation(
      (
        _eventName: string,
        next: (payload: SettingsOperation) => void,
      ) => {
        onEvent = next;
        return listener.promise;
      },
    );

    const { result } = renderHook(() => useSettingsOperations());
    await waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));

    act(() => {
      onEvent(live);
    });
    expect(result.current.operations).toEqual([retained, live]);

    await act(async () => {
      listener.resolve(vi.fn<() => void>());
      await listener.promise;
      gapPoll.resolve([retained, live]);
      await gapPoll.promise;
    });

    await waitFor(() => expect(result.current.hydrated).toBe(true));
    expect([...result.current.baselineOperationIds]).toEqual([
      "operation-retained",
    ]);
    expect(result.current.baselineOperationIds.has("operation-live")).toBe(
      false,
    );
  });

  it("keeps baseline capture pending after poll failure and excludes live event IDs", async () => {
    const recoveryPoll = deferred<SettingsOperation[]>();
    const retained = operation("operation-retained", "2026-07-16T10:00:00Z", {
      state: "succeeded",
    });
    const live = operation("operation-live", "2026-07-16T10:00:01Z", {
      state: "succeeded",
    });
    let onEvent: (payload: SettingsOperation) => void = () => {};
    listSettingsOperationsMock
      .mockRejectedValueOnce(new Error("registry starting"))
      .mockReturnValueOnce(recoveryPoll.promise);
    listenMock.mockImplementation(
      async (
        _eventName: string,
        next: (payload: SettingsOperation) => void,
      ) => {
        onEvent = next;
        return vi.fn();
      },
    );

    const { result } = renderHook(() => useSettingsOperations());
    await waitFor(() => expect(listSettingsOperationsMock).toHaveBeenCalledTimes(2));

    act(() => {
      onEvent(live);
    });
    recoveryPoll.resolve([retained, live]);

    await waitFor(() => expect(result.current.hydrated).toBe(true));
    expect([...result.current.baselineOperationIds]).toEqual([
      "operation-retained",
    ]);
    expect([...result.current.baselineTerminalOperationIds]).toEqual([
      "operation-retained",
    ]);
    expect(result.current.operations.map(({ id }) => id)).toEqual([
      "operation-live",
      "operation-retained",
    ]);
  });

  it("preserves existing row order when a recovery poll returns the same IDs reordered", async () => {
    const initial = [
      operation("operation-a", "2026-07-16T10:00:00Z"),
      operation("operation-b", "2026-07-16T10:00:01Z"),
    ];
    const reordered = [
      operation("operation-b", "2026-07-16T10:00:03Z", {
        state: "running",
      }),
      operation("operation-a", "2026-07-16T10:00:02Z", {
        state: "running",
      }),
    ];
    listSettingsOperationsMock
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(reordered);
    listenMock.mockResolvedValue(vi.fn());

    const { result } = renderHook(() => useSettingsOperations());

    await waitFor(() => expect(listSettingsOperationsMock).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(result.current.operations.map(({ id }) => id)).toEqual([
        "operation-a",
        "operation-b",
      ]),
    );
    expect(result.current.operations.every(({ state }) => state === "running")).toBe(
      true,
    );
  });

  it("rejects an equal-timestamp event so a late payload cannot regress an operation", async () => {
    const current = operation("operation-1", "2026-07-16T10:00:02Z", {
      state: "running",
      completedUnits: 2048,
    });
    let onEvent: (payload: SettingsOperation) => void = () => {};
    listSettingsOperationsMock.mockResolvedValue([current]);
    listenMock.mockImplementation(
      async (
        _eventName: string,
        next: (payload: SettingsOperation) => void,
      ) => {
        onEvent = next;
        return vi.fn();
      },
    );
    const { result } = renderHook(() => useSettingsOperations());
    await waitFor(() => expect(result.current.operations).toEqual([current]));

    act(() => {
      onEvent(
        operation("operation-1", current.updatedAt, {
          state: "queued",
          completedUnits: 0,
        }),
      );
    });

    expect(result.current.operations).toEqual([current]);
  });

  it("surfaces listener failure and retries registration until live sync succeeds", async () => {
    vi.useFakeTimers();
    const unlisten = vi.fn();
    listSettingsOperationsMock.mockResolvedValue([]);
    listenMock
      .mockRejectedValueOnce(new Error("event bridge unavailable"))
      .mockResolvedValueOnce(unlisten);

    const { result } = renderHook(() => useSettingsOperations());
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.error).toEqual({
      code: "settings.operations.listenerFailed",
      message: "Live settings operation updates are temporarily unavailable.",
      detail: "event bridge unavailable",
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });

    expect(listenMock).toHaveBeenCalledTimes(2);
    expect(result.current.error).toBeNull();
  });

  it("cancels a pending retry timer on cleanup", async () => {
    vi.useFakeTimers();
    listSettingsOperationsMock.mockResolvedValue([]);
    listenMock.mockRejectedValue(new Error("event bridge unavailable"));

    const { unmount } = renderHook(() => useSettingsOperations());
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(listenMock).toHaveBeenCalledTimes(1);

    unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2_000);
    });

    expect(listenMock).toHaveBeenCalledTimes(1);
  });

  it("surfaces poll failure, retains current data, and retries fallback polling", async () => {
    vi.useFakeTimers();
    const recovered = operation("operation-1", "2026-07-16T10:00:02Z");
    listSettingsOperationsMock
      .mockRejectedValueOnce(new Error("registry unavailable"))
      .mockRejectedValueOnce(new Error("registry still unavailable"))
      .mockResolvedValueOnce([recovered]);
    listenMock.mockResolvedValue(vi.fn());

    const { result } = renderHook(() => useSettingsOperations());
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });

    expect(result.current.error).toEqual({
      code: "settings.operations.pollFailed",
      message: "Settings operation recovery is temporarily unavailable.",
      detail: "registry still unavailable",
    });
    expect(result.current.operations).toEqual([]);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1_000);
    });

    expect(result.current.operations).toEqual([recovered]);
    expect(result.current.error).toBeNull();
  });

  it("unlistens after cleanup and ignores late poll and event results", async () => {
    const poll = deferred<SettingsOperation[]>();
    const listener = deferred<() => void>();
    let onEvent: (payload: SettingsOperation) => void = () => {};
    const unlisten = vi.fn();
    listSettingsOperationsMock.mockReturnValue(poll.promise);
    listenMock.mockImplementation(
      (
        _eventName: string,
        next: (payload: SettingsOperation) => void,
      ) => {
        onEvent = next;
        return listener.promise;
      },
    );

    const { result, unmount } = renderHook(() => useSettingsOperations());
    poll.resolve([]);
    await waitFor(() => expect(listenMock).toHaveBeenCalledTimes(1));
    unmount();

    await act(async () => {
      listener.resolve(unlisten);
      await Promise.all([listener.promise, poll.promise]);
      onEvent(operation("operation-2", "2026-07-16T10:00:01Z"));
    });

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(result.current.operations).toEqual([]);
    expect(listSettingsOperationsMock).toHaveBeenCalledTimes(1);
  });
});
