import type { ProjectTimeline, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";

/** Actions to apply, or user-facing copy; the shared clip command result. */
export type MultiTimelineResult = CommandResult;

export interface NestedSequenceItem {
  readonly trackId: string;
  readonly item: TimelineItem;
  readonly timelineId: string;
  /** Name of the referenced timeline, or null when it no longer exists. */
  readonly timelineName: string | null;
}

const missingTimelineReason = "That timeline no longer exists.";

/** Project timelines; legacy projects without `timelines` expose one implicit "Timeline 1". */
export function projectTimelineEntries(project: VideoProject): ProjectTimeline[] {
  return project.timelines?.length
    ? project.timelines
    : [{ id: project.activeTimelineId ?? "main", name: "Timeline 1", timeline: project.timeline }];
}

function activeTimelineId(project: VideoProject, entries: readonly ProjectTimeline[]) {
  return project.activeTimelineId ?? entries[0]?.id ?? "main";
}

/**
 * New timeline id `timeline-<count+1>` (collisions append -1, -2, ...). Name is
 * "Copy of <source name>" when duplicating (source defaults to the active timeline),
 * otherwise "Timeline <count+1>".
 */
export function planCreateTimeline(
  project: VideoProject,
  options: { readonly duplicate: boolean; readonly sourceTimelineId?: string },
): MultiTimelineResult {
  const entries = projectTimelineEntries(project);
  const count = entries.length;
  const base = `timeline-${(count + 1).toString()}`;
  const existingIds = new Set(entries.map((entry) => entry.id));
  let suffix = 1;
  let timelineId = base;
  while (existingIds.has(timelineId)) {
    timelineId = `${base}-${suffix.toString()}`;
    suffix += 1;
  }

  if (!options.duplicate) {
    return {
      actions: [
        { type: "createTimeline", timelineId, name: `Timeline ${(count + 1).toString()}`, duplicateActive: false },
      ],
    };
  }

  const sourceTimelineId = options.sourceTimelineId ?? activeTimelineId(project, entries);
  const source = entries.find((entry) => entry.id === sourceTimelineId);
  if (!source) return { blocked: missingTimelineReason };
  return {
    actions: [
      {
        type: "createTimeline",
        timelineId,
        name: `Copy of ${source.name}`,
        duplicateActive: true,
        sourceTimelineId,
      },
    ],
  };
}

/** Trims the name; blank names are blocked (legacy rename ignored them). */
export function planRenameTimeline(
  project: VideoProject,
  timelineId: string,
  name: string,
): MultiTimelineResult {
  if (!projectTimelineEntries(project).some((entry) => entry.id === timelineId)) {
    return { blocked: missingTimelineReason };
  }
  const trimmed = name.trim();
  if (trimmed.length === 0) return { blocked: "Enter a timeline name." };
  return { actions: [{ type: "renameTimeline", timelineId, name: trimmed }] };
}

/**
 * Blocks deleting the only timeline and timelines referenced as nested sequences by
 * another timeline (the reducer silently ignores both).
 */
export function planDeleteTimeline(project: VideoProject, timelineId: string): MultiTimelineResult {
  const entries = withLiveActiveTimeline(project);
  const target = entries.find((entry) => entry.id === timelineId);
  if (!target) return { blocked: missingTimelineReason };
  if (entries.length <= 1) return { blocked: "A project needs at least one timeline." };
  const referenced = entries.some(
    (entry) =>
      entry.id !== timelineId &&
      entry.timeline.tracks.some((track) =>
        track.items.some(
          (item) => item.source.type === "timeline" && item.source.timelineId === timelineId,
        ),
      ),
  );
  if (referenced) {
    return { blocked: `${target.name} is used as a nested sequence. Decompose or remove it first.` };
  }
  return { actions: [{ type: "deleteTimeline", timelineId }] };
}

/** Switching to the already active timeline is a no-op (no actions). */
export function planSetActiveTimeline(project: VideoProject, timelineId: string): MultiTimelineResult {
  const entries = projectTimelineEntries(project);
  if (!entries.some((entry) => entry.id === timelineId)) return { blocked: missingTimelineReason };
  if (timelineId === activeTimelineId(project, entries)) return { actions: [] };
  return { actions: [{ type: "setActiveTimeline", timelineId }] };
}

/** Items of the active timeline whose source is another timeline, in track then item order. */
export function nestedSequenceItems(project: VideoProject): NestedSequenceItem[] {
  const entries = projectTimelineEntries(project);
  const activeId = activeTimelineId(project, entries);
  return project.timeline.tracks.flatMap((track) =>
    track.items.flatMap((item) => {
      if (item.source.type !== "timeline" || item.source.timelineId === activeId) return [];
      const timelineId = item.source.timelineId;
      return [
        {
          trackId: track.id,
          item,
          timelineId,
          timelineName: entries.find((entry) => entry.id === timelineId)?.name ?? null,
        },
      ];
    }),
  );
}

/** Entries with the active entry's timeline replaced by the live `project.timeline`. */
function withLiveActiveTimeline(project: VideoProject): ProjectTimeline[] {
  const entries = projectTimelineEntries(project);
  const activeId = activeTimelineId(project, entries);
  return entries.map((entry) =>
    entry.id === activeId ? { ...entry, timeline: project.timeline } : entry,
  );
}
