import type { VideoProject } from "@/lib/project";
import { applyProjectActionLocally } from "@/lib/project";
import type { Timeline, TimelineItem, TimelineTrack, TimelineTransition } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

/**
 * Test fixtures equivalent to `src-tauri/tests/project_action/transitions.rs`, so the TypeScript
 * mirror is exercised against the same numbers and messages as the Rust actions.
 */

export const VIDEO_TRACK = "track-video";
export const AUDIO_TRACK = "track-audio";

/** A `media-1` clip whose source range starts at `sourceIn` and consumes `duration` seconds. */
export function clip(
  id: string,
  label: string,
  startSeconds: number,
  durationSeconds: number,
  sourceIn: number,
): TimelineItem {
  return {
    id,
    kind: "video_clip",
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: "media-1" },
    label,
    properties: { sourceIn, sourceOut: sourceIn + durationSeconds },
  };
}

function emptyTrack(id: string, name: string, kind: TimelineTrack["kind"]): TimelineTrack {
  return { id, name, kind, locked: false, enabled: true, items: [] };
}

/** Rust `sample_project`: a 12 s `media-1` video at 24 fps, with a video and an audio track. */
export function transitionSampleProject(videoItems: TimelineItem[] = []): VideoProject {
  const timeline: Timeline = {
    durationSeconds: 8,
    tracks: [{ ...emptyTrack(VIDEO_TRACK, "Video", "video"), items: videoItems }, emptyTrack(AUDIO_TRACK, "Audio", "audio")],
  };
  return {
    ...fixtureProject(),
    media: [
      {
        id: "media-1",
        relativePath: "media/input.mp4",
        kind: "video",
        durationSeconds: 12,
        width: 1920,
        height: 1080,
        fps: 24,
      },
    ],
    generatedAssets: [],
    transcripts: [],
    renderSettings: { width: 1920, height: 1080, fps: 24, loudnessLufs: -14, captions: "burn_in" },
    timeline,
    timelines: [{ id: "main", name: "Timeline 1", timeline }],
    activeTimelineId: "main",
  };
}

/**
 * Two 4 s clips of the 12 s `media-1` meeting at 4 s. Each side has 6 s of unused media at the
 * cut, so the maximum transition is limited to 4 s by the clip durations.
 */
export function cutProject(): VideoProject {
  return transitionSampleProject([
    clip("clip-a", "Opening shot", 0, 4, 2),
    clip("clip-b", "Closing shot", 4, 4, 6),
  ]);
}

export function transition(id: string, left: string, right: string, durationSeconds: number): TimelineTransition {
  return { id, leftItemId: left, rightItemId: right, kind: "crossfade", durationSeconds };
}

/** Replaces the active timeline's tracks, keeping the library entry in sync. */
export function withTracks(project: VideoProject, update: (tracks: TimelineTrack[]) => TimelineTrack[]): VideoProject {
  const timeline = { ...project.timeline, tracks: update(structuredClone(project.timeline.tracks)) };
  return {
    ...project,
    timeline,
    timelines: (project.timelines ?? []).map((entry) =>
      entry.id === (project.activeTimelineId ?? "main") ? { ...entry, timeline } : entry,
    ),
  };
}

/** Updates one item of the video track. */
export function withVideoItem(
  project: VideoProject,
  index: number,
  update: (item: TimelineItem) => TimelineItem,
): VideoProject {
  return withTracks(project, (tracks) =>
    tracks.map((track) =>
      track.id === VIDEO_TRACK
        ? { ...track, items: track.items.map((item, i) => (i === index ? update(item) : item)) }
        : track,
    ),
  );
}

export function withSourceRange(item: TimelineItem, sourceIn: number, sourceOut: number): TimelineItem {
  return { ...item, properties: { ...item.properties, sourceIn, sourceOut } };
}

export function trackOf(project: VideoProject, trackId: string): TimelineTrack {
  const found = project.timeline.tracks.find((track) => track.id === trackId);
  if (!found) throw new Error(`Missing track ${trackId}`);
  return found;
}

/** `cutProject` with a crossfade `fade-1` of `duration` on the clip-a | clip-b cut. */
export function projectWithCrossfade(durationSeconds = 1): VideoProject {
  const project = cutProject();
  const next = applyProjectActionLocally(project, {
    type: "addTransition",
    trackId: VIDEO_TRACK,
    transition: transition("fade-1", "clip-a", "clip-b", durationSeconds),
  });
  if (next === project) throw new Error("Expected the crossfade to be added");
  return next;
}
