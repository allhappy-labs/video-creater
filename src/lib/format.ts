export function formatDurationLabel(totalSeconds: number) {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) {
    return "00:00";
  }

  const roundedSeconds = Math.round(totalSeconds);
  const minutes = Math.floor(roundedSeconds / 60);
  const seconds = roundedSeconds % 60;

  return `${minutes.toString().padStart(2, "0")}:${seconds
    .toString()
    .padStart(2, "0")}`;
}

export function formatDurationBadge(durationSeconds: number) {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) {
    return null;
  }

  const roundedSeconds = Math.round(durationSeconds);
  const minutes = Math.floor(roundedSeconds / 60);
  const seconds = roundedSeconds % 60;

  if (minutes === 0) {
    return `00:${seconds.toString().padStart(2, "0")}`;
  }

  return `${minutes.toString().padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;
}

export function formatTimelinePlacementTime(seconds: number) {
  const boundedSeconds = Number.isFinite(seconds) ? Math.max(0, seconds) : 0;
  const roundedSeconds = Math.round(boundedSeconds);
  const minutes = Math.floor(roundedSeconds / 60);
  const secondsPart = roundedSeconds % 60;

  return `${minutes.toString().padStart(2, "0")}:${secondsPart
    .toString()
    .padStart(2, "0")}`;
}

export function formatTimecode(totalSeconds: number) {
  if (!Number.isFinite(totalSeconds) || totalSeconds <= 0) {
    return "00:00:00";
  }

  const totalMilliseconds = Math.round(totalSeconds * 1000);
  const hours = Math.floor(totalMilliseconds / 3_600_000);
  const minutes = Math.floor((totalMilliseconds % 3_600_000) / 60_000);
  const seconds = Math.floor((totalMilliseconds % 60_000) / 1000);
  const milliseconds = totalMilliseconds % 1000;
  const wholeSecondsLabel = `${hours.toString().padStart(2, "0")}:${minutes
    .toString()
    .padStart(2, "0")}:${seconds.toString().padStart(2, "0")}`;

  if (milliseconds === 0) {
    return wholeSecondsLabel;
  }

  return `${wholeSecondsLabel}.${milliseconds.toString().padStart(3, "0")}`;
}

export function formatSecondsShort(seconds: number) {
  return `${seconds.toFixed(2)}s`;
}

export function pluralize(count: number, singular: string, plural = `${singular}s`) {
  return `${count} ${count === 1 ? singular : plural}`;
}

export function roundTimelineSeconds(seconds: number) {
  return Number(seconds.toFixed(3));
}

/**
 * Rounds with `Math.round` instead of `toFixed`, so half-millisecond inputs such as
 * 1.0005 round up where `roundTimelineSeconds` rounds down. Kept distinct on purpose.
 */
export function roundGenerationTimelineSeconds(seconds: number) {
  return Math.round(seconds * 1000) / 1000;
}

export function formatCredits(value: number) {
  return new Intl.NumberFormat("en-US").format(value);
}
