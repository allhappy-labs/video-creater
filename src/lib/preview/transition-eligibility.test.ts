import { describe, expect, it } from "vitest";
import type { MediaAsset } from "@/lib/project";
import type { Timeline, TimelineItem, TimelineTrack, TimelineTransition, TransitionKind } from "@/lib/timeline";
import { buildTimelinePreviewFrame } from "@/lib/timeline-preview";
import { flattenGroupsAt, transitionEligibility } from "./transition-eligibility";

const media: MediaAsset[] = [
  { id: "video", relativePath: "media/clip.mp4", kind: "video", durationSeconds: 10, width: 1920, height: 1080, fps: 30, folderId: null },
  { id: "still", relativePath: "media/still.png", kind: "image", durationSeconds: 0, width: 1920, height: 1080, fps: null, folderId: null },
  { id: "music", relativePath: "media/music.m4a", kind: "audio", durationSeconds: 10, width: null, height: null, fps: null, folderId: null },
];

const frameSeconds = 1 / 24;

// 4 s clips meeting at 4 s with 2 s of media on both sides of the cut; a 1 s transition spans [3.5, 4.5).
function clip(id: string, startSeconds: number, properties: TimelineItem["properties"] = {}, mediaId = "video"): TimelineItem {
  return {
    id,
    kind: "video_clip",
    startSeconds,
    durationSeconds: 4,
    label: id,
    source: { type: "media", mediaId },
    properties: { sourceIn: 2, sourceOut: 6, ...properties },
  };
}

function still(id: string, startSeconds: number): TimelineItem {
  return { ...clip(id, startSeconds, {}, "still"), kind: "image_clip", properties: {} };
}

function generated(id: string, startSeconds: number): TimelineItem {
  return { ...clip(id, startSeconds), kind: "generated_clip", source: { type: "generated", artifactId: "artifact" }, properties: {} };
}

function transition(kind: TransitionKind = "crossfade", leftItemId = "left", rightItemId = "right"): TimelineTransition {
  return { id: `${leftItemId}-${rightItemId}`, leftItemId, rightItemId, kind, durationSeconds: 1 };
}

function track(id: string, items: TimelineItem[], transitions: TimelineTransition[] = [], kind: TimelineTrack["kind"] = "video"): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items, ...(transitions.length > 0 ? { transitions } : {}) };
}

function timelineOf(...tracks: TimelineTrack[]): Timeline {
  return { durationSeconds: 8, tracks };
}

function eligibility(timeline: Timeline) {
  const result = transitionEligibility(timeline, { media, generatedAssets: [] }, frameSeconds);
  return { dropped: [...result.droppedTransitionIds], flattened: [...result.flattenedTransitionIds] };
}

const blended = { blendMode: "screen" };
const blurred = { effects: [{ effectType: "blur.gaussian", enabled: true }] };
const lut = { colorGrade: { lut: { path: "luts/look.cube" } } };
const denoised = { effects: [{ effectType: "audio.denoise", enabled: true, params: { amount: 0.5 } }] };

function audioClip(id: string, startSeconds: number, properties: TimelineItem["properties"] = {}): TimelineItem {
  return { ...clip(id, startSeconds, properties, "music"), kind: "audio_clip" };
}

