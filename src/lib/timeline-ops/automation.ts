import type { ProjectActionKeyframe, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import {
  numberProperty,
  timelineItemSourceMediaId as sourceMediaId,
} from "@/lib/timeline-ops/item-properties";

const sourceDurationToleranceSeconds = 0.01;

export function suppliedWaveformPeaks(item: TimelineItem) {
  const peaks = item.properties.waveformPeaks;
  if (!Array.isArray(peaks)) {
    return null;
  }

  const normalizedPeaks = peaks.flatMap((peak) =>
    typeof peak === "number" && Number.isFinite(peak)
      ? [Math.min(Math.max(peak, 0), 1)]
      : [],
  );

  return normalizedPeaks.length > 0 ? normalizedPeaks : null;
}

/** Video clips carry audio when the source media is a video (or generated) file or peaks are stored. */
export function timelineItemHasAudio(project: VideoProject, item: TimelineItem): boolean {
  if (item.kind === "audio_clip") return true;
  if (item.kind !== "video_clip") return false;
  if (suppliedWaveformPeaks(item)) return true;
  const mediaId = sourceMediaId(item);
  const kind = project.media.find((media) => media.id === mediaId)?.kind;
  return kind === "video" || kind === "generated";
}

function fallbackWaveformPeaks(item: TimelineItem) {
  const seed = Array.from(item.id).reduce(
    (total, character) => total + character.charCodeAt(0),
    Math.round(item.durationSeconds * 100),
  );

  return Array.from({ length: 24 }, (_, index) => {
    const wave = Math.sin((seed + index * 17) * 0.45);
    const pulse = Math.cos((seed + index * 7) * 0.18);

    return Number((0.22 + Math.abs(wave * pulse) * 0.72).toFixed(2));
  });
}

export function waveformPeaks(item: TimelineItem) {
  return suppliedWaveformPeaks(item) ?? fallbackWaveformPeaks(item);
}

export function overviewWaveformPeaks(value: unknown): readonly number[] {
  return Array.isArray(value)
    ? value.filter(
        (peak): peak is number => typeof peak === "number" && Number.isFinite(peak),
      )
    : [];
}

export function timelineAutomationPoints(item: TimelineItem, property: "opacity" | "volumeDb") {
  const keyframes = item.properties.keyframes;
  if (!keyframes || typeof keyframes !== "object" || Array.isArray(keyframes)) return [];
  const stored = (keyframes as Record<string, unknown>)[property];
  if (!Array.isArray(stored)) return [];
  return stored
    .flatMap((point) => {
      if (!point || typeof point !== "object" || Array.isArray(point)) return [];
      const record = point as Record<string, unknown>;
      return typeof record.atSeconds === "number" &&
        Number.isFinite(record.atSeconds) &&
        record.atSeconds >= 0 &&
        record.atSeconds <= item.durationSeconds &&
        typeof record.value === "number" &&
        Number.isFinite(record.value)
        ? [{
            atSeconds: record.atSeconds,
            value: record.value,
            easing: typeof record.easing === "string"
              ? record.easing as ProjectActionKeyframe["easing"]
              : undefined,
          }]
        : [];
    })
    .sort((left, right) => left.atSeconds - right.atSeconds);
}

export function automationValueBounds(property: "opacity" | "volumeDb") {
  return property === "opacity"
    ? { minimum: 0, maximum: 1, step: 0.05 }
    : { minimum: -60, maximum: 24, step: 0.5 };
}

/**
 * Percent position of a keyframe inside its clip: `left` along the clip duration, `top` from
 * the maximum value down. Pass a lane property or explicit value bounds (keyframe configs).
 */
export function automationPointPosition(
  item: TimelineItem,
  propertyOrBounds: "opacity" | "volumeDb" | { readonly minimum: number; readonly maximum: number },
  point: Pick<ProjectActionKeyframe, "atSeconds" | "value">,
) {
  const bounds = typeof propertyOrBounds === "string" ? automationValueBounds(propertyOrBounds) : propertyOrBounds;
  const normalizedValue = Math.min(
    1,
    Math.max(0, (point.value - bounds.minimum) / (bounds.maximum - bounds.minimum)),
  );
  return {
    left: Number(((point.atSeconds / item.durationSeconds) * 100).toFixed(2)),
    top: Number(((1 - normalizedValue) * 100).toFixed(2)),
  };
}

export function sourceBoundaryRange(item: TimelineItem) {
  const sourceIn = numberProperty(item, "sourceIn");
  const sourceOut = numberProperty(item, "sourceOut");
  if (
    sourceMediaId(item) === null ||
    sourceIn === null ||
    sourceOut === null ||
    sourceOut <= sourceIn
  ) {
    return null;
  }

  return { sourceIn, sourceOut };
}

/** Clip playback speed; missing or invalid speeds fall back to 1. */
function playbackSpeed(item: TimelineItem) {
  const speed = numberProperty(item, "speed");
  return speed !== null && speed > 0 ? speed : 1;
}

/** Reports when `sourceOut - sourceIn` differs from `durationSeconds * speed`. */
export function sourceRangeDurationMismatch(item: TimelineItem) {
  const range = sourceBoundaryRange(item);
  if (!range) {
    return null;
  }

  const sourceSpan = Number((range.sourceOut - range.sourceIn).toFixed(3));
  const expectedSourceSpan = item.durationSeconds * playbackSpeed(item);
  if (Math.abs(sourceSpan - expectedSourceSpan) <= sourceDurationToleranceSeconds) {
    return null;
  }

  return {
    sourceSpan,
    clipDuration: item.durationSeconds,
  };
}
