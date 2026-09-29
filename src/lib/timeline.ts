import { isReversedItem, sourceSecondsAt, trimmedSourceRange } from "./timeline-ops/reverse";

export type TrackKind = "video" | "hyperframe_scene" | "overlay" | "caption" | "audio";

export type TimelineItemKind =
  | "video_clip"
  | "image_clip"
  | "lottie_clip"
  | "generated_clip"
  | "hyperframe_scene"
  | "overlay"
  | "caption"
  | "audio_clip";

type TimelineSource =
  | { type: "media"; mediaId: string }
  | { type: "generated"; artifactId: string }
  | { type: "timeline"; timelineId: string }
  | { type: "text"; text: string };

export interface TimelineItem {
  id: string;
  kind: TimelineItemKind;
  startSeconds: number;
  durationSeconds: number;
  source: TimelineSource;
  label: string;
  properties: Record<string, unknown>;
}

export interface TimelineTrack {
  id: string;
  name: string;
  kind: TrackKind;
  locked: boolean;
  syncLocked?: boolean;
  enabled?: boolean;
  items: TimelineItem[];
  /**
   * Clip-to-clip transitions centered on cuts between adjacent items on this track. Omitted when
   * empty, like Rust `TimelineTrack.transitions`.
   */
  transitions?: TimelineTransition[];
}

/** Mirrors Rust `TransitionKind` (camelCase on the wire). */
export type TransitionKind = "crossfade" | "dipToBlack" | "dipToWhite" | "wipe";

/** Mirrors Rust `TimelineTransition`. */
export interface TimelineTransition {
  id: string;
  leftItemId: string;
  rightItemId: string;
  kind: TransitionKind;
  durationSeconds: number;
}

export interface Timeline {
  durationSeconds: number;
  tracks: TimelineTrack[];
}

export interface TimelineRow {
  id: string;
  title: string;
}

export type TimelineMediaKind = "video" | "audio" | "image" | "lottie" | "generated";

export function canonicalTimelineItemKindForMediaKind(
  mediaKind: TimelineMediaKind,
): TimelineItemKind {
  switch (mediaKind) {
    case "audio":
      return "audio_clip";
    case "image":
      return "image_clip";
    case "lottie":
      return "lottie_clip";
    case "generated":
      return "generated_clip";
    case "video":
      return "video_clip";
  }
}

export function itemAllowedOnTrack(
  itemKind: TimelineItemKind,
  trackKind: TrackKind,
): boolean {
  return (
    ((itemKind === "video_clip" ||
      itemKind === "image_clip" ||
      itemKind === "lottie_clip" ||
      itemKind === "generated_clip") &&
      trackKind === "video") ||
    (itemKind === "hyperframe_scene" && trackKind === "hyperframe_scene") ||
    (itemKind === "overlay" && trackKind === "overlay") ||
    (itemKind === "caption" && trackKind === "caption") ||
    (itemKind === "audio_clip" && trackKind === "audio")
  );
}

export type TimelinePatch =
  | {
      type: "moveItem";
      itemId: string;
      targetTrackId: string;
      startSeconds: number;
    }
  | {
      type: "resizeItem";
      itemId: string;
      durationSeconds: number;
    }
  | {
      type: "trimItem";
      itemId: string;
      startSeconds: number;
      durationSeconds: number;
      sourceIn?: number;
      sourceOut?: number;
    }
  | {
      type: "editCaptionText";
      itemId: string;
      text: string;
    };

export const sampleTimeline: Timeline = {
  durationSeconds: 4,
  tracks: [
    {
      id: "track-video",
      name: "Video",
      kind: "video",
      locked: false,
      enabled: true,
      items: [
        {
          id: "item-1",
          kind: "video_clip",
          startSeconds: 0,
          durationSeconds: 4,
          source: { type: "media", mediaId: "media-1" },
          label: "Opening clip",
          properties: {},
        },
      ],
    },
    {
      id: "track-scenes",
      name: "HyperFrames",
      kind: "hyperframe_scene",
      locked: false,
      enabled: true,
      items: [],
    },
    {
      id: "track-overlays",
      name: "Overlays",
      kind: "overlay",
      locked: false,
      enabled: true,
      items: [],
    },
    {
      id: "track-captions",
      name: "Captions",
      kind: "caption",
      locked: false,
      enabled: true,
      items: [
        {
          id: "caption-1",
          kind: "caption",
          startSeconds: 0.65,
          durationSeconds: 1.35,
          source: { type: "text", text: "Original caption text" },
          label: "Caption 1",
          properties: {
            sourceIn: 0.65,
            sourceOut: 2,
            stylePreset: "boldReadableLower",
            visualTreatment:
              "bold phone-readable lower-third caption with subtle translucent backing",
            motion: "quick pop-in, hold, and soft fade out",
            safeZone: "keep essential text inside 10% margins",
            avoid: "full-width opaque black slabs, faces, hands, and main action",
            textEdited: false,
          },
        },
        {
          id: "caption-2",
          kind: "caption",
          startSeconds: 2.15,
          durationSeconds: 1.2,
          source: { type: "text", text: "Second clean split" },
          label: "Caption 2",
          properties: {
            sourceIn: 2.15,
            sourceOut: 3.35,
            stylePreset: "boldReadableLower",
            visualTreatment:
              "bold phone-readable lower-third caption with subtle translucent backing",
            motion: "quick pop-in, hold, and soft fade out",
            safeZone: "keep essential text inside 10% margins",
            avoid: "full-width opaque black slabs, faces, hands, and main action",
            textEdited: false,
          },
        },
      ],
    },
    {
      id: "track-audio",
      name: "Audio",
      kind: "audio",
      locked: false,
      enabled: true,
      items: [
        {
          id: "music-bed",
          kind: "audio_clip",
          startSeconds: 0,
          durationSeconds: 4,
          source: { type: "media", mediaId: "media-2" },
          label: "Music bed",
          properties: {
            sourceIn: 0,
            sourceOut: 4,
            fadeOutSeconds: 0.75,
            waveformPeaks: [
              0.22, 0.38, 0.3, 0.62, 0.48, 0.76, 0.42, 0.58, 0.8, 0.44, 0.54, 0.72,
              0.5, 0.34, 0.64, 0.86, 0.48, 0.7, 0.56, 0.42, 0.68, 0.58, 0.36, 0.24,
            ],
          },
        },
      ],
    },
  ],
};

