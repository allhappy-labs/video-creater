import type { Timeline, TimelineTrack, TrackKind } from "@/lib/timeline";
import { formatTimecode, roundTimelineSeconds as roundedSeconds } from "@/lib/format";
import { clampPlayheadSecondsForTimeline as clampPlayheadSeconds } from "@/lib/timeline-ops/item-properties";

const defaultTrackDisplayHeight = 48;
const expandedTrackDisplayHeight = 64;
const minimumTrackDisplayHeight = 44;
const maximumTrackDisplayHeight = 200;
export const basePixelsPerSecond = 80;
const minimumZoomPercent = 50;
const maximumZoomPercent = 200;
export const timelineSnapSeconds = 0.25;
const minimumOverviewWindowSeconds = 0.25;
const minimumWindowSeconds = 0.25;

export interface TimelineOverviewWindow {
  startSeconds: number;
  endSeconds: number;
}

export type TimelineOverviewInteractionMode = "pan" | "resizeStart" | "resizeEnd";

export interface TimelineOverviewInteraction {
  mode: TimelineOverviewInteractionMode;
  pointerId: number;
  pointerStartX: number;
  overviewWidth: number;
  window: TimelineOverviewWindow;
  moved: boolean;
}

export interface TimelineRangeSelection {
  startSeconds: number;
  endSeconds: number;
}

export interface TimelineGapSelection {
  trackId: string;
  startSeconds: number;
  endSeconds: number;
}

export function timelineGapAtSeconds(
  track: TimelineTrack,
  seconds: number,
): TimelineGapSelection | null {
  if (track.items.some(
    (item) => seconds >= item.startSeconds && seconds < item.startSeconds + item.durationSeconds,
  )) return null;
  const nextStart = track.items
    .map((item) => item.startSeconds)
    .filter((startSeconds) => startSeconds > seconds)
    .sort((left, right) => left - right)[0];
  if (nextStart === undefined) return null;
  const previousEnd = track.items
    .map((item) => item.startSeconds + item.durationSeconds)
    .filter((endSeconds) => endSeconds <= seconds)
    .reduce((latest, endSeconds) => Math.max(latest, endSeconds), 0);
  if (nextStart <= previousEnd) return null;
  return { trackId: track.id, startSeconds: previousEnd, endSeconds: nextStart };
}

export interface TimelineRippleTrimRequest {
  itemId: string;
  edge: "left" | "right";
  deltaSeconds: number;
}

export function trackDisplayHeight(track: TimelineTrack) {
  const stored = (track as TimelineTrack & { displayHeight?: number }).displayHeight;
  return typeof stored === "number" && Number.isFinite(stored)
    ? clampTrackDisplayHeight(stored)
    : track.kind === "video" || track.kind === "audio"
      ? expandedTrackDisplayHeight
      : defaultTrackDisplayHeight;
}

export function clampTrackDisplayHeight(value: number) {
  return Math.min(maximumTrackDisplayHeight, Math.max(minimumTrackDisplayHeight, value));
}

export function buildTimelineTrackGeometry(tracks: readonly TimelineTrack[]) {
  let top = 0;
  const entries = tracks.map((track, index) => {
    const height = trackDisplayHeight(track);
    const entry = { trackId: track.id, index, top, height, bottom: top + height };
    top += height;
    return entry;
  });
  const byId = new Map(entries.map((entry) => [entry.trackId, entry]));
  const trackIndexAtY = (y: number) => {
    if (entries.length === 0) return -1;
    const clampedY = Math.min(Math.max(y, 0), Math.max(0, top - 0.001));
    return entries.find((entry) => clampedY >= entry.top && clampedY < entry.bottom)?.index
      ?? entries.length - 1;
  };
  return { entries, byId, totalHeight: top, trackIndexAtY };
}

export function formatTimestamp(totalSeconds: number) {
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = Math.floor(totalSeconds % 60);

  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
}

export function timelineEditPointSeconds(timeline: Timeline) {
  const points = [0, timeline.durationSeconds];

  for (const track of timeline.tracks) {
    for (const item of track.items) {
      points.push(item.startSeconds);
      points.push(item.startSeconds + item.durationSeconds);
    }
  }

  return Array.from(
    new Set(
      points.map((point) =>
        clampPlayheadSeconds(point, timeline.durationSeconds),
      ),
    ),
  ).sort((first, second) => first - second);
}

export function previousTimelineEditPoint(
  editPoints: number[],
  currentSeconds: number,
) {
  for (let index = editPoints.length - 1; index >= 0; index -= 1) {
    const editPoint = editPoints[index];
    if (editPoint !== undefined && editPoint < currentSeconds - 0.001) {
      return editPoint;
    }
  }

  return null;
}

export function nextTimelineEditPoint(editPoints: number[], currentSeconds: number) {
  return (
    editPoints.find((editPoint) => editPoint > currentSeconds + 0.001) ?? null
  );
}

