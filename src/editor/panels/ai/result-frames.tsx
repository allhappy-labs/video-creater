import { useEffect, useMemo, useState } from "react";
import { clockLabel } from "@/lib/agent/result-facts";
import { projectContentEqual } from "@/lib/jobs/merge-job-state";
import { previewUrlForMedia } from "@/lib/media/preview-source";
import { captureCanonicalPreviewFrameInSplitProjectFolder, type CodexProposalImpact, type VideoProject } from "@/lib/project";
import type { AgentAppliedResult } from "../../store/agent-slice";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { pollable } from "../../store/jobs-slice";

/**
 * Post-apply frames for an applied card, rendered by the canonical frame capture from the saved
 * project. Captures run one at a time app-wide (each is a render), and results are cached by
 * project folder, result revision and time, so remounting a card never renders again.
 */

const maxFrames = 4;
/** Times closer than this share one frame. */
const sameFrameSeconds = 0.05;
/** The capture must start before the timeline end. */
const endMarginSeconds = 0.05;
const maxRememberedResults = 24;

type FrameCapture = { readonly status: "ready"; readonly url: string } | { readonly status: "failed" };

const frameCache = new Map<string, FrameCapture>();
/** The applied project per result, to tell content edits from bookkeeping-only revision bumps. */
const resultProjects = new Map<string, VideoProject>();
let captureQueue: Promise<unknown> = Promise.resolve();
let captureSequence = 0;

function enqueue(task: () => Promise<void>): Promise<void> {
  const run = captureQueue.then(task, task);
  captureQueue = run.catch(() => undefined);
  return run;
}

function frameKey(projectDir: string, revision: number, seconds: number): string {
  return `${projectDir}\n${revision.toString()}\n${seconds.toFixed(3)}`;
}

/** The preview timestamp, then the starts of the first three affected ranges; at most four, in range. */
function frameTimes(impact: CodexProposalImpact, durationSeconds: number): number[] {
  const last = durationSeconds - endMarginSeconds;
  const times: number[] = [];
  for (const candidate of [impact.previewTimestamp, ...impact.affectedRanges.slice(0, 3).map((range) => range.startSeconds)]) {
    if (!Number.isFinite(candidate) || last <= 0) continue;
    const seconds = Math.min(Math.max(0, candidate), last);
    if (times.every((time) => Math.abs(time - seconds) > sameFrameSeconds)) times.push(seconds);
  }
  return times.slice(0, maxFrames);
}

function resultKey(projectDir: string, result: AgentAppliedResult): string {
  return `${projectDir}\n${result.historyEntryId}`;
}

/**
 * Whether the project changed since the result applied. Bookkeeping writes (the frame capture's own
 * job record, polled jobs) bump the revision without changing content, so once the applied project
 * was seen the check compares content; otherwise any newer revision counts as a change.
 */
function isStale(projectDir: string, result: AgentAppliedResult, project: VideoProject): boolean {
  const key = resultKey(projectDir, result);
  const revision = project.contentRevision ?? 0;
  const applied = resultProjects.get(key);
  if (!applied && revision === result.revision) {
    resultProjects.set(key, project);
    const [oldest] = resultProjects.keys();
    if (resultProjects.size > maxRememberedResults && oldest !== undefined) resultProjects.delete(oldest);
    return false;
  }
  return applied ? !projectContentEqual(applied, project) : revision > result.revision;
}

function useResultStale(result: AgentAppliedResult): boolean {
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  return isStale(projectDir, result, project);
}

function captureJobId(revision: number, seconds: number): string {
  captureSequence += 1;
  return `agent-result-frame-${revision.toString()}-${Math.round(seconds * 1000).toString()}-${Date.now().toString(36)}-${captureSequence.toString()}`;
}

type FrameView = { readonly seconds: number; readonly capture: FrameCapture | null };

interface ResultFramesProps {
  readonly result: AgentAppliedResult;
  onOpenViewer(seconds: number): void;
}