export function buildTimelineRows(timeline: Timeline): TimelineRow[] {
  return timeline.tracks.map((track) => ({
    id: track.id,
    title: track.name,
  }));
}

export function createMovePatchFromDrag(input: {
  itemId: string;
  targetTrackId: string;
  startSeconds: number;
}): TimelinePatch {
  return {
    type: "moveItem",
    itemId: input.itemId,
    targetTrackId: input.targetTrackId,
    startSeconds: input.startSeconds,
  };
}

export function createEditCaptionTextPatch(input: {
  itemId: string;
  text: string;
}): TimelinePatch {
  return {
    type: "editCaptionText",
    itemId: input.itemId,
    text: input.text,
  };
}

export function createResizePatchFromDrag(input: {
  itemId: string;
  durationSeconds: number;
}): TimelinePatch {
  return {
    type: "resizeItem",
    itemId: input.itemId,
    durationSeconds: input.durationSeconds,
  };
}

function timelineNumberProperty(item: TimelineItem, key: string) {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function timelinePlaybackSpeed(item: TimelineItem) {
  const speed = timelineNumberProperty(item, "speed");
  return speed !== null && speed > 0 ? speed : 1;
}

function roundTimelineSeconds(value: number) {
  return Number(value.toFixed(3));
}

export function createTrimPatchFromSourceMark(
  item: TimelineItem,
  playheadSeconds: number,
  mark: "in" | "out",
): TimelinePatch | null {
  const itemEndSeconds = item.startSeconds + item.durationSeconds;
  if (
    !Number.isFinite(playheadSeconds) ||
    playheadSeconds <= item.startSeconds ||
    playheadSeconds >= itemEndSeconds
  ) {
    return null;
  }

  const speed = timelinePlaybackSpeed(item);
  const sourceIn = timelineNumberProperty(item, "sourceIn") ?? 0;
  const sourceOut =
    timelineNumberProperty(item, "sourceOut") ?? sourceIn + item.durationSeconds * speed;
  const reverse = isReversedItem(item);
  const sourceAtPlayhead = sourceSecondsAt(
    { sourceIn, sourceOut, speed, reverse },
    playheadSeconds - item.startSeconds,
  );
  // The kept part reads source between the playhead and the edge it keeps; reversed clips read
  // `sourceOut` at their start and `sourceIn` at their end.
  const [keptIn, keptOut] =
    mark === "in"
      ? reverse ? [sourceIn, sourceAtPlayhead] : [sourceAtPlayhead, sourceOut]
      : reverse ? [sourceAtPlayhead, sourceOut] : [sourceIn, sourceAtPlayhead];

  if (mark === "in") {
    return {
      type: "trimItem",
      itemId: item.id,
      startSeconds: roundTimelineSeconds(playheadSeconds),
      durationSeconds: roundTimelineSeconds(itemEndSeconds - playheadSeconds),
      sourceIn: roundTimelineSeconds(keptIn),
      sourceOut: roundTimelineSeconds(keptOut),
    };
  }

  return {
    type: "trimItem",
    itemId: item.id,
    startSeconds: roundTimelineSeconds(item.startSeconds),
    durationSeconds: roundTimelineSeconds(playheadSeconds - item.startSeconds),
    sourceIn: roundTimelineSeconds(keptIn),
    sourceOut: roundTimelineSeconds(keptOut),
  };
}

export function createLeftTrimPatchFromDrag(
  item: TimelineItem,
  nextStartSeconds: number,
  minimumDurationSeconds = 0.1,
): TimelinePatch {
  const originalEndSeconds = item.startSeconds + item.durationSeconds;
  const sourceIn = timelineNumberProperty(item, "sourceIn");
  const sourceOut = timelineNumberProperty(item, "sourceOut");
  const speed = timelinePlaybackSpeed(item);
  const reverse = isReversedItem(item);
  // A forward clip's left edge can extend back to source 0. A reversed clip's extends towards the
  // media end, which this patch does not know, so the trim action validates it.
  const minimumStartSeconds =
    sourceIn !== null && sourceOut !== null && !reverse
      ? Math.max(0, item.startSeconds - sourceIn / speed)
      : 0;
  const maximumStartSeconds = originalEndSeconds - minimumDurationSeconds;
  const clampedStartSeconds = Math.max(
    minimumStartSeconds,
    Math.min(nextStartSeconds, maximumStartSeconds),
  );
  const patch: TimelinePatch = {
    type: "trimItem",
    itemId: item.id,
    startSeconds: roundTimelineSeconds(clampedStartSeconds),
    durationSeconds: roundTimelineSeconds(originalEndSeconds - clampedStartSeconds),
  };

  if (sourceIn !== null && sourceOut !== null) {
    const [trimmedIn, trimmedOut] = trimmedSourceRange(
      { sourceIn, sourceOut, speed, reverse },
      "left",
      item.startSeconds - clampedStartSeconds,
    );
    patch.sourceIn = roundTimelineSeconds(Math.max(0, trimmedIn));
    patch.sourceOut = roundTimelineSeconds(trimmedOut);
  }

  return patch;
}

export function createRightTrimPatchFromDrag(
  item: TimelineItem,
  nextEndSeconds: number,
  minimumDurationSeconds = 0.1,
): TimelinePatch | null {
  if (item.source.type !== "media") {
    return null;
  }

  const sourceIn = timelineNumberProperty(item, "sourceIn");
  const sourceOut = timelineNumberProperty(item, "sourceOut");
  if (sourceIn === null || sourceOut === null) {
    return null;
  }

  const speed = timelinePlaybackSpeed(item);
  const reverse = isReversedItem(item);
  const minimumEndSeconds = item.startSeconds + minimumDurationSeconds;
  // A reversed clip's right edge extends towards source 0: at most `sourceIn / speed`.
  const maximumEndSeconds = reverse
    ? item.startSeconds + item.durationSeconds + sourceIn / speed
    : Number.POSITIVE_INFINITY;
  const clampedEndSeconds = Math.max(Math.min(nextEndSeconds, maximumEndSeconds), minimumEndSeconds);
  const durationSeconds = clampedEndSeconds - item.startSeconds;
  const [trimmedIn, trimmedOut] = trimmedSourceRange(
    { sourceIn, sourceOut, speed, reverse },
    "right",
    durationSeconds - item.durationSeconds,
  );

  return {
    type: "trimItem",
    itemId: item.id,
    startSeconds: roundTimelineSeconds(item.startSeconds),
    durationSeconds: roundTimelineSeconds(durationSeconds),
    sourceIn: roundTimelineSeconds(reverse ? Math.max(0, trimmedIn) : trimmedIn),
    sourceOut: roundTimelineSeconds(trimmedOut),
  };
}

export function createCaptionLeftResizePatches(input: {
  itemId: string;
  targetTrackId: string;
  originalStartSeconds: number;
  originalDurationSeconds: number;
  nextStartSeconds: number;
  minimumDurationSeconds: number;
}): TimelinePatch[] {
  const originalEndSeconds = input.originalStartSeconds + input.originalDurationSeconds;
  const clampedStartSeconds = Math.max(
    0,
    Math.min(input.nextStartSeconds, originalEndSeconds - input.minimumDurationSeconds),
  );
  const durationSeconds = originalEndSeconds - clampedStartSeconds;

  return [
    createMovePatchFromDrag({
      itemId: input.itemId,
      targetTrackId: input.targetTrackId,
      startSeconds: Number(clampedStartSeconds.toFixed(3)),
    }),
    createResizePatchFromDrag({
      itemId: input.itemId,
      durationSeconds: Number(durationSeconds.toFixed(3)),
    }),
  ];
}

export function getTimelineItemText(item: TimelineItem): string {
  return item.source.type === "text" ? item.source.text : "";
}

export function isTemplateTimelineItem(item: TimelineItem): boolean {
  return typeof item.properties.templateId === "string";
}

export function getCaptionReadingWarning(input: {
  text: string;
  durationSeconds: number;
}): string | null {
  const trimmed = input.text.trim();
  if (!trimmed) {
    return "Caption text cannot be empty.";
  }

  const characterCount = Array.from(trimmed).length;
  if (characterCount > 42) {
    return "Caption text is too long for one deterministic cue.";
  }

  const readingRate = characterCount / Math.max(input.durationSeconds, 0.1);
  if (readingRate > 24) {
    return "Caption text may be too dense for this cue duration.";
  }

  return null;
}
