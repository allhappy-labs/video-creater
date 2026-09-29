import { mediaContentKind } from "@/lib/media/media-filters";
import { mediaDisplayName } from "@/lib/media/names";
import type { ProjectActionRippleDeleteRange, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import { timelineSilenceRippleRanges } from "@/lib/timeline-ops/silence";
import { linkedAudioItem } from "../../properties/linked-audio";

export interface CleanupTargetOption {
  /** `item:<id>` for the selected clip, `media:<id>` for a source across the whole project. */
  readonly value: string;
  readonly label: string;
}

export interface CleanupTarget {
  readonly kind: "item" | "media";
  readonly mediaId: string;
  /** The selected clip, or every timeline clip of the source media. */
  readonly items: readonly TimelineItem[];
}

const rangeToleranceSeconds = 0.001;

function timelineItems(project: VideoProject): TimelineItem[] {
  return project.timeline.tracks.flatMap((track) => track.items);
}

function speechMediaId(project: VideoProject, item: TimelineItem): string | null {
  const mediaId = timelineItemSourceMediaId(item);
  const media = project.media.find((candidate) => candidate.id === mediaId);
  if (!media) return null;
  const kind = mediaContentKind(media);
  return kind === "audio" || kind === "video" ? media.id : null;
}

/**
 * "Clean up speech" choices: the first selected clip with speech-capable media, then every audio or
 * video source, in project order. The first option is the default.
 */
export function cleanupTargetOptions(project: VideoProject, selectedItemIds: readonly string[]): CleanupTargetOption[] {
  const options: CleanupTargetOption[] = [];
  const items = timelineItems(project);
  const selected = selectedItemIds
    .map((itemId) => items.find((item) => item.id === itemId))
    .find((item): item is TimelineItem => item !== undefined && speechMediaId(project, item) !== null);
  if (selected) options.push({ value: `item:${selected.id}`, label: `Selected clip · ${selected.label}` });

  const usedMediaIds = new Set(items.map((item) => timelineItemSourceMediaId(item)));
  const sources = project.media.filter((media) => {
    const kind = mediaContentKind(media);
    return kind === "audio" || kind === "video";
  });
  // Sources on the timeline come first so the default target has clips to clean up.
  const ordered = [...sources.filter((media) => usedMediaIds.has(media.id)), ...sources.filter((media) => !usedMediaIds.has(media.id))];
  for (const media of ordered) options.push({ value: `media:${media.id}`, label: mediaDisplayName(media) });
  return options;
}

export function resolveCleanupTarget(project: VideoProject, value: string | null): CleanupTarget | null {
  if (value === null) return null;
  const items = timelineItems(project);
  if (value.startsWith("item:")) {
    const item = items.find((candidate) => candidate.id === value.slice("item:".length));
    const mediaId = item ? speechMediaId(project, item) : null;
    return item && mediaId ? { kind: "item", mediaId, items: [item] } : null;
  }
  if (value.startsWith("media:")) {
    const mediaId = value.slice("media:".length);
    if (!project.media.some((media) => media.id === mediaId)) return null;
    return { kind: "media", mediaId, items: items.filter((item) => timelineItemSourceMediaId(item) === mediaId) };
  }
  return null;
}

/** Detected silences for the target as ripple ranges (speed-aware), limited to the clip's span for a clip. */
export function targetSilenceRanges(project: VideoProject, target: CleanupTarget): ProjectActionRippleDeleteRange[] {
  const mediaSilenceRanges = (project.mediaSilenceRanges ?? []).filter((range) => range.mediaId === target.mediaId);
  const ranges = timelineSilenceRippleRanges({ ...project, mediaSilenceRanges });
  const [item] = target.items;
  if (target.kind === "media" || !item) return ranges;
  const end = item.startSeconds + item.durationSeconds;
  return ranges.filter(
    (range) => range.startSeconds >= item.startSeconds - rangeToleranceSeconds && range.endSeconds <= end + rangeToleranceSeconds,
  );
}

/** The audio clips that carry the target's sound (video clips resolve to their linked audio clip). */
export function targetAudioItems(project: VideoProject, target: CleanupTarget): TimelineItem[] {
  const byId = new Map<string, TimelineItem>();
  for (const item of target.items) {
    const audio = linkedAudioItem(project, item);
    if (audio) byId.set(audio.id, audio);
  }
  return [...byId.values()];
}

/** The clip whose audio speech analysis reads: its audio clip when there is one. */
export function speechAnalysisItem(project: VideoProject, target: CleanupTarget): TimelineItem | null {
  return targetAudioItems(project, target)[0] ?? target.items[0] ?? null;
}
