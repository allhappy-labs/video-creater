import type { CodexProposalImpact, VideoProject } from "@/lib/project";
import { basePixelsPerSecond } from "@/lib/timeline-ops/navigation";
import { useLayoutMode } from "../../shell/use-layout-mode";
import { useEditorStoreApi } from "../../store/editor-store-context";
import type { SelectionSlice } from "../../store/selection-slice";

type HighlightRanges = Parameters<SelectionSlice["setHighlights"]>[1];

/** Space left of the first changed range when the timeline scrolls to it. */
const revealMarginPixels = 48;

/**
 * Timeline bands for the affected ranges, clamped to the current timeline (impact ranges can cover
 * where clips were before the edit) with empty ranges dropped. Each range spans the tracks of the
 * affected items that overlap it, else the tracks of every affected item, else every track.
 */
function highlightRangesFor(project: VideoProject, impact: CodexProposalImpact): HighlightRanges {
  const affected = new Set(impact.affectedItemIds);
  const { tracks, durationSeconds } = project.timeline;
  const affectedTracks = tracks.filter((track) => track.items.some((item) => affected.has(item.id)));
  return impact.affectedRanges.flatMap((range) => {
    const startSeconds = Math.max(0, range.startSeconds);
    const endSeconds = Math.min(durationSeconds, range.endSeconds);
    if (!Number.isFinite(startSeconds) || !Number.isFinite(endSeconds) || endSeconds <= startSeconds) return [];
    const overlapping = affectedTracks.filter((track) =>
      track.items.some((item) => affected.has(item.id) && item.startSeconds < endSeconds && item.startSeconds + item.durationSeconds > startSeconds),
    );
    const spanned = overlapping.length > 0 ? overlapping : affectedTracks.length > 0 ? affectedTracks : tracks;
    return [{ trackIds: spanned.map((track) => track.id), startSeconds, endSeconds }];
  });
}

/**
 * Show changes: highlights the affected items and ranges, seeks to the preview timestamp, scrolls
 * the desktop timeline to the first range (the mobile timeline centers the playhead itself), and
 * closes the AI sheet on mobile so the timeline is visible.
 */
export function useShowChanges(): (impact: CodexProposalImpact) => void {
  const store = useEditorStoreApi();
  const mobile = useLayoutMode() === "mobile";
  return (impact) => {
    const state = store.getState();
    const present = new Set(state.project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)));
    const ranges = highlightRangesFor(state.project, impact);
    state.setHighlights(
      impact.affectedItemIds.filter((id) => present.has(id)),
      ranges,
    );
    state.previewTimeline();
    state.seek(impact.previewTimestamp);
    const [first] = ranges;
    if (mobile) {
      if (state.openSheetId === "ai") state.closeSheet();
    } else if (first) {
      const pixelsPerSecond = (basePixelsPerSecond * state.zoomPercent) / 100;
      state.setScrollLeft(Math.max(0, first.startSeconds * pixelsPerSecond - revealMarginPixels));
    }
  };
}

/** Open viewer (frame fallback): seeks the timeline preview and closes the AI sheet on mobile. */
export function useOpenViewer(): (seconds: number) => void {
  const store = useEditorStoreApi();
  const mobile = useLayoutMode() === "mobile";
  return (seconds) => {
    const state = store.getState();
    state.previewTimeline();
    state.seek(seconds);
    if (mobile && state.openSheetId === "ai") state.closeSheet();
  };
}
