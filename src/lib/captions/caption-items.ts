import type { ProjectAction, Transcript, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { numberProperty, stringProperty } from "@/lib/timeline-ops/item-properties";

export type CaptionStylePreset = "boldReadableLower" | "kineticFocus" | "centeredMinimal";
export type CaptionPlacement = "lower" | "center" | "upper";
export type CaptionMotionPreset = "snap-pop-v1" | "pulse-emphasis-v2" | "soft-depth-card-v2";
export interface CaptionWordTiming {
  wordIndex: number;
  startSeconds: number;
  endSeconds: number;
}
type CaptionWordEasing = "linear" | "outQuad" | "outBack";
export type CaptionWordAnimationPreset = "sequentialPop" | "groupPulse" | "karaokeFade";
export interface CaptionWordAnimation {
  wordIndex: number;
  enterStartSeconds: number;
  enterEndSeconds: number;
  holdEndSeconds: number;
  exitEndSeconds: number;
  emphasisScale: number;
  emphasisColor: string;
  emphasisOpacity: number;
  easing: CaptionWordEasing;
}

export function captionWordTokens(text: string) {
  return text.match(/\S+/g) ?? [];
}

export function captionWordTimings(
  item: TimelineItem | null,
  wordCount: number,
): CaptionWordTiming[] {
  const durationSeconds = item?.durationSeconds ?? 0;
  const stored = item?.properties.captionWordTimings;
  const byIndex = new Map<number, CaptionWordTiming>();
  if (Array.isArray(stored)) {
    for (const timing of stored) {
      if (
        timing &&
        typeof timing === "object" &&
        "wordIndex" in timing &&
        "startSeconds" in timing &&
        "endSeconds" in timing &&
        typeof timing.wordIndex === "number" &&
        typeof timing.startSeconds === "number" &&
        typeof timing.endSeconds === "number" &&
        Number.isInteger(timing.wordIndex) &&
        timing.wordIndex >= 0 &&
        timing.wordIndex < wordCount &&
        timing.startSeconds >= 0 &&
        timing.endSeconds > timing.startSeconds &&
        timing.endSeconds <= durationSeconds
      ) {
        byIndex.set(timing.wordIndex, timing);
      }
    }
  }
  return Array.from({ length: wordCount }, (_, wordIndex) => {
    const fallbackStart = durationSeconds * (wordIndex / wordCount);
    const fallbackEnd = durationSeconds * ((wordIndex + 1) / wordCount);
    return byIndex.get(wordIndex) ?? {
      wordIndex,
      startSeconds: Number(fallbackStart.toFixed(3)),
      endSeconds: Number(fallbackEnd.toFixed(3)),
    };
  });
}

export interface CaptionBuildRange {
  startSeconds: number;
  endSeconds: number;
  timelineStartSeconds?: number;
  timelineSecondsPerSourceSecond?: number;
}

export interface CaptionBuildOptions {
  transcript: Transcript;
  range: CaptionBuildRange;
  wordsPerCue: number;
  stylePreset: CaptionStylePreset;
  groupId: string;
}

export const captionStyleDetails: Record<
  CaptionStylePreset,
  Pick<TimelineItem["properties"], "visualTreatment" | "motion" | "safeZone" | "avoid">
> = {
  boldReadableLower: {
    visualTreatment:
      "bold phone-readable lower-third caption with a shaped translucent backing and accent stroke",
    motion: "quick upward pop-in, short hold, and soft fade out",
    safeZone: "keep essential text inside 10% margins and above the lower safe area",
    avoid: "full-width opaque black slabs, faces, hands, and main action",
  },
  kineticFocus: {
    visualTreatment:
      "kinetic emphasized caption with selective word scale, restrained highlight color, and transparent backing",
    motion: "word-aware scale and tracking accent on the stressed phrase, then a short ease out",
    safeZone: "keep essential text inside 10% margins and away from faces or product details",
    avoid: "static text-only cards, default-font template looks, and long unmoving holds",
  },
  centeredMinimal: {
    visualTreatment:
      "compact centered caption with soft shadow, subtle material backing, and generous breathing room",
    motion: "brief fade and 2% scale settle on entry, then a soft fade out",
    safeZone: "keep essential text inside 12% margins and clear of the primary action",
    avoid: "opaque boxes, edge-to-edge text, and covering faces, hands, or product details",
  },
};

export function captionStyleProperties(stylePreset: CaptionStylePreset): TimelineItem["properties"] {
  return {
    stylePreset,
    motionPresetId:
      stylePreset === "kineticFocus"
        ? "pulse-emphasis-v2"
        : stylePreset === "centeredMinimal"
          ? "soft-depth-card-v2"
          : "snap-pop-v1",
    ...captionStyleDetails[stylePreset],
  };
}

export function buildCaptionItems({
  transcript,
  range,
  wordsPerCue,
  stylePreset,
  groupId,
}: CaptionBuildOptions): TimelineItem[] {
  const normalizedWordsPerCue = Math.min(12, Math.max(1, Math.round(wordsPerCue)));
  const words = transcript.words.filter(
    (word) =>
      word.endSeconds > range.startSeconds && word.startSeconds < range.endSeconds,
  );

  return Array.from(
    { length: Math.ceil(words.length / normalizedWordsPerCue) },
    (_, chunkIndex) => {
      const firstWordIndex = chunkIndex * normalizedWordsPerCue;
      const chunk = words.slice(firstWordIndex, firstWordIndex + normalizedWordsPerCue);
      const first = chunk[0];
      const last = chunk.at(-1);
      if (!first || !last) {
        throw new Error("Caption builder received an empty word chunk.");
      }
      const sourceStartSeconds = Math.max(range.startSeconds, first.startSeconds);
      const sourceEndSeconds = Math.min(range.endSeconds, last.endSeconds);
      const timelineStartSeconds =
        (range.timelineStartSeconds ?? range.startSeconds) +
        (sourceStartSeconds - range.startSeconds) *
          (range.timelineSecondsPerSourceSecond ?? 1);
      const timelineEndSeconds =
        (range.timelineStartSeconds ?? range.startSeconds) +
        (sourceEndSeconds - range.startSeconds) *
          (range.timelineSecondsPerSourceSecond ?? 1);
      return {
        id: `caption-${groupId}-${chunkIndex + 1}`,
        kind: "caption",
        startSeconds: Number(timelineStartSeconds.toFixed(3)),
        durationSeconds: Number((timelineEndSeconds - timelineStartSeconds).toFixed(3)),
        source: { type: "text", text: chunk.map((word) => word.text).join(" ") },
        label: `Caption ${chunkIndex + 1}`,
        properties: {
          transcriptId: transcript.id,
          wordStartIndex: transcript.words.indexOf(first),
          wordEndIndex: transcript.words.indexOf(last),
          captionWordTimings: chunk.map((word, wordIndex) => ({
            wordIndex,
            startSeconds: Number(
              ((Math.max(word.startSeconds, sourceStartSeconds) - sourceStartSeconds) *
                (range.timelineSecondsPerSourceSecond ?? 1)).toFixed(3),
            ),
            endSeconds: Number(
              ((Math.min(word.endSeconds, sourceEndSeconds) - sourceStartSeconds) *
                (range.timelineSecondsPerSourceSecond ?? 1)).toFixed(3),
            ),
          })),
          sourceIn: sourceStartSeconds,
          sourceOut: sourceEndSeconds,
          captionGroupId: groupId,
          ...captionStyleProperties(stylePreset),
          textEdited: false,
        },
      };
    },
  );
}

/** Clip playback speed; missing or invalid speeds fall back to 1. */
function playbackSpeed(item: TimelineItem) {
  const speed = numberProperty(item, "speed");
  return speed !== null && speed > 0 ? speed : 1;
}

export function captionBuildRangeForSourceItem(item: TimelineItem): CaptionBuildRange {
  const sourceStartSeconds = numberProperty(item, "sourceIn") ?? 0;
  const configuredSourceEndSeconds = numberProperty(item, "sourceOut");
  const sourceEndSeconds =
    configuredSourceEndSeconds && configuredSourceEndSeconds > sourceStartSeconds
      ? configuredSourceEndSeconds
      : sourceStartSeconds + item.durationSeconds * playbackSpeed(item);
  const sourceDurationSeconds = sourceEndSeconds - sourceStartSeconds;

  return {
    startSeconds: sourceStartSeconds,
    endSeconds: sourceEndSeconds,
    timelineStartSeconds: item.startSeconds,
    timelineSecondsPerSourceSecond:
      sourceDurationSeconds > 0 ? item.durationSeconds / sourceDurationSeconds : 1,
  };
}

/** How far a cue's start or end may sit from its transcript word and still count as that word. */
const captionRepairTimingToleranceSeconds = 1e-3;

/**
 * `applyCaptionRepair` for a caption cue, or null when a repair is not valid for it. A repair
 * replaces one transcript word and retimes the cue to it, so it is only built when the cue is
 * exactly one transcript word: a `wordIndex` link (and no wider `wordStartIndex`–`wordEndIndex`
 * range) to a word that exists, with the cue's timing within 1 ms of that word, and the new text is
 * a single word. The repair takes the word's own timing. Otherwise callers use `editCaptionText`,
 * which keeps the cue's timing and the transcript, like the Captions panel word fix.
 */
export function captionRepairActionForItem(
  item: TimelineItem,
  text: string,
  project: Pick<VideoProject, "transcripts">,
): ProjectAction | null {
  const transcriptId = stringProperty(item, "transcriptId");
  const wordIndex = numberProperty(item, "wordIndex");
  if (!transcriptId || wordIndex === null || !Number.isInteger(wordIndex) || wordIndex < 0) {
    return null;
  }
  const wordStartIndex = numberProperty(item, "wordStartIndex");
  const wordEndIndex = numberProperty(item, "wordEndIndex");
  if ((wordStartIndex !== null && wordStartIndex !== wordIndex) || (wordEndIndex !== null && wordEndIndex !== wordIndex)) {
    return null;
  }
  const word = project.transcripts.find((transcript) => transcript.id === transcriptId)?.words[wordIndex];
  if (!word || captionWordTokens(text).length !== 1) return null;

  const cueEndSeconds = item.startSeconds + item.durationSeconds;
  if (
    Math.abs(item.startSeconds - word.startSeconds) > captionRepairTimingToleranceSeconds ||
    Math.abs(cueEndSeconds - word.endSeconds) > captionRepairTimingToleranceSeconds
  ) {
    return null;
  }

  return {
    type: "applyCaptionRepair",
    repair: {
      captionItemId: item.id,
      transcriptId,
      wordIndex,
      text,
      startSeconds: word.startSeconds,
      endSeconds: word.endSeconds,
      repairId: `caption-repair-${item.id}-${Date.now().toString(36)}`,
      createdAt: new Date().toISOString(),
    },
  };
}