export function clampZoomPercent(zoomPercent: number) {
  if (!Number.isFinite(zoomPercent)) {
    return 100;
  }

  return Math.min(
    Math.max(zoomPercent, minimumZoomPercent),
    maximumZoomPercent,
  );
}

export function resolveTimelineOverviewViewState(input: {
  currentWindow: TimelineOverviewWindow;
  requestedWindow: TimelineOverviewWindow;
  durationSeconds: number;
  viewportWidth: number;
}) {
  const durationSeconds = Number.isFinite(input.durationSeconds)
    ? Math.max(0, input.durationSeconds)
    : 0;
  const viewportWidth = Number.isFinite(input.viewportWidth)
    ? Math.max(0, input.viewportWidth)
    : 0;
  const currentStart = Number.isFinite(input.currentWindow.startSeconds)
    ? input.currentWindow.startSeconds
    : 0;
  const currentEnd = Number.isFinite(input.currentWindow.endSeconds)
    ? input.currentWindow.endSeconds
    : currentStart;
  const requestedStart = Number.isFinite(input.requestedWindow.startSeconds)
    ? input.requestedWindow.startSeconds
    : currentStart;
  const requestedEnd = Number.isFinite(input.requestedWindow.endSeconds)
    ? input.requestedWindow.endSeconds
    : currentEnd;
  const requestedDuration = Math.max(
    minimumOverviewWindowSeconds,
    requestedEnd - requestedStart,
  );
  const requestedPixelsPerSecond = viewportWidth / requestedDuration;
  const zoomPercent = clampZoomPercent(
    (requestedPixelsPerSecond / basePixelsPerSecond) * 100,
  );
  const pixelsPerSecond = basePixelsPerSecond * (zoomPercent / 100);
  const realizableDuration = Math.min(
    durationSeconds,
    viewportWidth / pixelsPerSecond,
  );
  const startChanged = Math.abs(requestedStart - currentStart) > 0.0005;
  const endChanged = Math.abs(requestedEnd - currentEnd) > 0.0005;

  let anchoredStart = currentStart;
  if (startChanged && !endChanged) {
    anchoredStart = requestedEnd - realizableDuration;
  } else if (!startChanged && endChanged) {
    anchoredStart = requestedStart;
  } else if (startChanged && endChanged) {
    anchoredStart = (requestedStart + requestedEnd - realizableDuration) / 2;
  }

  const maximumStart = Math.max(0, durationSeconds - realizableDuration);
  const startSeconds = Math.min(maximumStart, Math.max(0, anchoredStart));
  return {
    zoomPercent,
    scrollLeft: Number((startSeconds * pixelsPerSecond).toFixed(3)),
    window: {
      startSeconds: Number(startSeconds.toFixed(3)),
      endSeconds: Number((startSeconds + realizableDuration).toFixed(3)),
    },
  };
}

export function snapTimelineSeconds(seconds: number) {
  if (!Number.isFinite(seconds)) {
    return 0;
  }

  return Number(
    (Math.round(seconds / timelineSnapSeconds) * timelineSnapSeconds).toFixed(3),
  );
}

export function trackEnabled(track: TimelineTrack | undefined) {
  return track?.enabled ?? true;
}

export function trackLanePrefix(kind: TrackKind) {
  switch (kind) {
    case "video":
      return "V";
    case "audio":
      return "A";
    case "caption":
      return "C";
    case "overlay":
      return "O";
    case "hyperframe_scene":
      return "H";
  }
}

export function trackLaneLabels(tracks: readonly TimelineTrack[]) {
  const counts: Record<TrackKind, number> = {
    audio: 0,
    caption: 0,
    hyperframe_scene: 0,
    overlay: 0,
    video: 0,
  };

  return new Map(
    tracks.map((track) => {
      counts[track.kind] += 1;
      return [track.id, `${trackLanePrefix(track.kind)}${counts[track.kind]}`] as const;
    }),
  );
}

export function trackKindAccentLabel(kind: TrackKind) {
  switch (kind) {
    case "video":
      return "video";
    case "hyperframe_scene":
      return "HyperFrames";
    case "overlay":
      return "overlay";
    case "caption":
      return "caption";
    case "audio":
      return "audio";
  }
}

interface TimelineInteractionErrorFeedback {
  displayMessage: string;
  itemId: string | null;
  nativeMessage: string;
  targetTrackId: string | null;
}

export function timelineInteractionErrorFeedback(
  nativeMessage: string,
): TimelineInteractionErrorFeedback | null {
  const trimmedMessage = nativeMessage.trim();
  if (!trimmedMessage) return null;

  const overlapMatch = trimmedMessage.match(
    /^timeline\s+items\s+overlap\s+on\s+track\s+(.+?)\s*:\s*(.+?)\s+overlaps\s+(.+?)\s*$/i,
  );
  const targetTrackId = overlapMatch?.[1]?.trim() ?? null;
  const itemId = overlapMatch?.[2]?.trim() ?? null;
  const blockingItemId = overlapMatch?.[3]?.trim() ?? null;

  return {
    displayMessage: blockingItemId
      ? `Overlaps ${blockingItemId}`
      : trimmedMessage,
    itemId,
    nativeMessage,
    targetTrackId,
  };
}