describe("transition eligibility", () => {
  it("keeps every transition of projects without rich clips or denoise", () => {
    expect(eligibility(timelineOf(track("v1", [clip("left", 0), clip("right", 4)])))).toEqual({ dropped: [], flattened: [] });
    expect(eligibility(timelineOf(track("v1", [clip("left", 0), clip("right", 4)], [transition()])))).toEqual({ dropped: [], flattened: [] });
    expect(eligibility(timelineOf(track("v1", [clip("left", 0), still("right", 4)], [transition()])))).toEqual({ dropped: [], flattened: [] });
  });

  it("bakes a pair of video clips into the flattened composite of a rich clip", () => {
    expect(eligibility(timelineOf(track("v1", [clip("left", 0, blended), clip("right", 4)], [transition()])))).toEqual({
      dropped: [],
      flattened: ["left-right"],
    });
    // A rich clip on a higher track widens its group over a video pair beneath it.
    const beneath = timelineOf(
      track("v1", [clip("left", 0), clip("right", 4)], [transition("dipToBlack")]),
      track("v2", [{ ...clip("top", 4.2, blurred), durationSeconds: 1 }]),
    );
    expect(eligibility(beneath)).toEqual({ dropped: [], flattened: ["left-right"] });
  });

  it("drops a rich clip's transition to an image or generated clip, like the render's hard cut", () => {
    expect(eligibility(timelineOf(track("v1", [clip("left", 0, blurred), still("right", 4)], [transition()])))).toEqual({
      dropped: ["left-right"],
      flattened: [],
    });
    expect(eligibility(timelineOf(track("v1", [generated("left", 0), clip("right", 4, blended)], [transition("wipe")])))).toEqual({
      dropped: ["left-right"],
      flattened: [],
    });
  });

  it("drops an unflattenable pair beneath a rich clip only when the group reaches its cut", () => {
    const stills = track("v1", [still("left", 0), still("right", 4)], [transition()]);
    const overCut = timelineOf(stills, track("v2", [{ ...clip("top", 3, blended), durationSeconds: 2 }]));
    expect(eligibility(overCut)).toEqual({ dropped: ["left-right"], flattened: [] });
    const awayFromCut = timelineOf(stills, track("v2", [{ ...clip("top", 6, blended), durationSeconds: 1 }]));
    expect(eligibility(awayFromCut)).toEqual({ dropped: [], flattened: [] });
    // A disabled rich track flattens nothing.
    const disabled = timelineOf(stills, { ...track("v2", [{ ...clip("top", 3, blended), durationSeconds: 2 }]), enabled: false });
    expect(eligibility(disabled)).toEqual({ dropped: [], flattened: [] });
  });

  it("keeps LUT and Lottie prepared sources, which carry their handles, unless effects after the LUT remain", () => {
    expect(eligibility(timelineOf(track("v1", [clip("left", 0, lut), still("right", 4)], [transition()])))).toEqual({ dropped: [], flattened: [] });
    const baked = { effects: [{ effectType: "color.exposure", enabled: true }, { effectType: "color.lut", enabled: true, params: { path: "luts/look.cube" } }] };
    expect(eligibility(timelineOf(track("v1", [clip("left", 0, baked), still("right", 4)], [transition()])))).toEqual({ dropped: [], flattened: [] });
    const afterLut = { ...lut, effects: [{ effectType: "stylize.vignette", enabled: true }] };
    expect(eligibility(timelineOf(track("v1", [clip("left", 0, afterLut), still("right", 4)], [transition()])))).toEqual({
      dropped: ["left-right"],
      flattened: [],
    });
    // A colour grade without a LUT is a prepared effect.
    const grade = { colorGrade: { exposure: 0.5 } };
    expect(eligibility(timelineOf(track("v1", [clip("left", 0, grade), still("right", 4)], [transition()])))).toEqual({
      dropped: ["left-right"],
      flattened: [],
    });
  });

  it("drops audio transitions of clips rendered from denoised intermediates", () => {
    const audio = (left: TimelineItem["properties"], right: TimelineItem["properties"]) =>
      eligibility(timelineOf(track("a1", [audioClip("left", 0, left), audioClip("right", 4, right)], [transition()], "audio")));
    expect(audio(denoised, {})).toEqual({ dropped: ["left-right"], flattened: [] });
    expect(audio({}, { effects: [{ type: "audio.denoise" }] })).toEqual({ dropped: ["left-right"], flattened: [] });
    expect(audio({ effects: [{ effectType: "audio.denoise", enabled: false }] }, {})).toEqual({ dropped: [], flattened: [] });
  });
});

