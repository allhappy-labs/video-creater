import type { JobProgressSnapshot, TemporalJobReconciliation } from "@/lib/project";
import { FixtureOperationUnsupportedError, isBackendUnavailableError } from "@/lib/runtime/backend-transport";

const progressIntervalMs = 500;
const reconcileIntervalMs = 30_000;

/** A timer loop the jobs slice starts with polling and nudges with `sync` on every change. */
export interface JobMonitor {
  start(): void;
  stop(): void;
  /** Schedules the next run when one is due, or clears the timer when none is. */
  sync(): void;
}

/** Without a backend command (a browser or a fixture without it) the monitor stops for the session. */
function unsupported(error: unknown): boolean {
  return error instanceof FixtureOperationUnsupportedError || isBackendUnavailableError(error);
}

interface LoopOptions {
  readonly intervalMs: number;
  readonly due: () => boolean;
  readonly run: () => Promise<void>;
  readonly runOnStart: boolean;
}

function createLoop({ intervalMs, due, run, runOnStart }: LoopOptions): JobMonitor {
  let enabled = false;
  let inFlight = false;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function clearTimer(): void {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  }

  async function tick(): Promise<void> {
    timer = null;
    inFlight = true;
    try {
      await run();
    } catch (error) {
      if (unsupported(error)) enabled = false;
    } finally {
      inFlight = false;
      sync();
    }
  }

  function sync(): void {
    if (!enabled || !due()) {
      clearTimer();
      return;
    }
    if (inFlight || timer !== null) return;
    timer = setTimeout(() => void tick(), intervalMs);
  }

  return {
    start() {
      if (enabled) return;
      enabled = true;
      if (runOnStart && !inFlight) {
        void tick();
        return;
      }
      sync();
    },
    stop() {
      enabled = false;
      clearTimer();
    },
    sync,
  };
}

interface ProgressMonitorOptions {
  readonly load: () => Promise<readonly JobProgressSnapshot[]>;
  readonly hasRunningTask: () => boolean;
  readonly onSnapshots: (snapshots: readonly JobProgressSnapshot[]) => void;
}

/**
 * Polls lease-free progress snapshots every 500 ms while a task runs. It is separate from the folder
 * reload, which blocks for a render's whole duration, so progress keeps moving during the render.
 */
export function createProgressMonitor({ load, hasRunningTask, onSnapshots }: ProgressMonitorOptions): JobMonitor {
  return createLoop({
    intervalMs: progressIntervalMs,
    due: hasRunningTask,
    runOnStart: false,
    run: async () => onSnapshots(await load()),
  });
}

interface TemporalReconcilerOptions {
  readonly reconcile: () => Promise<TemporalJobReconciliation>;
  readonly hasActiveTemporalTask: () => boolean;
  readonly onResult: (result: TemporalJobReconciliation) => void | Promise<void>;
}

/**
 * Reconciles Temporal-backed jobs with their workflows once when the editor opens, then every 30 s
 * while a Temporal-backed task is active. One reconciliation runs at a time.
 */
export function createTemporalReconciler({ reconcile, hasActiveTemporalTask, onResult }: TemporalReconcilerOptions): JobMonitor {
  return createLoop({
    intervalMs: reconcileIntervalMs,
    due: hasActiveTemporalTask,
    runOnStart: true,
    run: async () => onResult(await reconcile()),
  });
}
