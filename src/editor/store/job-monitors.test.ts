import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { JobProgressSnapshot, TemporalJobReconciliation } from "@/lib/project";
import { BackendUnavailableError, FixtureOperationUnsupportedError } from "@/lib/runtime/backend-transport";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { createProgressMonitor, createTemporalReconciler } from "./job-monitors";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

const snapshot: JobProgressSnapshot = { jobId: "export-1", progress: 0.4, updatedAt: "2026-09-16T10:00:00.000Z" };

describe("createProgressMonitor", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("polls every 500 ms while a task runs and stops when none does", async () => {
    let running = true;
    const load = vi.fn(async () => [snapshot]);
    const onSnapshots = vi.fn();
    const monitor = createProgressMonitor({ load, hasRunningTask: () => running, onSnapshots });
    monitor.start();

    await vi.advanceTimersByTimeAsync(499);
    expect(load).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(1);
    expect(load).toHaveBeenCalledTimes(1);
    expect(onSnapshots).toHaveBeenLastCalledWith([snapshot]);
    await vi.advanceTimersByTimeAsync(1_000);
    expect(load).toHaveBeenCalledTimes(3);

    running = false;
    monitor.sync();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(load).toHaveBeenCalledTimes(3);
    monitor.stop();
  });

  it("never overlaps an in-flight poll and keeps polling while another reload is pending", async () => {
    const pending = deferred<JobProgressSnapshot[]>();
    const load = vi.fn(() => pending.promise);
    const folderReload = new Promise(() => undefined);
    const monitor = createProgressMonitor({ load, hasRunningTask: () => true, onSnapshots: () => undefined });
    void folderReload;
    monitor.start();

    await vi.advanceTimersByTimeAsync(500);
    expect(load).toHaveBeenCalledTimes(1);
    monitor.sync();
    await vi.advanceTimersByTimeAsync(2_000);
    expect(load).toHaveBeenCalledTimes(1);

    pending.resolve([snapshot]);
    await vi.advanceTimersByTimeAsync(500);
    expect(load).toHaveBeenCalledTimes(2);
    monitor.stop();
  });

  it.each([new FixtureOperationUnsupportedError("load_job_progress_from_split_project_folder"), new BackendUnavailableError()])(
    "disables itself when the backend can't load progress (%s)",
    async (error) => {
      const load = vi.fn(async () => {
        throw error;
      });
      const monitor = createProgressMonitor({ load, hasRunningTask: () => true, onSnapshots: () => undefined });
      monitor.start();

      await vi.advanceTimersByTimeAsync(500);
      monitor.sync();
      await vi.advanceTimersByTimeAsync(5_000);
      expect(load).toHaveBeenCalledTimes(1);
      monitor.stop();
    },
  );
});

describe("createTemporalReconciler", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  const reachable: TemporalJobReconciliation = { project: fixtureProject(), failedJobIds: ["export-1"], serviceReachable: true, detail: null };
  const unreachable: TemporalJobReconciliation = { project: null, failedJobIds: [], serviceReachable: false, detail: "Workflow service unreachable" };

  it("runs once on start and every 30 s while a Temporal-backed task is active", async () => {
    let active = true;
    const reconcile = vi.fn(async () => reachable);
    const onResult = vi.fn();
    const reconciler = createTemporalReconciler({ reconcile, hasActiveTemporalTask: () => active, onResult });
    reconciler.start();

    await vi.advanceTimersByTimeAsync(0);
    expect(reconcile).toHaveBeenCalledTimes(1);
    expect(onResult).toHaveBeenLastCalledWith(reachable);
    await vi.advanceTimersByTimeAsync(29_999);
    expect(reconcile).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(reconcile).toHaveBeenCalledTimes(2);

    active = false;
    reconciler.sync();
    await vi.advanceTimersByTimeAsync(60_000);
    expect(reconcile).toHaveBeenCalledTimes(2);
    reconciler.stop();
  });

  it("passes reachability, detail and a null project to its callback", async () => {
    const onResult = vi.fn();
    const reconciler = createTemporalReconciler({ reconcile: async () => unreachable, hasActiveTemporalTask: () => false, onResult });
    reconciler.start();

    await vi.advanceTimersByTimeAsync(0);
    expect(onResult).toHaveBeenCalledWith({ project: null, failedJobIds: [], serviceReachable: false, detail: "Workflow service unreachable" });
    reconciler.stop();
  });

  it("never runs two reconciliations at once", async () => {
    const pending = deferred<TemporalJobReconciliation>();
    const reconcile = vi.fn(() => pending.promise);
    const reconciler = createTemporalReconciler({ reconcile, hasActiveTemporalTask: () => true, onResult: () => undefined });
    reconciler.start();

    await vi.advanceTimersByTimeAsync(90_000);
    reconciler.start();
    reconciler.sync();
    expect(reconcile).toHaveBeenCalledTimes(1);

    pending.resolve(reachable);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(reconcile).toHaveBeenCalledTimes(2);
    reconciler.stop();
  });

  it("disables itself when the backend has no reconciliation command", async () => {
    const reconcile = vi.fn(async () => {
      throw new FixtureOperationUnsupportedError("reconcile_temporal_jobs_in_split_project_folder");
    });
    const reconciler = createTemporalReconciler({ reconcile, hasActiveTemporalTask: () => true, onResult: () => undefined });
    reconciler.start();

    await vi.advanceTimersByTimeAsync(120_000);
    reconciler.sync();
    await vi.advanceTimersByTimeAsync(120_000);
    expect(reconcile).toHaveBeenCalledTimes(1);
    reconciler.stop();
  });
});
