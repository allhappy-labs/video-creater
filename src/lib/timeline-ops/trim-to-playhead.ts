import { roundTimelineSeconds } from "@/lib/format";
import { projectActionFromTimelinePatch, type ProjectAction, type VideoProject } from "@/lib/project";
import { createLeftTrimPatchFromDrag, createResizePatchFromDrag, createRightTrimPatchFromDrag } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";

type TrimAction = Extract<ProjectAction, { type: "trimItems" }>;
type ResizeAction = Extract<ProjectAction, { type: "resizeItems" }>;

/**
 * Legacy `[` / `]`: trims the start or end of every selected, unlocked clip the playhead is
 * strictly inside to the playhead. Start edges trim (moving `sourceIn`); end edges trim media
 * clips (moving `sourceOut`) and resize the rest. One `trimItems` and one `resizeItems` batch.
 */
export function trimItemsToPlayhead(
  project: VideoProject,
  itemIds: readonly string[],
  playheadSeconds: number,
  edge: "start" | "end",
): CommandResult {
  const wanted = new Set(itemIds);
  const selected = project.timeline.tracks.flatMap((track) =>
    track.items.filter((item) => wanted.has(item.id)).map((item) => ({ item, track })),
  );
  if (selected.length === 0) return { blocked: "Select a clip to trim." };
  const unlocked = selected.filter(({ track }) => !track.locked);
  if (unlocked.length === 0) return { blocked: "Unlock the selected tracks to trim these clips." };
  const seconds = roundTimelineSeconds(playheadSeconds);
  const inside = unlocked.filter(({ item }) => seconds > item.startSeconds && seconds < item.startSeconds + item.durationSeconds);
  if (inside.length === 0) return { blocked: "Move the playhead over the selected clip to trim." };

  const trims: TrimAction["trims"][number][] = [];
  const resizes: ResizeAction["resizes"][number][] = [];
  for (const { item } of inside) {
    const patch =
      edge === "start"
        ? createLeftTrimPatchFromDrag(item, seconds)
        : (createRightTrimPatchFromDrag(item, seconds) ??
          createResizePatchFromDrag({ itemId: item.id, durationSeconds: roundTimelineSeconds(seconds - item.startSeconds) }));
    const action = projectActionFromTimelinePatch(patch);
    if (action.type === "trimItems") trims.push(...action.trims);
    else if (action.type === "resizeItems") resizes.push(...action.resizes);
  }
  const actions: ProjectAction[] = [];
  if (trims.length > 0) actions.push({ type: "trimItems", trims });
  if (resizes.length > 0) actions.push({ type: "resizeItems", resizes });
  return { actions };
}
