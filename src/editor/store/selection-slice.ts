import { activeTimelineHasTransition } from "@/lib/preview/selection-kind";
import type { VideoProject } from "@/lib/project";
import type { TimelineClipboard } from "@/lib/timeline-ops/clipboard";
import type { MarqueeRect } from "@/lib/timeline-ops/marquee";
import { timelineGapAtSeconds, type TimelineGapSelection } from "@/lib/timeline-ops/navigation";
import type { EditorSliceCreator } from "./editor-store";

/** A contiguous run of transcript words, by index into `transcript.words` (inclusive). */
interface TranscriptWordRange {
  readonly transcriptId: string;
  readonly startWordIndex: number;
  readonly endWordIndex: number;
}

/** A timeline span to band; an empty `trackIds` spans every track. */
interface HighlightRange {
  readonly trackIds: readonly string[];
  readonly startSeconds: number;
  readonly endSeconds: number;
}

export interface SelectionSlice {
  readonly selectedItemIds: readonly string[];
  readonly highlightedItemIds: readonly string[];
  readonly highlightedRanges: readonly HighlightRange[];
  readonly clipboard: TimelineClipboard | null;
  readonly selectedTrackId: string | null;
  /** The in-progress marquee drag, in tracks-area content pixels. */
  readonly marquee: MarqueeRect | null;
  /** I/O range marks in timeline seconds. */
  readonly rangeIn: number | null;
  readonly rangeOut: number | null;
  /** The empty gap picked by clicking empty lane space; Delete closes it. Item selection clears it. */
  readonly selectedGap: TimelineGapSelection | null;
  /**
   * The transition picked by clicking its badge on the active timeline. It replaces the item and gap
   * selection (and item selection clears it); Properties then shows its Transition tab.
   */
  readonly selectedTransitionId: string | null;
  /** Reserved for VC-006 transcript cutting (strike and delete); the Captions tab only reads it for now. */
  readonly transcriptRange: TranscriptWordRange | null;
  selectItems(itemIds: readonly string[]): void;
  toggleItemSelection(itemId: string): void;
  clearSelection(): void;
  hasSelection(): boolean;
  /** Show changes: outlines the agent's affected items and bands its affected ranges on the timeline. */
  setHighlights(itemIds: readonly string[], ranges: readonly HighlightRange[]): void;
  /** Drops the highlights (the next edit, Esc, a timeline switch); a no-op when there are none. */
  clearHighlights(): void;
  pruneSelection(project: VideoProject): void;
  setClipboard(clipboard: TimelineClipboard | null): void;
  selectTrack(trackId: string | null): void;
  setMarquee(marquee: MarqueeRect | null): void;
  setRangeIn(seconds: number | null): void;
  setRangeOut(seconds: number | null): void;
  clearRange(): void;
  selectGap(gap: TimelineGapSelection | null): void;
  selectTransition(transitionId: string | null): void;
  setTranscriptRange(range: TranscriptWordRange | null): void;
  /** Clears item, gap, transition and track selection, marquee, range marks, highlights, clipboard and the replace target (timeline switch). */
  clearTimelineContext(): void;
}

function projectItemIds(project: VideoProject): Set<string> {
  const ids = new Set<string>();
  const timelines = project.timelines?.length ? project.timelines.map((entry) => entry.timeline) : [project.timeline];
  for (const timeline of timelines) {
    for (const track of timeline.tracks) {
      for (const item of track.items) ids.add(item.id);
    }
  }
  return ids;
}

/** The gap still exists on its track with the same bounds. */
function gapStillOpen(project: VideoProject, gap: TimelineGapSelection): boolean {
  const track = project.timeline.tracks.find((candidate) => candidate.id === gap.trackId);
  const current = track ? timelineGapAtSeconds(track, (gap.startSeconds + gap.endSeconds) / 2) : null;
  return current !== null && current.startSeconds === gap.startSeconds && current.endSeconds === gap.endSeconds;
}

