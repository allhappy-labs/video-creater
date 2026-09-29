import { describe, expect, it } from "vitest";
import { indicatorState } from "@/lib/jobs/task-indicator";
import type { TaskRecord } from "@/lib/jobs/task-records";

const now = Date.parse("2026-09-15T12:00:00Z");

function record(id: string, overrides: Partial<TaskRecord> = {}): TaskRecord {
  return {
    id,
    kind: "export",
    label: id,
    status: "completed",
    progress: null,
    detail: null,
    failureReason: null,
    artifactPath: null,
    logPath: null,
    cancel: { available: false, reason: "This task is no longer running." },
    retry: false,
    updatedAt: "2026-09-15T11:59:00Z",
    workflow: null,
    ...overrides,
  };
}

function minutesAgo(minutes: number): string {
  return new Date(now - minutes * 60_000).toISOString();
}

describe("indicatorState", () => {
  it("is hidden without tasks", () => {
    expect(indicatorState([], now)).toEqual({ visible: false });
  });

  it("labels a running task with its progress percentage", () => {
    const transcription = record("transcribe", { kind: "transcription", status: "running", progress: 0.62 });

    expect(indicatorState([transcription], now)).toEqual({
      visible: true,
      tone: "running",
      record: transcription,
      label: "Transcribing · 62%",
      newestFailed: false,
      activeCount: 1,
    });
  });

  it("labels a running task without progress with an ellipsis", () => {
    expect(indicatorState([record("export", { status: "running" })], now)).toMatchObject({ label: "Exporting…" });
  });

  it("clamps progress to 0–100%", () => {
    expect(indicatorState([record("render", { kind: "render", status: "running", progress: 1.4 })], now)).toMatchObject({ label: "Rendering · 100%" });
  });

  it("prefers running over queued, then the oldest running task", () => {
    const queued = record("queued", { kind: "transcription", status: "queued", updatedAt: minutesAgo(30) });
    const newer = record("newer", { kind: "generation", status: "running", updatedAt: minutesAgo(1) });
    const older = record("older", { kind: "render", status: "running", updatedAt: minutesAgo(5) });

    expect(indicatorState([queued, newer, older], now)).toMatchObject({ record: older, label: "Rendering…", activeCount: 3 });
    expect(indicatorState([queued, record("blocked", { status: "blocked", updatedAt: minutesAgo(40) })], now)).toMatchObject({
      label: "Export queued",
      activeCount: 2,
    });
  });

  it("shows active tasks however old they are", () => {
    expect(indicatorState([record("agent", { kind: "agent", status: "running", updatedAt: minutesAgo(120) })], now)).toMatchObject({
      visible: true,
      label: "Editing…",
    });
  });

  it("flags a newest failed record while other tasks run", () => {
    const running = record("running", { status: "running", updatedAt: minutesAgo(3) });
    const failed = record("failed", { kind: "generation", status: "failed", updatedAt: minutesAgo(1) });

    expect(indicatorState([running, failed], now)).toMatchObject({ tone: "running", record: running, newestFailed: true });
  });

  it("shows the newest finished task for 10 minutes", () => {
    const completed = record("done", { updatedAt: minutesAgo(10) });
    const failed = record("failed", { kind: "transcription", status: "failed", updatedAt: minutesAgo(2) });
    const older = record("older", { updatedAt: minutesAgo(9) });

    expect(indicatorState([completed], now)).toMatchObject({ visible: true, tone: "completed", label: "Export complete", activeCount: 0 });
    expect(indicatorState([older, failed], now)).toMatchObject({ tone: "failed", record: failed, label: "Transcription failed", newestFailed: true });
    expect(indicatorState([record("cancelled", { kind: "render", status: "cancelled" })], now)).toMatchObject({ tone: "completed", label: "Render cancelled" });
  });

  it("hides once the newest finished task is more than 10 minutes old", () => {
    expect(indicatorState([record("done", { updatedAt: minutesAgo(10.01) })], now)).toEqual({ visible: false });
    expect(indicatorState([record("failed", { status: "failed", updatedAt: minutesAgo(30) })], now)).toEqual({ visible: false });
  });
});
