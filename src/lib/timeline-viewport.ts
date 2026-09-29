export interface TimelineViewportWindow {
  visibleStartSeconds: number;
  visibleEndSeconds: number;
  renderStartSeconds: number;
  renderEndSeconds: number;
}

export interface TimelineTickSeries {
  majorSeconds: number[];
  minorSeconds: number[];
}

export interface VisibleTimelineTrackRange {
  firstTrackNumber: number;
  lastTrackNumber: number;
  totalTracks: number;
  label: string;
}

const majorStepCandidates = [0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300, 600] as const;
const maxRenderedTickElements = 400;
const timelineTickConsumers = 2;
const maxTickSeriesElements = maxRenderedTickElements / timelineTickConsumers;

export function timelineViewportWindow(input: {
  durationSeconds: number;
  pixelsPerSecond: number;
  scrollLeft: number;
  viewportWidth: number;
  overscanViewports?: number;
}): TimelineViewportWindow {
  const durationSeconds = Math.max(0, input.durationSeconds);
  const pixelsPerSecond = Math.max(0.001, input.pixelsPerSecond);
  const viewportSeconds = Math.max(0, input.viewportWidth) / pixelsPerSecond;
  const maxVisibleStartSeconds = Math.max(0, durationSeconds - viewportSeconds);
  const visibleStartSeconds = Math.min(maxVisibleStartSeconds, Math.max(0, input.scrollLeft / pixelsPerSecond));
  const visibleEndSeconds = Math.min(durationSeconds, visibleStartSeconds + viewportSeconds);
  const overscanSeconds = viewportSeconds * Math.max(0, input.overscanViewports ?? 1);

  return {
    visibleStartSeconds: Number(visibleStartSeconds.toFixed(3)),
    visibleEndSeconds: Number(visibleEndSeconds.toFixed(3)),
    renderStartSeconds: Number(Math.max(0, visibleStartSeconds - overscanSeconds).toFixed(3)),
    renderEndSeconds: Number(Math.min(durationSeconds, visibleEndSeconds + overscanSeconds).toFixed(3)),
  };
}

function ticksBetween(start: number, end: number, step: number) {
  const first = Math.ceil(start / step) * step;
  const values: number[] = [];

  for (let value = first; value <= end + 0.0001; value += step) {
    values.push(Number(value.toFixed(3)));
  }

  return values;
}

export function adaptiveTimelineTicks(input: {
  window: Pick<TimelineViewportWindow, "renderStartSeconds" | "renderEndSeconds">;
  pixelsPerSecond: number;
  snapSeconds: number;
}): TimelineTickSeries {
  const majorStep = majorStepCandidates.find((step) => (
    step * input.pixelsPerSecond >= 72
    && ticksBetween(input.window.renderStartSeconds, input.window.renderEndSeconds, step).length <= maxTickSeriesElements
  )) ?? 600;
  const majorSeconds = ticksBetween(input.window.renderStartSeconds, input.window.renderEndSeconds, majorStep);
  const majorSet = new Set(majorSeconds.map((seconds) => seconds.toFixed(3)));
  const minorTickCandidates = input.snapSeconds * input.pixelsPerSecond < 8
    ? []
    : ticksBetween(input.window.renderStartSeconds, input.window.renderEndSeconds, input.snapSeconds)
        .filter((seconds) => !majorSet.has(seconds.toFixed(3)));
  const minorSeconds = majorSeconds.length + minorTickCandidates.length <= maxTickSeriesElements
    ? minorTickCandidates
    : [];

  return { majorSeconds, minorSeconds };
}

export function timelineItemIntersectsWindow(
  item: { id: string; startSeconds: number; durationSeconds: number },
  window: Pick<TimelineViewportWindow, "renderStartSeconds" | "renderEndSeconds">,
  persistentItemIds: ReadonlySet<string>,
) {
  if (persistentItemIds.has(item.id)) return true;

  const endSeconds = item.startSeconds + item.durationSeconds;
  return endSeconds >= window.renderStartSeconds && item.startSeconds <= window.renderEndSeconds;
}

export function visibleTimelineTrackRange(input: {
  entries: readonly { top: number; bottom: number }[];
  scrollTop: number;
  viewportHeight: number;
}): VisibleTimelineTrackRange {
  const totalTracks = input.entries.length;
  if (totalTracks === 0) {
    return {
      firstTrackNumber: 0,
      lastTrackNumber: 0,
      totalTracks: 0,
      label: "No tracks",
    };
  }

  const visibleTop = Number.isFinite(input.scrollTop)
    ? Math.max(0, input.scrollTop)
    : 0;
  const visibleBottom = Number.isFinite(input.viewportHeight) && input.viewportHeight > 0
    ? visibleTop + input.viewportHeight
    : Number.POSITIVE_INFINITY;
  const firstIndex = input.entries.findIndex(
    (entry) => entry.bottom > visibleTop && entry.top < visibleBottom,
  );
  const resolvedFirstIndex = firstIndex >= 0 ? firstIndex : totalTracks - 1;
  let lastIndex = resolvedFirstIndex;
  for (let index = resolvedFirstIndex; index < totalTracks; index += 1) {
    const entry = input.entries[index];
    if (!entry || entry.top >= visibleBottom) break;
    if (entry.bottom > visibleTop) lastIndex = index;
  }

  const firstTrackNumber = resolvedFirstIndex + 1;
  const lastTrackNumber = lastIndex + 1;
  const label = firstTrackNumber === 1 && lastTrackNumber === totalTracks
    ? `${totalTracks} ${totalTracks === 1 ? "track" : "tracks"}`
    : `Tracks ${firstTrackNumber}–${lastTrackNumber} of ${totalTracks}`;

  return { firstTrackNumber, lastTrackNumber, totalTracks, label };
}
