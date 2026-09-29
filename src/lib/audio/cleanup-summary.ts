interface TimelineRange {
  readonly startSeconds: number;
  readonly endSeconds: number;
}

export interface SilenceSummary {
  readonly count: number;
  readonly secondsSaved: number;
}

/**
 * "N silences · X s saved" for the Remove silences card. `count` is the number of reviewed ranges;
 * `secondsSaved` is the timeline time the ripple delete removes, counting overlapping ranges (such as
 * linked video and audio tracks) once, rounded to 0.1 s.
 */
export function silenceSummary(ranges: readonly TimelineRange[]): SilenceSummary {
  const valid = [...ranges]
    .filter(
      (range) =>
        Number.isFinite(range.startSeconds) &&
        Number.isFinite(range.endSeconds) &&
        range.endSeconds > range.startSeconds,
    )
    .sort((left, right) => left.startSeconds - right.startSeconds);

  let total = 0;
  let merged: { startSeconds: number; endSeconds: number } | null = null;
  for (const range of valid) {
    if (merged && range.startSeconds <= merged.endSeconds) {
      merged.endSeconds = Math.max(merged.endSeconds, range.endSeconds);
      continue;
    }
    if (merged) total += merged.endSeconds - merged.startSeconds;
    merged = { startSeconds: range.startSeconds, endSeconds: range.endSeconds };
  }
  if (merged) total += merged.endSeconds - merged.startSeconds;

  // Timeline seconds are millisecond-precise; settle float noise there before rounding to 0.1 s.
  const milliseconds = Math.round(total * 1000);
  return { count: ranges.length, secondsSaved: Math.round(milliseconds / 100) / 10 };
}