export function isEditableKeyboardTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return (
    target.isContentEditable ||
    target.tagName === "INPUT" ||
    target.tagName === "SELECT" ||
    target.tagName === "TEXTAREA"
  );
}

export function finiteOr(value: number, fallback: number) {
  return Number.isFinite(value) ? value : fallback;
}

export function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(maximum, Math.max(minimum, value));
}

export function normalizedDuration(durationSeconds: number) {
  return Math.max(0, finiteOr(durationSeconds, 0));
}

export function normalizeWindow(
  window: TimelineOverviewWindow,
  durationSeconds: number,
): TimelineOverviewWindow {
  const duration = normalizedDuration(durationSeconds);
  if (duration === 0) {
    return { startSeconds: 0, endSeconds: 0 };
  }

  const minimumDuration = Math.min(minimumWindowSeconds, duration);
  const requestedStart = finiteOr(window.startSeconds, 0);
  const requestedEnd = finiteOr(window.endSeconds, requestedStart + minimumDuration);
  const windowDuration = clamp(
    requestedEnd - requestedStart,
    minimumDuration,
    duration,
  );
  const startSeconds = clamp(requestedStart, 0, duration - windowDuration);

  return {
    startSeconds: roundedSeconds(startSeconds),
    endSeconds: roundedSeconds(startSeconds + windowDuration),
  };
}

export function interactionWindow(
  interaction: TimelineOverviewInteraction,
  clientX: number,
  durationSeconds: number,
): TimelineOverviewWindow | null {
  const duration = normalizedDuration(durationSeconds);
  if (duration === 0) return null;

  const deltaSeconds =
    ((clientX - interaction.pointerStartX) / interaction.overviewWidth) * duration;
  const currentWindowDuration =
    interaction.window.endSeconds - interaction.window.startSeconds;

  if (interaction.mode === "pan") {
    return normalizeWindow(
      {
        startSeconds: interaction.window.startSeconds + deltaSeconds,
        endSeconds:
          interaction.window.startSeconds + deltaSeconds + currentWindowDuration,
      },
      duration,
    );
  }

  if (interaction.mode === "resizeStart") {
    return {
      startSeconds: roundedSeconds(
        clamp(
          interaction.window.startSeconds + deltaSeconds,
          0,
          interaction.window.endSeconds - Math.min(minimumWindowSeconds, duration),
        ),
      ),
      endSeconds: interaction.window.endSeconds,
    };
  }

  return {
    startSeconds: interaction.window.startSeconds,
    endSeconds: roundedSeconds(
      clamp(
        interaction.window.endSeconds + deltaSeconds,
        interaction.window.startSeconds + Math.min(minimumWindowSeconds, duration),
        duration,
      ),
    ),
  };
}

export function windowsMatch(
  first: TimelineOverviewWindow,
  second: TimelineOverviewWindow,
) {
  return (
    Math.abs(first.startSeconds - second.startSeconds) <= 0.0005 &&
    Math.abs(first.endSeconds - second.endSeconds) <= 0.0005
  );
}

const badgeCharacterWidthPixels = 6;
const badgeHorizontalChromePixels = 14;

export function timelineBadgeLayout(input: {
  label: string;
  seconds: number;
  pixelsPerSecond: number;
  visibleLeft: number;
  visibleRight: number;
}) {
  const viewportWidth = Math.max(0, input.visibleRight - input.visibleLeft);
  const preferredWidth = input.label.length * badgeCharacterWidthPixels
    + badgeHorizontalChromePixels;
  const width = Math.min(viewportWidth, preferredWidth);
  const maximumLeft = Math.max(input.visibleLeft, input.visibleRight - width);

  return {
    left: clamp(
      input.seconds * input.pixelsPerSecond,
      input.visibleLeft,
      maximumLeft,
    ),
    width,
  };
}

export function renderedBadgeSeparation(
  first: { left: number; width: number },
  second: { left: number; width: number },
) {
  return Math.max(
    first.left - (second.left + second.width),
    second.left - (first.left + first.width),
    0,
  );
}

type TimelineItemDensity = "accent" | "title" | "timing" | "rich";

export function timelineItemDensity(pixelWidth: number): TimelineItemDensity {
  if (pixelWidth < 48) return "accent";
  if (pixelWidth < 120) return "title";
  if (pixelWidth < 220) return "timing";
  return "rich";
}

export function formatInteractionTimecode(totalSeconds: number) {
  const timecode = formatTimecode(totalSeconds);
  return timecode.includes(".") ? timecode : `${timecode}.000`;
}
