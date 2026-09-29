import { describe, expect, it } from "vitest";
import {
  buildTimelineRows,
  canonicalTimelineItemKindForMediaKind,
  createCaptionLeftResizePatches,
  createEditCaptionTextPatch,
  createLeftTrimPatchFromDrag,
  createMovePatchFromDrag,
  createResizePatchFromDrag,
  createRightTrimPatchFromDrag,
  createTrimPatchFromSourceMark,
  getCaptionReadingWarning,
  getTimelineItemText,
  isTemplateTimelineItem,
  itemAllowedOnTrack,
  sampleTimeline,
} from "./timeline";
import { requiredAt } from "../test-utils/required";

describe("timeline adapter", () => {
  it("maps project tracks to timeline rows", () => {
    const rows = buildTimelineRows(sampleTimeline);

    expect(rows).toEqual([
      { id: "track-video", title: "Video" },
      { id: "track-scenes", title: "HyperFrames" },
      { id: "track-overlays", title: "Overlays" },
      { id: "track-captions", title: "Captions" },
      { id: "track-audio", title: "Audio" },
    ]);
  });

  it("creates a typed move patch from a drag result", () => {
    const patch = createMovePatchFromDrag({
      itemId: "item-1",
      targetTrackId: "track-video",
      startSeconds: 2.25,
    });

    expect(patch).toEqual({
      type: "moveItem",
      itemId: "item-1",
      targetTrackId: "track-video",
      startSeconds: 2.25,
    });
  });

  it("models image, lottie, and generated visual sources as first-class video-track clip kinds", () => {
    expect(canonicalTimelineItemKindForMediaKind("video")).toBe("video_clip");
    expect(canonicalTimelineItemKindForMediaKind("image")).toBe("image_clip");
    expect(canonicalTimelineItemKindForMediaKind("lottie")).toBe("lottie_clip");
    expect(canonicalTimelineItemKindForMediaKind("generated")).toBe("generated_clip");
    expect(canonicalTimelineItemKindForMediaKind("audio")).toBe("audio_clip");

    expect(itemAllowedOnTrack("image_clip", "video")).toBe(true);
    expect(itemAllowedOnTrack("lottie_clip", "video")).toBe(true);
    expect(itemAllowedOnTrack("generated_clip", "video")).toBe(true);
    expect(itemAllowedOnTrack("image_clip", "audio")).toBe(false);
  });

  it("creates a typed caption text patch", () => {
    const patch = createEditCaptionTextPatch({
      itemId: "caption-1",
      text: "Corrected caption",
    });

    expect(patch).toEqual({
      type: "editCaptionText",
      itemId: "caption-1",
      text: "Corrected caption",
    });
  });

  it("reads text sources from timeline items", () => {
    const captionTrack = sampleTimeline.tracks.find((track) => track.kind === "caption");
    const caption = captionTrack?.items[0];

    expect(caption).toBeDefined();
    expect(caption ? getTimelineItemText(caption) : "").toBe("Original caption text");
  });

  it("detects timeline items backed by motion templates", () => {
    const templateItem = {
      id: "template-item-1",
      kind: "overlay" as const,
      startSeconds: 0,
      durationSeconds: 2.4,
      source: { type: "generated" as const, artifactId: "template:kinetic-lower-third-v1" },
      label: "Kinetic Lower Third",
      properties: { templateId: "kinetic-lower-third-v1" },
    };
    const plainItem = requiredAt(
      requiredAt(sampleTimeline.tracks, 0, "sample video track").items,
      0,
      "sample video item",
    );

    expect(isTemplateTimelineItem(templateItem)).toBe(true);
    expect(isTemplateTimelineItem(plainItem)).toBe(false);
  });

  it("warns when edited caption text is too dense for the cue", () => {
    const warning = getCaptionReadingWarning({
      text: "A very long correction that exceeds the recommended cue density",
      durationSeconds: 0.8,
    });

    expect(warning).toContain("too long");
  });

  it("creates a typed resize patch from a resize result", () => {
    const patch = createResizePatchFromDrag({
      itemId: "caption-1",
      durationSeconds: 1.8,
    });

    expect(patch).toEqual({
      type: "resizeItem",
      itemId: "caption-1",
      durationSeconds: 1.8,
    });
  });

  it("creates source-in and source-out trim patches from playhead marks", () => {
    const item = {
      id: "clip-1",
      kind: "video_clip" as const,
      startSeconds: 1,
      durationSeconds: 4,
      source: { type: "media" as const, mediaId: "media-1" },
      label: "Hook clip",
      properties: { sourceIn: 0, sourceOut: 4 },
    };

    expect(createTrimPatchFromSourceMark(item, 4, "in")).toEqual({
      type: "trimItem",
      itemId: "clip-1",
      startSeconds: 4,
      durationSeconds: 1,
      sourceIn: 3,
      sourceOut: 4,
    });
    expect(createTrimPatchFromSourceMark(item, 4, "out")).toEqual({
      type: "trimItem",
      itemId: "clip-1",
      startSeconds: 1,
      durationSeconds: 3,
      sourceIn: 0,
      sourceOut: 3,
    });
  });

  it("creates a source-preserving trim patch from a left-edge drag", () => {
    const patch = createLeftTrimPatchFromDrag(
      {
        id: "clip-1",
        kind: "video_clip" as const,
        startSeconds: 1,
        durationSeconds: 4,
        source: { type: "media" as const, mediaId: "media-1" },
        label: "Hook clip",
        properties: { sourceIn: 0, sourceOut: 4 },
      },
      2.25,
    );

    expect(patch).toEqual({
      type: "trimItem",
      itemId: "clip-1",
      startSeconds: 2.25,
      durationSeconds: 2.75,
      sourceIn: 1.25,
      sourceOut: 4,
    });
  });

  it("clamps left-edge trim to available source media", () => {
    const patch = createLeftTrimPatchFromDrag(
      {
        id: "clip-1",
        kind: "video_clip" as const,
        startSeconds: 1,
        durationSeconds: 4,
        source: { type: "media" as const, mediaId: "media-1" },
        label: "Hook clip",
        properties: { sourceIn: 0.5, sourceOut: 4.5 },
      },
      0,
    );

    expect(patch).toMatchObject({
      startSeconds: 0.5,
      durationSeconds: 4.5,
      sourceIn: 0,
      sourceOut: 4.5,
    });
  });

  it("creates a source-preserving trim patch from a right-edge drag", () => {
    const patch = createRightTrimPatchFromDrag(
      {
        id: "clip-1",
        kind: "video_clip" as const,
        startSeconds: 1,
        durationSeconds: 4,
        source: { type: "media" as const, mediaId: "media-1" },
        label: "Hook clip",
        properties: { sourceIn: 0.5, sourceOut: 4.5 },
      },
      6,
    );

    expect(patch).toEqual({
      type: "trimItem",
      itemId: "clip-1",
      startSeconds: 1,
      durationSeconds: 5,
      sourceIn: 0.5,
      sourceOut: 5.5,
    });
  });

  describe("speed-adjusted clips", () => {
    const fastClip = {
      id: "clip-fast",
      kind: "video_clip" as const,
      startSeconds: 1,
      durationSeconds: 2,
      source: { type: "media" as const, mediaId: "media-1" },
      label: "Fast clip",
      properties: { sourceIn: 1, sourceOut: 5, speed: 2 },
    };

    it("scales source marks by clip speed", () => {
      expect(createTrimPatchFromSourceMark(fastClip, 2, "in")).toEqual({
        type: "trimItem",
        itemId: "clip-fast",
        startSeconds: 2,
        durationSeconds: 1,
        sourceIn: 3,
        sourceOut: 5,
      });
      expect(createTrimPatchFromSourceMark(fastClip, 2, "out")).toEqual({
        type: "trimItem",
        itemId: "clip-fast",
        startSeconds: 1,
        durationSeconds: 1,
        sourceIn: 1,
        sourceOut: 3,
      });
    });

    it("defaults a missing source-out mark to duration times speed", () => {
      const clip = { ...fastClip, properties: { speed: 2 } };

      expect(createTrimPatchFromSourceMark(clip, 2, "in")).toMatchObject({
        durationSeconds: 1,
        sourceIn: 2,
        sourceOut: 4,
      });
    });

    it("scales left-edge drag source deltas and clamps by clip speed", () => {
      expect(createLeftTrimPatchFromDrag(fastClip, 1.5)).toEqual({
        type: "trimItem",
        itemId: "clip-fast",
        startSeconds: 1.5,
        durationSeconds: 1.5,
        sourceIn: 2,
        sourceOut: 5,
      });
      expect(createLeftTrimPatchFromDrag(fastClip, 0)).toEqual({
        type: "trimItem",
        itemId: "clip-fast",
        startSeconds: 0.5,
        durationSeconds: 2.5,
        sourceIn: 0,
        sourceOut: 5,
      });
    });

    it("scales right-edge drag source deltas by clip speed", () => {
      expect(createRightTrimPatchFromDrag(fastClip, 4)).toEqual({
        type: "trimItem",
        itemId: "clip-fast",
        startSeconds: 1,
        durationSeconds: 3,
        sourceIn: 1,
        sourceOut: 7,
      });
    });
  });

  it("does not create a right trim patch for items without source ranges", () => {
    const patch = createRightTrimPatchFromDrag(
      {
        id: "clip-1",
        kind: "video_clip" as const,
        startSeconds: 1,
        durationSeconds: 4,
        source: { type: "media" as const, mediaId: "media-1" },
        label: "Hook clip",
        properties: {},
      },
      6,
    );

    expect(patch).toBeNull();
  });

  it("creates move and resize patches for left-edge caption resizing", () => {
    const patches = createCaptionLeftResizePatches({
      itemId: "caption-1",
      targetTrackId: "track-captions",
      originalStartSeconds: 1,
      originalDurationSeconds: 2,
      nextStartSeconds: 1.4,
      minimumDurationSeconds: 0.1,
    });

    expect(patches).toEqual([
      {
        type: "moveItem",
        itemId: "caption-1",
        targetTrackId: "track-captions",
        startSeconds: 1.4,
      },
      {
        type: "resizeItem",
        itemId: "caption-1",
        durationSeconds: 1.6,
      },
    ]);
  });

  it("clamps left-edge resize to keep caption duration above the minimum", () => {
    const patches = createCaptionLeftResizePatches({
      itemId: "caption-1",
      targetTrackId: "track-captions",
      originalStartSeconds: 1,
      originalDurationSeconds: 2,
      nextStartSeconds: 2.98,
      minimumDurationSeconds: 0.25,
    });

    expect(patches[0]).toMatchObject({ startSeconds: 2.75 });
    expect(patches[1]).toMatchObject({ durationSeconds: 0.25 });
  });
});
