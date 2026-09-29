import { describe, expect, it } from "vitest";
import {
  applyProjectActionLocally,
  type MediaAsset,
  type MediaKind,
  type ProjectAction,
  type VideoProject,
} from "@/lib/project";
import { transitionHandleSourceSeconds } from "@/lib/preview/transition-frame";
import {
  createLeftTrimPatchFromDrag,
  createRightTrimPatchFromDrag,
  createTrimPatchFromSourceMark,
  type TimelineItem,
} from "@/lib/timeline";
import { evaluateTimelineResize } from "@/lib/timeline-edit-evaluator";
import {
  handleSourceSeconds,
  headHandleSeconds,
  localSecondsForSource,
  sourceSecondsAt,
  sourceSubrange,
  tailHandleSeconds,
  timelineRangeForSource,
  trimmedSourceRange,
  type SourceWindow,
} from "./reverse";
import { AUDIO_TRACK, VIDEO_TRACK, transitionSampleProject, withTracks } from "./transition-fixtures";
import { transitionBounds } from "./transitions";

/** Mirrors `src-tauri/tests/project_action/reverse.rs` and `tests/render_pipeline/reverse.rs`. */

function clip(
  id: string,
  kind: "video_clip" | "audio_clip" | "image_clip",
  [startSeconds, durationSeconds]: [number, number],
  [sourceIn, sourceOut]: [number, number],
  speed: number,
  reverse: boolean,
): TimelineItem {
  return {
    id,
    kind,
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: kind === "audio_clip" ? "tone" : kind === "image_clip" ? "still" : "reel" },
    label: id,
    properties: {
      sourceIn,
      sourceOut,
      ...(speed !== 1 ? { speed } : {}),
      ...(reverse ? { reverse: true } : {}),
    },
  };
}

/** `rev`: 10-14 s on the timeline, source 2-10 at speed 2, reversed. */
const reversedClip = () => clip("rev", "video_clip", [10, 4], [2, 10], 2, true);

function projectWith(videoItems: TimelineItem[], audioItems: TimelineItem[] = []): VideoProject {
  const base = transitionSampleProject();
  const media = (id: string, kind: MediaKind): MediaAsset => ({
    id,
    relativePath: `media/${id}`,
    kind,
    durationSeconds: 20,
    width: null,
    height: null,
    fps: null,
  });
  return withTracks(
    { ...base, media: [...base.media, media("reel", "video"), media("tone", "audio"), media("still", "image")] },
    (tracks) =>
      tracks.map((track) =>
        track.id === VIDEO_TRACK ? { ...track, items: videoItems } : track.id === AUDIO_TRACK ? { ...track, items: audioItems } : track,
      ),
  );
}

function itemOf(project: VideoProject, id: string): TimelineItem {
  const found = project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === id);
  if (!found) throw new Error(`Missing item ${id}`);
  return found;
}

const sourceRange = (item: TimelineItem) => [item.properties.sourceIn, item.properties.sourceOut];

const window = (reverse: boolean): SourceWindow => ({ sourceIn: 2, sourceOut: 10, speed: 2, reverse });

const reverseAction = (itemId: string, reverse: boolean): ProjectAction => ({ type: "updateClipReverse", itemId, reverse });

describe("updateClipReverse", () => {
  it("sets and clears reverse on video and audio clips without changing timing", () => {
    const project = projectWith(
      [clip("forward", "video_clip", [0, 4], [2, 10], 2, false)],
      [clip("sound", "audio_clip", [0, 4], [0, 4], 1, false)],
    );
    let next = project;
    for (const id of ["forward", "sound"]) next = applyProjectActionLocally(next, reverseAction(id, true));
    expect(itemOf(next, "forward").properties.reverse).toBe(true);
    expect(itemOf(next, "sound").properties.reverse).toBe(true);
    expect(sourceRange(itemOf(next, "forward"))).toEqual([2, 10]);
    for (const id of ["forward", "sound"]) next = applyProjectActionLocally(next, reverseAction(id, false));
    expect(itemOf(next, "forward")).toEqual(itemOf(project, "forward"));
    expect(itemOf(next, "sound").properties).not.toHaveProperty("reverse");
  });

  it("rejects other clips, missing items and locked tracks", () => {
    const imageVideo = { ...clip("image-video", "video_clip", [6, 2], [0, 2], 1, false), source: { type: "media" as const, mediaId: "still" } };
    const generatedSound = { ...clip("generated-sound", "audio_clip", [0, 2], [0, 2], 1, false), source: { type: "generated" as const, artifactId: "artifact-1" } };
    const project = projectWith([reversedClip(), clip("still-1", "image_clip", [4, 2], [0, 2], 1, false), imageVideo], [generatedSound]);
    expect(() => applyProjectActionLocally(project, reverseAction("generated-sound", true))).toThrow(
      "Timeline item generated-sound cannot be reversed.",
    );
    expect(() => applyProjectActionLocally(project, reverseAction("still-1", true))).toThrow(
      "Timeline item still-1 cannot be reversed.",
    );
    expect(() => applyProjectActionLocally(project, reverseAction("image-video", true))).toThrow(
      "Timeline item image-video cannot be reversed.",
    );
    expect(() => applyProjectActionLocally(project, reverseAction("missing", true))).toThrow(
      "Timeline item missing was not found.",
    );
    const locked = withTracks(project, (tracks) => tracks.map((track) => ({ ...track, locked: track.id === VIDEO_TRACK })));
    expect(() => applyProjectActionLocally(locked, reverseAction("rev", false))).toThrow(`Track ${VIDEO_TRACK} is locked.`);
  });
});

