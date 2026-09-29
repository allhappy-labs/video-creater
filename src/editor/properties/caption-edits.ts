import { buildCaptionItems, captionWordTokens } from "@/lib/captions/caption-items";
import type { ProjectAction, VideoProject } from "@/lib/project";
import {
  captionGroupItems,
  captionStylePreset,
  captionWordAnimationPreset,
  captionWordAnimations,
  captionWordStaggerSeconds,
} from "@/lib/properties/caption-properties";
import { getTimelineItemText, type TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { numberProperty, stringProperty } from "@/lib/timeline-ops/item-properties";

/** The caption builder's words-per-cue bounds (`buildCaptionItems` clamps to these). */
export const captionWordsPerCueRange = { min: 1, max: 12 } as const;

/** Group-wide style a regroup keeps; the rebuilt cues otherwise start from the preset. */
const carriedStyleKeys = ["captionPlacement", "fontName", "fontSize", "highlightColor", "motionPresetId"] as const;

export interface CaptionRegroup {
  readonly actions: readonly ProjectAction[];
  readonly fromCount: number;
  readonly toCount: number;
  /** The rebuilt cue covering the selected cue's start, so the selection survives. */
  readonly selectItemId: string | null;
}

function cueWordCount(item: TimelineItem): number {
  const start = numberProperty(item, "wordStartIndex");
  const end = numberProperty(item, "wordEndIndex");
  if (start !== null && end !== null && end >= start) return end - start + 1;
  return captionWordTokens(getTimelineItemText(item)).length;
}

/** Max words per line for the cue's group: its longest cue. */
export function captionWordsPerCue(project: VideoProject, itemId: string): number {
  return Math.max(captionWordsPerCueRange.min, ...captionGroupItems(project, itemId).map(cueWordCount));
}

function trackOf(project: VideoProject, itemId: string) {
  return project.timeline.tracks.find((track) => track.items.some((item) => item.id === itemId));
}

/**
 * Rebuilds the cue's caption group with `buildCaptionItems` from its transcript and source range:
 * one batch that removes the old cues and adds the new ones under the same group id.
 */
export function captionRegroup(project: VideoProject, itemId: string, wordsPerCue: number): CaptionRegroup | { blocked: string } {
  const group = [...captionGroupItems(project, itemId)].sort((left, right) => left.startSeconds - right.startSeconds);
  const selected = group.find((item) => item.id === itemId);
  if (!selected) return { blocked: "Select a caption to regroup its cues." };
  if (!Number.isInteger(wordsPerCue) || wordsPerCue < captionWordsPerCueRange.min || wordsPerCue > captionWordsPerCueRange.max) {
    return { blocked: "Max words per line must be a whole number from 1 to 12." };
  }
  const groupId = stringProperty(selected, "captionGroupId");
  if (!groupId) return { blocked: "This caption isn't part of a caption group." };
  const transcriptId = stringProperty(selected, "transcriptId");
  const transcript = project.transcripts.find((candidate) => candidate.id === transcriptId);
  if (!transcript) return { blocked: "This caption group has no transcript to regroup from." };
  const track = trackOf(project, itemId);
  if (!track || group.some((item) => trackOf(project, item.id) !== track)) {
    return { blocked: "Move the caption group onto one track to regroup its cues." };
  }
  if (track.locked) return { blocked: "Unlock the caption track to regroup its cues." };

  const sourceIns = group.map((item) => numberProperty(item, "sourceIn"));
  const sourceOuts = group.map((item) => numberProperty(item, "sourceOut"));
  if (sourceIns.includes(null) || sourceOuts.includes(null)) {
    return { blocked: "This caption group has no source timing to regroup from." };
  }
  const sourceStart = Math.min(...(sourceIns as number[]));
  const sourceEnd = Math.max(...(sourceOuts as number[]));
  const timelineStart = Math.min(...group.map((item) => item.startSeconds));
  const timelineEnd = Math.max(...group.map((item) => item.startSeconds + item.durationSeconds));
  const carried = Object.fromEntries(
    carriedStyleKeys.flatMap((key) => (selected.properties[key] === undefined ? [] : [[key, selected.properties[key]]])),
  );
  const items = buildCaptionItems({
    transcript,
    range: {
      startSeconds: sourceStart,
      endSeconds: sourceEnd,
      timelineStartSeconds: timelineStart,
      timelineSecondsPerSourceSecond: sourceEnd > sourceStart ? (timelineEnd - timelineStart) / (sourceEnd - sourceStart) : 1,
    },
    wordsPerCue,
    stylePreset: captionStylePreset(selected),
    groupId,
  }).map((item) => ({ ...item, properties: { ...item.properties, ...carried } }));
  if (items.length === 0) return { blocked: "No transcript words fall inside this caption group." };

  const covering = items.find((item) => item.startSeconds + item.durationSeconds > selected.startSeconds) ?? items[0];
  return {
    actions: [
      { type: "removeItems", itemIds: group.map((item) => item.id) },
      { type: "addItems", targetTrackId: track.id, items },
    ],
    fromCount: group.length,
    toCount: items.length,
    selectItemId: covering?.id ?? null,
  };
}

/** Stored emphasis filtered to the cue's word indices (legacy inspector rule). */
export function captionEmphasizedWords(item: TimelineItem): number[] {
  const count = captionWordTokens(getTimelineItemText(item)).length;
  const stored = item.properties.emphasizedWordIndices;
  return Array.isArray(stored)
    ? stored.filter((value): value is number => typeof value === "number" && Number.isInteger(value) && value >= 0 && value < count)
    : [];
}

/**
 * Toggles one word's emphasis on this cue: a non-empty list sets `emphasizedWordIndices`, an empty
 * one removes it. When a word animation preset is applied, its animations are rebuilt for the new
 * words in the same update so the preset keeps covering every emphasized word.
 */
export function captionEmphasisAction(item: TimelineItem, wordIndex: number): CommandResult {
  if (item.kind !== "caption") return { blocked: "Select a caption to emphasize its words." };
  const current = captionEmphasizedWords(item);
  const next = current.includes(wordIndex)
    ? current.filter((index) => index !== wordIndex)
    : [...current, wordIndex].sort((left, right) => left - right);
  const set: Record<string, unknown> = next.length > 0 ? { emphasizedWordIndices: next } : {};
  const remove = next.length > 0 ? [] : ["emphasizedWordIndices"];
  if (stringProperty(item, "captionWordAnimationPreset") !== null) {
    const withWords = { ...item, properties: { ...item.properties, emphasizedWordIndices: next } };
    set.captionWordAnimations = captionWordAnimations(withWords, captionWordAnimationPreset(item), captionWordStaggerSeconds(item));
  }
  return { actions: [{ type: "updateItemProperties", updates: [{ itemId: item.id, set, remove }] }] };
}
