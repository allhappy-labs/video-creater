import type { VideoProject } from "@/lib/project";

export function timelineMediaItemId(project: VideoProject, mediaId: string) {
  const slug = mediaId
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  const baseId = `timeline-${slug || "media"}`;
  const existingItemIds = new Set(
    project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)),
  );
  if (!existingItemIds.has(baseId)) {
    return baseId;
  }

  let suffix = 2;
  while (existingItemIds.has(`${baseId}-${suffix.toString()}`)) {
    suffix += 1;
  }

  return `${baseId}-${suffix.toString()}`;
}

export function duplicateTimelineItemId(project: VideoProject, itemId: string) {
  const baseId = `${itemId}-copy`;
  const existingItemIds = new Set(
    project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)),
  );
  if (!existingItemIds.has(baseId)) {
    return baseId;
  }

  let suffix = 2;
  while (existingItemIds.has(`${baseId}-${suffix.toString()}`)) {
    suffix += 1;
  }

  return `${baseId}-${suffix.toString()}`;
}

/**
 * A transition id for the cut between `leftItemId` and `rightItemId`, unique across the transitions
 * of the active timeline and every timeline in the project library.
 */
export function timelineTransitionId(project: VideoProject, leftItemId: string, rightItemId: string) {
  const slug = `${leftItemId}-${rightItemId}`
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
  const baseId = `transition-${slug || "cut"}`;
  const timelines = [project.timeline, ...(project.timelines ?? []).map((entry) => entry.timeline)];
  const existingIds = new Set(
    timelines.flatMap((timeline) => timeline.tracks.flatMap((track) => (track.transitions ?? []).map((transition) => transition.id))),
  );
  if (!existingIds.has(baseId)) {
    return baseId;
  }

  let suffix = 2;
  while (existingIds.has(`${baseId}-${suffix.toString()}`)) {
    suffix += 1;
  }

  return `${baseId}-${suffix.toString()}`;
}
