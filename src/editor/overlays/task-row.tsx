import { Bot, Captions, CircleAlert, Download, Film, FileText, FolderOpen, Info, ScanSearch, Sparkles, Upload, X, type LucideIcon } from "lucide-react";
import { useId } from "react";
import { Tooltip } from "@/components/ui/tooltip";
import type { TaskKind, TaskRecord } from "@/lib/jobs/task-records";
import { cn } from "@/lib/utils";
import { useRuntimeMode } from "../services/use-runtime-mode";

const kindIcon: Record<TaskKind, LucideIcon> = {
  transcription: Captions,
  analysis: ScanSearch,
  generation: Sparkles,
  render: Film,
  export: Upload,
  agent: Bot,
};

const actionClass =
  "grid h-7 w-7 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

function isActive(record: TaskRecord): boolean {
  return record.status === "running" || record.status === "queued" || record.status === "blocked";
}

/** "just now", "4 min ago", "2 h ago", then the date. */
function timeAgo(timestamp: string, now: number): string | null {
  const time = Date.parse(timestamp);
  if (!Number.isFinite(time)) return null;
  const minutes = Math.floor(Math.max(0, now - time) / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} h ago`;
  return new Date(time).toLocaleDateString();
}

/**
 * The file Show reveals. Only renders and exports: their paths are recorded export output, which the
 * restricted reveal command accepts; a generation's artifact is project media, not an export.
 */
function revealablePath(record: TaskRecord): string | null {
  return record.kind === "render" || record.kind === "export" ? record.artifactPath : null;
}

interface TaskRowProps {
  readonly record: TaskRecord;
  readonly now: number;
  /** Reveals a recorded export or render log path in the file manager. */
  onReveal(path: string): void;
  onCancel(id: string): void;
  onRetry(id: string): void;
  onDetails(id: string): void;
}

/** One background task: kind icon, label, progress or status line, plain failure copy, and its actions. */
export function TaskRow({ record, now, onReveal, onCancel, onRetry, onDetails }: TaskRowProps) {
  const runtimeMode = useRuntimeMode();
  const Icon = kindIcon[record.kind];
  const failed = record.status === "failed";
  const finished = !isActive(record);
  const ago = finished ? timeAgo(record.updatedAt, now) : null;
  const percent = record.progress === null ? null : Math.round(Math.min(1, Math.max(0, record.progress)) * 100);
  const statusLine = [record.detail, ago].filter(Boolean).join(" · ");
  const showPath = revealablePath(record);
  const logPath = record.logPath;
  const browser = runtimeMode === "browser";

  return (
    <li aria-label={record.label} className="flex items-start gap-2.5 border-b border-line px-1 py-2.5 last:border-b-0">
      <span className={cn("mt-0.5 grid h-6 w-6 shrink-0 place-items-center rounded-md bg-raised", failed ? "text-destructive" : "text-muted-foreground")}>
        <Icon className="h-3.5 w-3.5" aria-hidden />
      </span>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] text-foreground" title={record.label}>
          {record.label}
        </p>
        {percent !== null && isActive(record) ? (
          <div className="mt-1.5 flex items-center gap-2">
            <div
              role="progressbar"
              aria-label={`${record.label} progress`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percent}
              className="h-1 flex-1 overflow-hidden rounded-full bg-hover"
            >
              <div className="h-full rounded-full bg-primary motion-safe:transition-[width]" style={{ width: `${percent}%` }} />
            </div>
            <span className="tabular-time text-[11px] text-muted-foreground">{percent}%</span>
          </div>
        ) : (
          statusLine && <p className="mt-0.5 truncate text-[11px] text-muted-foreground">{statusLine}</p>
        )}
        {failed && record.failureReason && (
          <p className="mt-0.5 flex items-start gap-1 text-[11px] text-destructive">
            <CircleAlert className="mt-px h-3 w-3 shrink-0" aria-hidden />
            <span>{record.failureReason}</span>
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        {showPath && (
          <Tooltip content={browser ? "Download" : "Show in folder"}>
            <button type="button" aria-label={browser ? "Download" : "Show"} onClick={() => onReveal(showPath)} className={actionClass}>
              {browser ? <Download className="h-3.5 w-3.5" aria-hidden /> : <FolderOpen className="h-3.5 w-3.5" aria-hidden />}
            </button>
          </Tooltip>
        )}
        {logPath && !browser && (
          <Tooltip content="Show log in folder">
            <button type="button" aria-label="Log" onClick={() => onReveal(logPath)} className={actionClass}>
              <FileText className="h-3.5 w-3.5" aria-hidden />
            </button>
          </Tooltip>
        )}
        {record.retry && (
          <button
            type="button"
            onClick={() => onRetry(record.id)}
            className="h-7 rounded-md px-2 text-xs font-medium text-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            Retry
          </button>
        )}
        <CancelAction record={record} onCancel={onCancel} />
        <Tooltip content="Details">
          <button type="button" aria-label="Details" onClick={() => onDetails(record.id)} className={actionClass}>
            <Info className="h-3.5 w-3.5" aria-hidden />
          </button>
        </Tooltip>
      </div>
    </li>
  );
}

/** Enabled when the record has a real cancel; a running Temporal task shows it disabled with the reason. */
function CancelAction({ record, onCancel }: { readonly record: TaskRecord; onCancel(id: string): void }) {
  const reasonId = useId();
  if (record.cancel.available) {
    return (
      <Tooltip content="Cancel">
        <button type="button" aria-label="Cancel" onClick={() => onCancel(record.id)} className={actionClass}>
          <X className="h-3.5 w-3.5" aria-hidden />
        </button>
      </Tooltip>
    );
  }
  if (!isActive(record) || record.workflow?.backend !== "temporal") return null;
  return (
    <>
      <Tooltip content={record.cancel.reason} className="max-w-60">
        <button
          type="button"
          aria-label="Cancel"
          aria-disabled="true"
          aria-describedby={reasonId}
          className={cn(actionClass, "cursor-not-allowed opacity-40 hover:bg-transparent hover:text-muted-foreground")}
        >
          <X className="h-3.5 w-3.5" aria-hidden />
        </button>
      </Tooltip>
      <span id={reasonId} className="sr-only">
        {record.cancel.reason}
      </span>
    </>
  );
}
