import type { TaskKind, TaskRecord } from "@/lib/jobs/task-records";

/** How long the indicator stays after the newest task finished. */
const recentWindowMs = 10 * 60 * 1000;

type TaskIndicatorState =
  | { readonly visible: false }
  | {
      readonly visible: true;
      /** `running` shows a spinner; `failed` a failed dot; `completed` a finished pill. */
      readonly tone: "running" | "failed" | "completed";
      /** The task the pill describes: the most important active task, or the newest finished one. */
      readonly record: TaskRecord;
      readonly label: string;
      /** True when the newest task (by `updatedAt`) failed. */
      readonly newestFailed: boolean;
      readonly activeCount: number;
    };

const activeVerb: Record<TaskKind, string> = {
  transcription: "Transcribing",
  analysis: "Analyzing",
  generation: "Generating",
  render: "Rendering",
  export: "Exporting",
  agent: "Editing",
};

const taskNoun: Record<TaskKind, string> = {
  transcription: "Transcription",
  analysis: "Analysis",
  generation: "Generation",
  render: "Render",
  export: "Export",
  agent: "Agent edit",
};

function updatedAtMs(record: TaskRecord): number {
  const timestamp = Date.parse(record.updatedAt);
  return Number.isFinite(timestamp) ? timestamp : Number.NEGATIVE_INFINITY;
}

function isActive(record: TaskRecord): boolean {
  return record.status === "running" || record.status === "queued" || record.status === "blocked";
}

/** Running before queued (and blocked), then the oldest first. */
function importance(left: TaskRecord, right: TaskRecord): number {
  const rank = (record: TaskRecord) => (record.status === "running" ? 0 : 1);
  return rank(left) - rank(right) || updatedAtMs(left) - updatedAtMs(right);
}

function activeLabel(record: TaskRecord): string {
  if (record.status !== "running") return `${taskNoun[record.kind]} queued`;
  if (record.progress === null) return `${activeVerb[record.kind]}…`;
  const percent = Math.round(Math.min(1, Math.max(0, record.progress)) * 100);
  return `${activeVerb[record.kind]} · ${percent}%`;
}

function finishedLabel(record: TaskRecord): string {
  const noun = taskNoun[record.kind];
  if (record.status === "failed") return `${noun} failed`;
  if (record.status === "cancelled") return `${noun} cancelled`;
  return `${noun} complete`;
}

/**
 * The top-bar tasks pill. Active tasks show the most important one ("Transcribing · 62%" with
 * progress in 0–1, "Exporting…" without). With nothing active, the newest finished task shows for
 * 10 minutes after its `updatedAt` (failed or completed tone); after that, and when there are no
 * tasks, the pill is hidden.
 */
export function indicatorState(records: readonly TaskRecord[], now: number): TaskIndicatorState {
  const newest = records.reduce<TaskRecord | null>((latest, record) => (!latest || updatedAtMs(record) > updatedAtMs(latest) ? record : latest), null);
  if (!newest) return { visible: false };
  const newestFailed = newest.status === "failed";
  const active = records.filter(isActive).sort(importance);
  const [first] = active;
  if (first) {
    return { visible: true, tone: "running", record: first, label: activeLabel(first), newestFailed, activeCount: active.length };
  }
  if (now - updatedAtMs(newest) > recentWindowMs) return { visible: false };
  return { visible: true, tone: newestFailed ? "failed" : "completed", record: newest, label: finishedLabel(newest), newestFailed, activeCount: 0 };
}