describe("flatten groups at the playhead", () => {
  const sources = { media, generatedAssets: [] };

  it("finds a rich clip's group over a plain lower-track clip without any transitions", () => {
    const timeline = timelineOf(track("v1", [clip("base", 0)]), track("v2", [{ ...clip("top", 1, blended), durationSeconds: 2 }]));
    expect(flattenGroupsAt(timeline, sources, frameSeconds, 2)).toEqual([{ start: 1, end: 3, topTrackIndex: 1 }]);
    expect(flattenGroupsAt(timeline, sources, frameSeconds, 3)).toEqual([]);
    expect(flattenGroupsAt(timeline, sources, frameSeconds, 0.5)).toEqual([]);
    expect(flattenGroupsAt(timelineOf(track("v1", [clip("base", 0)])), sources, frameSeconds, 2)).toEqual([]);
  });

  it("widens groups over flattenable transitions like transition eligibility", () => {
    const beneath = timelineOf(
      track("v1", [clip("left", 0), clip("right", 4)], [transition("dipToBlack")]),
      track("v2", [{ ...clip("top", 4.2, blurred), durationSeconds: 1 }]),
    );
    // The dip window [3.5, 4.5) widens the [4.2, 5.2) group back to 3.5.
    expect(flattenGroupsAt(beneath, sources, frameSeconds, 3.6)).toEqual([{ start: 3.5, end: 5.2, topTrackIndex: 1 }]);
    expect(eligibility(beneath)).toEqual({ dropped: [], flattened: ["left-right"] });
  });
});

describe("preview frames with render transition eligibility", () => {
  function frameAt(timeline: Timeline, playheadSeconds: number) {
    return buildTimelinePreviewFrame({ timeline, media, playheadSeconds, fps: 24 });
  }

  it("hard-cuts a rich clip into an image", () => {
    const timeline = timelineOf(track("v1", [clip("left", 0, blurred), still("right", 4)], [transition("dipToBlack")]));
    for (const [seconds, itemId] of [[3.75, "left"], [4, "right"], [4.25, "right"]] as const) {
      const frame = frameAt(timeline, seconds);
      expect(frame.layers.map((layer) => ({ itemId: layer.itemId, opacity: layer.opacity, transition: layer.transition }))).toEqual([
        { itemId, opacity: 1, transition: undefined },
      ]);
      expect(frame).not.toHaveProperty("transitions");
    }
  });

  it("marks transitions baked into a flattened composite and leaves plain pairs unmarked", () => {
    const baked = frameAt(timelineOf(track("v1", [clip("left", 0, blended), clip("right", 4)], [transition("dipToWhite")])), 4);
    expect(baked.layers.map((layer) => layer.itemId)).toEqual(["left", "right"]);
    expect(baked.transitions).toMatchObject([{ transitionId: "left-right", solidColor: "white", flattened: true }]);
    const plain = frameAt(timelineOf(track("v1", [clip("left", 0), clip("right", 4)], [transition()])), 4);
    expect(plain.transitions).toEqual([
      { transitionId: "left-right", kind: "crossfade", leftItemId: "left", rightItemId: "right", startSeconds: 3.5, durationSeconds: 1, progress: 0.5, solidColor: null },
    ]);
  });

  it("lists lower-track clips inside an active flattened group as covered", () => {
    const timeline = timelineOf(
      track("v1", [clip("base", 0)]),
      track("v2", [{ ...clip("top", 1, blended), durationSeconds: 2 }]),
      track("v3", [clip("above", 0)]),
    );
    expect(frameAt(timeline, 2).flattenedCoverItemIds).toEqual(["base", "top"]);
    expect(frameAt(timeline, 3.5)).not.toHaveProperty("flattenedCoverItemIds");
    expect(frameAt(timelineOf(track("v1", [clip("base", 0)])), 2)).not.toHaveProperty("flattenedCoverItemIds");
  });

  it("does not crossfade denoised audio", () => {
    const timeline = timelineOf(track("a1", [audioClip("left", 0, denoised), audioClip("right", 4)], [transition()], "audio"));
    expect(frameAt(timeline, 3.75).audioLayers.map(({ itemId, gain, transition: fade }) => ({ itemId, gain, fade }))).toEqual([
      { itemId: "left", gain: 1, fade: undefined },
    ]);
    expect(frameAt(timeline, 4).audioLayers.map(({ itemId, gain }) => ({ itemId, gain }))).toEqual([{ itemId: "right", gain: 1 }]);
  });
});