export function ResultFrames({ result, onOpenViewer }: ResultFramesProps) {
  const store = useEditorStoreApi();
  const projectDir = useEditorStore((state) => state.projectDir);
  const durationSeconds = useEditorStore((state) => state.project.timeline.durationSeconds);
  const stale = useResultStale(result);
  const times = useMemo(() => frameTimes(result.impact, durationSeconds), [result.impact, durationSeconds]);
  const [retryToken, setRetryToken] = useState(0);
  const [, setCaptured] = useState(0);
  const timesKey = times.join(",");

  useEffect(() => {
    let mounted = true;
    void (async () => {
      for (const seconds of timesKey.split(",").filter(Boolean).map(Number)) {
        const key = frameKey(projectDir, result.revision, seconds);
        if (frameCache.has(key)) continue;
        // An unmounted card stops queueing; remounting resumes from the cache.
        if (!mounted) return;
        await enqueue(async () => {
          if (frameCache.has(key)) return;
          const state = store.getState();
          // A changed project would render the wrong frames, and only saved split projects can render.
          if (!pollable(state.project, state.projectDir) || isStale(projectDir, result, state.project)) {
            frameCache.set(key, { status: "failed" });
            return;
          }
          try {
            const captured = await captureCanonicalPreviewFrameInSplitProjectFolder({
              projectDir,
              playheadSeconds: seconds,
              jobId: captureJobId(result.revision, seconds),
              updatedAt: new Date().toISOString(),
            });
            const url = previewUrlForMedia(projectDir, captured.previewFrame);
            frameCache.set(key, url ? { status: "ready", url } : { status: "failed" });
            // The capture recorded a job; merge it so the next save doesn't conflict.
            void store.getState().mergeLoadedProject(captured.project);
          } catch {
            frameCache.set(key, { status: "failed" });
          }
        });
        if (mounted) setCaptured((count) => count + 1);
      }
    })();
    return () => {
      mounted = false;
    };
    // `result` only matters through its revision and history entry.
  }, [store, projectDir, result.revision, result.historyEntryId, timesKey, retryToken]);

  if (times.length === 0) return null;
  const frames: FrameView[] = times.map((seconds) => ({ seconds, capture: frameCache.get(frameKey(projectDir, result.revision, seconds)) ?? null }));
  const ready = frames.filter((frame): frame is { seconds: number; capture: { status: "ready"; url: string } } => frame.capture?.status === "ready");
  const settled = frames.every((frame) => frame.capture !== null);
  const failed = settled && ready.length === 0;

  function retry() {
    for (const seconds of times) {
      const key = frameKey(projectDir, result.revision, seconds);
      if (frameCache.get(key)?.status === "failed") frameCache.delete(key);
    }
    setRetryToken((token) => token + 1);
  }

  function play(seconds: number) {
    const state = store.getState();
    state.previewTimeline();
    state.seek(seconds);
    state.setPlaying(true);
  }

  if (failed) {
    return (
      <div role="group" aria-label="Result preview" className="flex flex-wrap items-center gap-x-2 gap-y-1 px-3 pb-2.5 text-[12px] text-muted-foreground">
        <span className="min-w-0 flex-1">Preview frames aren't available.</span>
        <button type="button" onClick={() => onOpenViewer(result.impact.previewTimestamp)} className={linkButtonClass}>
          Open viewer
        </button>
        <span aria-hidden className="text-dim">
          ·
        </span>
        <button type="button" onClick={retry} className={linkButtonClass}>
          Retry preview
        </button>
      </div>
    );
  }

  return (
    <div role="group" aria-label="Result preview" className="flex flex-col gap-1 px-3 pb-2.5">
      {stale && ready.length > 0 && <p className="text-[11px] font-medium text-warning">Earlier version</p>}
      <ul className="grid grid-cols-4 gap-1">
        {frames.map((frame) =>
          frame.capture?.status === "ready" ? (
            <li key={frame.seconds}>
              <button
                type="button"
                aria-label={`Play from ${clockLabel(frame.seconds)}`}
                onClick={() => play(frame.seconds)}
                className="block aspect-video w-full overflow-hidden rounded bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <img src={frame.capture.url} alt="" width={160} height={90} draggable={false} className={`h-full w-full object-cover ${stale ? "opacity-60" : ""}`} />
              </button>
            </li>
          ) : frame.capture === null ? (
            <li key={frame.seconds} aria-hidden className="aspect-video rounded bg-panel motion-safe:animate-pulse" />
          ) : null,
        )}
      </ul>
      {!settled && (
        <p role="status" className="text-[11px] text-dim">
          Preparing the preview…
        </p>
      )}
    </div>
  );
}

const linkButtonClass =
  "h-6 rounded-md px-1.5 text-[12px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";
