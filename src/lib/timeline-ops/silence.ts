import { roundTimelineSeconds } from "@/lib/format";
import type { ProjectActionRippleDeleteRange, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty as timelineItemNumberProperty } from "./item-properties";
import { isReversedItem, timelineRangeForSource } from "./reverse";

const removeSilenceBoundaryPaddingSeconds = 0.12;
const removeSilenceMinGapSeconds = 0.75;

export function timelineSilenceRippleRanges(project: VideoProject): ProjectActionRippleDeleteRange[] {
  const silenceRanges = project.mediaSilenceRanges ?? [];
  const ranges: ProjectActionRippleDeleteRange[] = [];
  const seenRanges = new Set<string>();

  for (const silence of silenceRanges) {
    if (!validMediaSilenceRange(silence)) {
      continue;
    }

    const sourceStartSeconds = silence.sourceIn + removeSilenceBoundaryPaddingSeconds;
    const sourceEndSeconds = silence.sourceOut - removeSilenceBoundaryPaddingSeconds;
    if (
      sourceEndSeconds <= sourceStartSeconds ||
      sourceEndSeconds - sourceStartSeconds < removeSilenceMinGapSeconds
    ) {
      continue;
    }

    for (const track of project.timeline.tracks) {
      for (const item of track.items) {
        if (item.source.type !== "media" || item.source.mediaId !== silence.mediaId) {
          continue;
        }

        const speed = playbackSpeed(item);
        const itemSourceIn = timelineItemNumberProperty(item, "sourceIn") ?? 0;
        const itemSourceOut =
          timelineItemNumberProperty(item, "sourceOut") ??
          itemSourceIn + item.durationSeconds * speed;
        if (sourceStartSeconds < itemSourceIn || sourceEndSeconds > itemSourceOut) {
          continue;
        }

        const sourceWindow = { sourceIn: itemSourceIn, sourceOut: itemSourceOut, speed, reverse: isReversedItem(item) };
        const [timelineStart, timelineEnd] = timelineRangeForSource(sourceWindow, item.startSeconds, sourceStartSeconds, sourceEndSeconds);
        const startSeconds = roundTimelineSeconds(timelineStart);
        const endSeconds = roundTimelineSeconds(timelineEnd);
        if (endSeconds <= startSeconds) {
          continue;
        }

        const key = `${startSeconds}:${endSeconds}`;
        if (seenRanges.has(key)) {
          continue;
        }
        seenRanges.add(key);
        ranges.push({
          startSeconds,
          endSeconds,
          trackIds: [track.id],
        });
      }
    }
  }

  return ranges;
}

export function currentTimelineSilenceRippleRange(
  project: VideoProject,
  playheadSeconds: number,
): ProjectActionRippleDeleteRange | null {
  return timelineSilenceRippleRanges(project).find(
    (range) => playheadSeconds >= range.startSeconds && playheadSeconds < range.endSeconds,
  ) ?? null;
}

export function rippleTrackIdsForItem(project: VideoProject, sourceItem: TimelineItem, sourceTrackId: string) {
  const linkGroupId =
    typeof sourceItem.properties.linkGroupId === "string" &&
    sourceItem.properties.linkGroupId.trim().length > 0
      ? sourceItem.properties.linkGroupId
      : null;
  if (!linkGroupId) return [sourceTrackId];
  return project.timeline.tracks
    .filter((track) => track.items.some((item) => item.properties.linkGroupId === linkGroupId))
    .map((track) => track.id);
}

/** Clip playback speed; missing or invalid speeds fall back to 1 (mirrors the Rust tool). */
function playbackSpeed(item: TimelineItem) {
  const speed = timelineItemNumberProperty(item, "speed");
  return speed !== null && speed > 0 ? speed : 1;
}

function validMediaSilenceRange(
  range: NonNullable<VideoProject["mediaSilenceRanges"]>[number],
) {
  return (
    range.mediaId.trim().length > 0 &&
    Number.isFinite(range.sourceIn) &&
    Number.isFinite(range.sourceOut) &&
    Number.isFinite(range.confidence) &&
    range.sourceOut > range.sourceIn
  );
}
