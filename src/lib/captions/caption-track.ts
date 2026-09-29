import { buildCaptionItems, type CaptionBuildOptions } from "@/lib/captions/caption-items";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { planDropTarget, type DropTargetPlan } from "@/lib/timeline-ops/dynamic-tracks";
import { orderedTracksByBand } from "@/lib/timeline-ops/track-bands";

export type CaptionBuildPlan =
  | { readonly actions: ProjectAction[]; readonly trackId: string; readonly itemIds: string[] }
  | { readonly blocked: string };

/**
 * One action list that places built captions: the first unlocked caption track (display order)
 * that accepts the whole cue span, or else a new caption track (`createTrack`, plus its
 * `reorderTrack` when needed) followed by `addItems`. `newTrackId` is caller-supplied, e.g. from
 * `newTrackId(timeline, "caption")`.
 */
export function captionBuildActions(
  project: VideoProject,
  options: CaptionBuildOptions,
  newTrackId: string,
): CaptionBuildPlan {
  const items = buildCaptionItems(options);
  if (items.length === 0) return { blocked: "There are no transcript words in this range." };
  const startSeconds = Math.min(...items.map((item) => item.startSeconds));
  const endSeconds = Math.max(...items.map((item) => item.startSeconds + item.durationSeconds));
  const { timeline } = project;
  const span = { timeline, assetKind: "caption", startSeconds, durationSeconds: endSeconds - startSeconds, newTrackId } as const;

  let plan: DropTargetPlan | null = null;
  for (const track of orderedTracksByBand(timeline)) {
    if (track.kind !== "caption" || track.locked) continue;
    const candidate = planDropTarget({ ...span, hoveredTrackId: track.id, insertIndex: null });
    if (candidate.kind === "existing") {
      plan = candidate;
      break;
    }
  }
  plan ??= planDropTarget({ ...span, hoveredTrackId: null, insertIndex: null });
  if (plan.kind === "invalid") return { blocked: plan.reason };

  const actions: ProjectAction[] = [];
  if (plan.kind === "create") {
    actions.push(plan.createTrack);
    if (plan.reorderTrack) actions.push(plan.reorderTrack);
  }
  actions.push({ type: "addItems", targetTrackId: plan.trackId, items });
  return { actions, trackId: plan.trackId, itemIds: items.map((item) => item.id) };
}