describe("reversed source mapping", () => {
  it("reads source backwards from sourceOut", () => {
    expect(sourceSecondsAt(window(false), 1)).toBe(4);
    expect(sourceSecondsAt(window(true), 0)).toBe(10);
    expect(sourceSecondsAt(window(true), 1)).toBe(8);
    expect(sourceSecondsAt(window(true), 4)).toBe(2);
  });

  it("maps source seconds back to the timeline, mirrored for reversed clips", () => {
    const tenSeconds = (speed: number, reverse: boolean): SourceWindow => ({ sourceIn: 0, sourceOut: 10, speed, reverse });
    expect(localSecondsForSource(tenSeconds(1, false), 1)).toBe(1);
    expect(localSecondsForSource(tenSeconds(1, true), 1)).toBe(9);
    expect(localSecondsForSource(tenSeconds(2, true), 1)).toBe(4.5);
    expect(localSecondsForSource(window(true), 8)).toBe(1);
    // Source 1-2 s of a 0-10 s window plays at 1-2 s forward and 8-9 s reversed.
    expect(timelineRangeForSource(tenSeconds(1, false), 0, 1, 2)).toEqual([1, 2]);
    expect(timelineRangeForSource(tenSeconds(1, true), 0, 1, 2)).toEqual([8, 9]);
    expect(timelineRangeForSource(tenSeconds(1, true), 10, 1, 2)).toEqual([18, 19]);
    expect(timelineRangeForSource(tenSeconds(2, false), 0, 1, 2)).toEqual([0.5, 1]);
    expect(timelineRangeForSource(tenSeconds(2, true), 0, 1, 2)).toEqual([4, 4.5]);
  });

  it("swaps head and tail handles", () => {
    expect([headHandleSeconds(window(false), 20), tailHandleSeconds(window(false), 20)]).toEqual([1, 5]);
    expect([headHandleSeconds(window(true), 20), tailHandleSeconds(window(true), 20)]).toEqual([5, 1]);
    expect(handleSourceSeconds(window(true), "head", 0.5)).toBe(11);
    expect(handleSourceSeconds(window(true), "tail", 0.5)).toBe(1);
  });

  it("maps a range render overlap to the mirrored source window", () => {
    // Timeline 11-12.5 s is clip-local 1-2.5 s.
    expect([sourceSecondsAt(window(false), 1), sourceSecondsAt(window(false), 2.5)]).toEqual([4, 7]);
    expect([sourceSecondsAt(window(true), 2.5), sourceSecondsAt(window(true), 1)]).toEqual([5, 8]);
  });

  it("keeps sourceOut on the left part of a split", () => {
    expect(sourceSubrange(window(true), 0, 1.5, 4)).toEqual([7, 10]);
    expect(sourceSubrange(window(true), 1.5, 4, 4)).toEqual([2, 7]);
    const split = applyProjectActionLocally(projectWith([reversedClip()]), {
      type: "splitItems",
      splits: [{ itemId: "rev", newItemId: "rev-2", splitSeconds: 11.5 }],
    });
    expect(sourceRange(itemOf(split, "rev"))).toEqual([7, 10]);
    expect(sourceRange(itemOf(split, "rev-2"))).toEqual([2, 7]);
    expect(itemOf(split, "rev-2").properties.reverse).toBe(true);
  });

  it("mirrors both parts of a ripple delete", () => {
    const next = applyProjectActionLocally(projectWith([reversedClip()]), {
      type: "rippleDeleteRanges",
      ranges: [{ startSeconds: 11, endSeconds: 12, trackIds: [VIDEO_TRACK] }],
    });
    const items = next.timeline.tracks[0]?.items ?? [];
    expect(items.map(sourceRange)).toEqual([[8, 10], [2, 6]]);
  });

  const rippleTrim = (edge: "left" | "right", deltaSeconds: number) =>
    itemOf(
      applyProjectActionLocally(projectWith([reversedClip()]), {
        type: "rippleTrimItem",
        itemId: "rev",
        edge,
        deltaSeconds,
        propagateLinked: false,
        syncLockedTrackIds: [],
      }),
      "rev",
    );

  it("moves sourceOut when trimming the left edge", () => {
    const trimmed = rippleTrim("left", 0.5);
    expect(trimmed.durationSeconds).toBe(3.5);
    expect(sourceRange(trimmed)).toEqual([2, 9]);
    expect(trimmedSourceRange(window(true), "left", 0.5)).toEqual([2, 11]);
    expect(createLeftTrimPatchFromDrag(reversedClip(), 10.5)).toMatchObject({ sourceIn: 2, sourceOut: 9 });
    expect(createTrimPatchFromSourceMark(reversedClip(), 11, "in")).toMatchObject({ sourceIn: 2, sourceOut: 8 });
  });

  it("moves sourceIn when trimming the right edge", () => {
    const trimmed = rippleTrim("right", -0.5);
    expect(trimmed.durationSeconds).toBe(3.5);
    expect(sourceRange(trimmed)).toEqual([3, 10]);
    expect(trimmedSourceRange(window(true), "right", 0.5)).toEqual([1, 10]);
    expect(createRightTrimPatchFromDrag(reversedClip(), 13.5)).toMatchObject({ sourceIn: 3, sourceOut: 10 });
    expect(createTrimPatchFromSourceMark(reversedClip(), 11, "out")).toMatchObject({ sourceIn: 8, sourceOut: 10 });
  });

  it("resizes reversed clips within their mirrored handles", () => {
    const project = projectWith([reversedClip()]);
    const resize = (edge: "left" | "right", proposedStartSeconds: number, proposedDurationSeconds: number) =>
      evaluateTimelineResize({
        timeline: project.timeline,
        itemId: "rev",
        edge,
        proposedStartSeconds,
        proposedDurationSeconds,
        guideSeconds: null,
        sourceDurationSeconds: 20,
      });
    // The head handle is 5 s and the tail handle 1 s.
    expect(resize("left", 4, 10)).toMatchObject({ placement: { startSeconds: 5, durationSeconds: 9 }, patch: { sourceIn: 2, sourceOut: 20 } });
    expect(resize("right", 10, 8)).toMatchObject({ placement: { durationSeconds: 5 }, patch: { sourceIn: 0, sourceOut: 10 } });
  });

  it("uses reversed handles for transition maximums", () => {
    const leftReversed = projectWith([
      clip("a", "video_clip", [0, 4], [2, 10], 2, true),
      clip("b", "video_clip", [4, 4], [8, 12], 1, false),
    ]);
    const [a, b] = leftReversed.timeline.tracks[0]?.items ?? [];
    expect(transitionBounds(leftReversed, a!, b!)).toEqual({ maxSeconds: 2, limit: "leftHandle" });
    const rightReversed = projectWith([
      clip("c", "video_clip", [0, 4], [0, 4], 1, false),
      clip("d", "video_clip", [4, 4], [15, 19], 1, true),
    ]);
    const [c, d] = rightReversed.timeline.tracks[0]?.items ?? [];
    expect(transitionBounds(rightReversed, c!, d!)).toEqual({ maxSeconds: 2, limit: "rightHandle" });
  });

  it("keeps a transition on the right part of a split reversed left clip", () => {
    const project = withTracks(
      projectWith([
        clip("a", "video_clip", [0, 4], [2, 10], 2, true),
        clip("b", "video_clip", [4, 4], [8, 12], 1, false),
      ]),
      (tracks) =>
        tracks.map((track) =>
          track.id === VIDEO_TRACK
            ? { ...track, transitions: [{ id: "fade", leftItemId: "a", rightItemId: "b", kind: "crossfade" as const, durationSeconds: 1 }] }
            : track,
        ),
    );
    const split = applyProjectActionLocally(project, {
      type: "splitItems",
      splits: [{ itemId: "a", newItemId: "a-2", splitSeconds: 2 }],
    });
    expect(sourceRange(itemOf(split, "a-2"))).toEqual([2, 6]);
    expect(split.timeline.tracks[0]?.transitions).toEqual([
      { id: "fade", leftItemId: "a-2", rightItemId: "b", kind: "crossfade", durationSeconds: 1 },
    ]);
  });

  it("keeps detached sound reversed", () => {
    const detached = applyProjectActionLocally(projectWith([reversedClip()]), {
      type: "detachAudio",
      itemId: "rev",
      audioItemId: "rev-audio",
      targetTrackId: AUDIO_TRACK,
      linkGroupId: "link-rev",
    });
    expect(sourceRange(itemOf(detached, "rev-audio"))).toEqual([2, 10]);
    expect(itemOf(detached, "rev-audio").properties.reverse).toBe(true);
  });

  it("previews transition handles from the mirrored source edges", () => {
    expect(transitionHandleSourceSeconds(reversedClip(), 9.5, 2, 20)).toBe(11);
    expect(transitionHandleSourceSeconds(reversedClip(), 14.5, 2, 20)).toBe(1);
  });
});