function finiteSecondsOrNull(seconds: number | null): number | null {
  return seconds !== null && Number.isFinite(seconds) ? Math.max(0, seconds) : null;
}

export const createSelectionSlice: EditorSliceCreator<SelectionSlice> = (set, get) => ({
  selectedItemIds: [],
  highlightedItemIds: [],
  highlightedRanges: [],
  clipboard: null,
  selectedTrackId: null,
  marquee: null,
  rangeIn: null,
  rangeOut: null,
  selectedGap: null,
  selectedTransitionId: null,
  transcriptRange: null,
  selectItems: (itemIds) => set({ selectedItemIds: [...new Set(itemIds)], selectedGap: null, selectedTransitionId: null }),
  toggleItemSelection: (itemId) =>
    set((state) => ({
      selectedItemIds: state.selectedItemIds.includes(itemId)
        ? state.selectedItemIds.filter((id) => id !== itemId)
        : [...state.selectedItemIds, itemId],
      selectedGap: null,
      selectedTransitionId: null,
    })),
  clearSelection: () => set({ selectedItemIds: [], selectedGap: null, selectedTransitionId: null }),
  hasSelection: () => get().selectedItemIds.length > 0,
  setHighlights: (itemIds, ranges) => set({ highlightedItemIds: [...itemIds], highlightedRanges: [...ranges] }),
  clearHighlights: () => {
    const { highlightedItemIds, highlightedRanges } = get();
    if (highlightedItemIds.length > 0 || highlightedRanges.length > 0) set({ highlightedItemIds: [], highlightedRanges: [] });
  },
  pruneSelection: (project) => {
    const ids = projectItemIds(project);
    const { selectedItemIds: current, selectedTrackId, replaceTargetItemId, selectedGap, selectedTransitionId } = get();
    const kept = current.filter((id) => ids.has(id));
    const trackGone =
      selectedTrackId !== null && !project.timeline.tracks.some((track) => track.id === selectedTrackId);
    const replaceTargetGone = replaceTargetItemId !== null && !ids.has(replaceTargetItemId);
    const gapGone = selectedGap !== null && !gapStillOpen(project, selectedGap);
    const transitionGone = selectedTransitionId !== null && !activeTimelineHasTransition(project, selectedTransitionId);
    // Only write when something was dropped, so project subscribers never see a no-op update.
    if (kept.length !== current.length || trackGone || replaceTargetGone || gapGone || transitionGone) {
      set({
        selectedItemIds: kept,
        ...(trackGone ? { selectedTrackId: null } : {}),
        ...(replaceTargetGone ? { replaceTargetItemId: null } : {}),
        ...(gapGone ? { selectedGap: null } : {}),
        ...(transitionGone ? { selectedTransitionId: null } : {}),
      });
    }
  },
  setClipboard: (clipboard) => set({ clipboard }),
  selectTrack: (trackId) => set({ selectedTrackId: trackId }),
  setMarquee: (marquee) => set({ marquee }),
  setRangeIn: (seconds) => set({ rangeIn: finiteSecondsOrNull(seconds) }),
  setRangeOut: (seconds) => set({ rangeOut: finiteSecondsOrNull(seconds) }),
  clearRange: () => set({ rangeIn: null, rangeOut: null }),
  selectGap: (gap) => set(gap === null ? { selectedGap: null } : { selectedGap: gap, selectedTransitionId: null }),
  selectTransition: (transitionId) =>
    set(transitionId === null ? { selectedTransitionId: null } : { selectedTransitionId: transitionId, selectedItemIds: [], selectedGap: null }),
  setTranscriptRange: (range) => set({ transcriptRange: range }),
  clearTimelineContext: () =>
    set({
      selectedItemIds: [],
      selectedTrackId: null,
      marquee: null,
      rangeIn: null,
      rangeOut: null,
      clipboard: null,
      replaceTargetItemId: null,
      selectedGap: null,
      selectedTransitionId: null,
      highlightedItemIds: [],
      highlightedRanges: [],
    }),
});
