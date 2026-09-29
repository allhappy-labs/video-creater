import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { planAssetInsert, playheadAssetPlacement } from "@/lib/timeline-ops/asset-insert";
import { orderedTracksByBand } from "@/lib/timeline-ops/track-bands";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[] = [], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

/** Video 1 holds "a" 0–4 s; Audio 1 holds "m" 0–4 s. Media "media-1" (video) and "media-voiceover" (audio) last 4 s. */
function projectWith(tracks: TimelineTrack[] = [track("v1", "video", [clip("a", 0, 4)]), track("a1", "audio", [clip("m", 0, 4, { kind: "audio_clip" })])]): VideoProject {
  return { ...fixtureProject(), timeline: { durationSeconds: 20, tracks } };
}

function apply(project: VideoProject, result: ReturnType<typeof planAssetInsert>): VideoProject {
  if ("blocked" in result) throw new Error(result.blocked);
  return result.actions.reduce(applyProjectActionLocally, project);
}

describe("planAssetInsert", () => {
  it("adds media to a free hovered track at the drop time", () => {
    const project = projectWith();
    const result = planAssetInsert(project, { kind: "media", id: "media-1" }, { hoveredTrackId: "v1", insertIndex: null, startSeconds: 6.1234 }, "x");
    expect(result).toEqual({
      actions: [
        {
          type: "addItems",
          targetTrackId: "v1",
          items: [
            {
              id: "timeline-media-1",
              kind: "video_clip",
              startSeconds: 6.123,
              durationSeconds: 4,
              source: { type: "media", mediaId: "media-1" },
              label: expect.any(String),
              properties: { sourceIn: 0, sourceOut: 4 },
            },
          ],
        },
      ],
    });
  });

  it("creates a track above a colliding track in the same batch", () => {
    const project = projectWith();
    const result = planAssetInsert(project, { kind: "media", id: "media-1" }, { hoveredTrackId: "v1", insertIndex: null, startSeconds: 1 }, "x");
    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.actions.map((action) => action.type)).toEqual(["createTrack", "reorderTrack", "addItems"]);
    const after = apply(project, result);
    expect(orderedTracksByBand(after.timeline).map((entry) => [entry.id, entry.items.map((item) => item.id)])).toEqual([
      ["track-video-3", ["timeline-media-1"]],
      ["v1", ["a"]],
      ["a1", ["m"]],
    ]);
  });

  it("places templates and backgrounds between tracks in their band", () => {
    const project = projectWith();
    const template = planAssetInsert(project, { kind: "template", id: "kinetic-lower-third-v1" }, { hoveredTrackId: null, insertIndex: 1, startSeconds: 2 }, "k1");
    const withTemplate = apply(project, template);
    const text = orderedTracksByBand(withTemplate.timeline)[0];
    expect(text).toMatchObject({ kind: "overlay", items: [{ id: "template-kinetic-lower-third-v1-k1", kind: "overlay", startSeconds: 2 }] });

    const background = apply(project, planAssetInsert(project, { kind: "background", id: "shadertoy-octagrams-v1" }, { hoveredTrackId: null, insertIndex: null, startSeconds: 0 }, "b1"));
    expect(orderedTracksByBand(background.timeline)[0]).toMatchObject({
      kind: "hyperframe_scene",
      items: [{ id: "shader-background-shadertoy-octagrams-v1-b1", kind: "hyperframe_scene" }],
    });
  });

  it("places the default text overlay on a new text track in one batch", () => {
    const project = projectWith();
    const result = planAssetInsert(project, { kind: "text", id: "text" }, { hoveredTrackId: null, insertIndex: null, startSeconds: 2.5 }, "t1");
    if ("blocked" in result) throw new Error(result.blocked);
    expect(result.actions.map((action) => action.type)).toEqual(["createTrack", "addItems"]);
    expect(result.actions[1]).toEqual({
      type: "addItems",
      targetTrackId: "track-text-3",
      items: [
        {
          id: "text-overlay-t1",
          kind: "overlay",
          startSeconds: 2.5,
          durationSeconds: 3,
          source: { type: "text", text: "Your text" },
          label: "Text",
          properties: {
            text: "Your text",
            visualTreatment: "clean editable text overlay with high-contrast type and transparent backing",
            motion: "quick fade in, hold, and soft fade out",
            safeZone: "keep text inside 10% title-safe margins",
            avoid: "opaque slabs, default-font template look, and covering faces or key action",
          },
        },
      ],
    });
    expect(orderedTracksByBand(apply(project, result).timeline)[0]).toMatchObject({ kind: "overlay", items: [{ id: "text-overlay-t1" }] });
  });

  it("blocks drops onto locked tracks and missing or unplaceable assets", () => {
    const project = projectWith([track("v1", "video", [], true)]);
    expect(planAssetInsert(project, { kind: "media", id: "media-1" }, { hoveredTrackId: "v1", insertIndex: null, startSeconds: 0 }, "x")).toEqual({ blocked: "Track is locked" });
    expect(planAssetInsert(project, { kind: "media", id: "missing" }, { hoveredTrackId: null, insertIndex: null, startSeconds: 0 }, "x")).toEqual({
      blocked: "That media is no longer in the project.",
    });
    expect(planAssetInsert(project, { kind: "template", id: "missing-template" }, { hoveredTrackId: null, insertIndex: null, startSeconds: 0 }, "x")).toEqual({
      blocked: "This template can't be placed on the timeline.",
    });
    expect(planAssetInsert(project, { kind: "background", id: "missing" }, { hoveredTrackId: null, insertIndex: null, startSeconds: 0 }, "x")).toEqual({
      blocked: "This background can't be placed on the timeline.",
    });
  });
});

describe("playheadAssetPlacement", () => {
  it("targets the first unlocked compatible track, or a new track in the band", () => {
    const project = projectWith([track("v1", "video", [], true), track("v2", "video"), track("a1", "audio")]);
    expect(playheadAssetPlacement(project, { kind: "media", id: "media-1" }, 3)).toEqual({ hoveredTrackId: "v2", insertIndex: null, startSeconds: 3 });
    expect(playheadAssetPlacement(project, { kind: "media", id: "media-voiceover" }, 3)).toMatchObject({ hoveredTrackId: "a1" });
    expect(playheadAssetPlacement(project, { kind: "template", id: "kinetic-lower-third-v1" }, 3)).toMatchObject({ hoveredTrackId: null, insertIndex: null });
  });

  it("targets the first unlocked text track for the default text overlay", () => {
    const project = projectWith([track("v1", "video"), track("o1", "overlay", [], true), track("o2", "overlay")]);
    expect(playheadAssetPlacement(project, { kind: "text", id: "text" }, 1.5)).toEqual({ hoveredTrackId: "o2", insertIndex: null, startSeconds: 1.5 });
  });
});
