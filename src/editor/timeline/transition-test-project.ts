import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack, TimelineTransition } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

/** A `media-1` clip (4 s source) starting at `startSeconds` and using `sourceIn`..`sourceIn + duration`. */
export function sourceClip(id: string, startSeconds: number, durationSeconds: number, sourceIn: number): TimelineItem {
  return {
    id,
    kind: "video_clip",
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: "media-1" },
    label: id,
    properties: { sourceIn, sourceOut: sourceIn + durationSeconds },
  };
}

/**
 * Test project at 24 fps with Video 1 ("v1") holding `videoItems` and an empty Audio 1 ("a1"). The
 * default is "a" 0–2 s and "b" 2–4 s split from the 4 s source: the cut at 2 s allows up to 2 s.
 * At 80 px/s after the 118 px header the cut sits at x = 278 and Video 1 spans y 0–58.
 */
export function transitionTestProject(
  videoItems: TimelineItem[] = [sourceClip("a", 0, 2, 0), sourceClip("b", 2, 2, 2)],
  transitions: TimelineTransition[] = [],
): VideoProject {
  const project = fixtureProject();
  const video: TimelineTrack = { id: "v1", name: "v1", kind: "video", locked: false, enabled: true, items: videoItems };
  const audio: TimelineTrack = { id: "a1", name: "a1", kind: "audio", locked: false, enabled: true, items: [] };
  project.timeline = {
    durationSeconds: 8,
    tracks: [transitions.length > 0 ? { ...video, transitions } : video, audio],
  };
  return project;
}

export function crossfade(durationSeconds = 0.5): TimelineTransition {
  return { id: "fade", leftItemId: "a", rightItemId: "b", kind: "crossfade", durationSeconds };
}

export function transitionsOf(project: VideoProject): TimelineTransition[] {
  return project.timeline.tracks.flatMap((track) => track.transitions ?? []);
}
