import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import { commonPropertySupport, selectionKind } from "@/lib/preview/selection-kind";
import type { TimelineItem, TimelineItemKind, TimelineTrack, TrackKind } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function item(
  id: string,
  kind: TimelineItemKind,
  properties: Record<string, unknown> = {},
  source: TimelineItem["source"] = { type: "media", mediaId: "media-1" },
): TimelineItem {
  return { id, kind, startSeconds: 0, durationSeconds: 2, source, label: id, properties };
}

function track(id: string, kind: TrackKind, items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

function project(): VideoProject {
  const tracks = [
    track("video", "video", [
      item("video", "video_clip"),
      item("image", "image_clip"),
      item("lottie", "lottie_clip"),
      item("generated", "generated_clip", {}, { type: "generated", artifactId: "asset-1" }),
      item("nested", "video_clip", {}, { type: "timeline", timelineId: "timeline-2" }),
    ]),
    track("scenes", "hyperframe_scene", [item("scene", "hyperframe_scene")]),
    track("overlays", "overlay", [
      item("text", "overlay", {}, { type: "text", text: "Hello" }),
      item("template", "overlay", { templateId: "kinetic-lower-third-v1" }, { type: "text", text: "" }),
      item("media-overlay", "overlay"),
    ]),
    track("captions", "caption", [item("caption", "caption", {}, { type: "text", text: "Hi there" })]),
    track("audio", "audio", [item("audio", "audio_clip")]),
  ];
  return { ...fixtureProject(), timeline: { durationSeconds: 2, tracks } };
}

describe("selectionKind", () => {
  it.each([
    ["video", "visual"],
    ["image", "visual"],
    ["lottie", "visual"],
    ["generated", "visual"],
    ["nested", "visual"],
    ["scene", "visual"],
    ["media-overlay", "visual"],
    ["text", "text"],
    ["template", "template"],
    ["caption", "caption"],
    ["audio", "audio"],
  ])("maps %s to %s", (itemId, kind) => {
    expect(selectionKind(project(), [itemId])).toBe(kind);
  });

  it("maps an empty selection and unknown ids to none", () => {
    expect(selectionKind(project(), [])).toBe("none");
    expect(selectionKind(project(), ["missing"])).toBe("none");
  });

  it("maps two or more existing items to multiple, even of the same kind", () => {
    expect(selectionKind(project(), ["video", "audio"])).toBe("multiple");
    expect(selectionKind(project(), ["video", "image"])).toBe("multiple");
  });

  it("ignores duplicate and missing ids when counting", () => {
    expect(selectionKind(project(), ["video", "video", "missing"])).toBe("visual");
  });

  it("maps a selected transition on the active timeline to transition when no item is selected", () => {
    const withTransition = project();
    const [videoTrack] = withTransition.timeline.tracks;
    if (!videoTrack) throw new Error("missing video track");
    videoTrack.transitions = [{ id: "fade", leftItemId: "video", rightItemId: "image", kind: "crossfade", durationSeconds: 0.5 }];
    expect(selectionKind(withTransition, [], "fade")).toBe("transition");
    expect(selectionKind(withTransition, [], "missing")).toBe("none");
    expect(selectionKind(withTransition, ["audio"], "fade")).toBe("audio");
  });
});

describe("commonPropertySupport", () => {
  it("supports opacity only when every selected item is visual", () => {
    expect(commonPropertySupport(project(), ["video", "image", "text"]).opacity).toBe(true);
    expect(commonPropertySupport(project(), ["video", "audio"]).opacity).toBe(false);
    expect(commonPropertySupport(project(), ["video", "caption"]).opacity).toBe(false);
  });

  it("supports volume only when every selected item is an audio clip", () => {
    expect(commonPropertySupport(project(), ["audio"]).volume).toBe(true);
    expect(commonPropertySupport(project(), ["audio", "video"]).volume).toBe(false);
  });

  it("supports effects only for visual source clips that are not nested timelines", () => {
    expect(commonPropertySupport(project(), ["video", "image", "generated"]).effects).toBe(true);
    expect(commonPropertySupport(project(), ["video", "nested"]).effects).toBe(false);
    expect(commonPropertySupport(project(), ["video", "text"]).effects).toBe(false);
  });

  it("supports nothing for an empty selection", () => {
    expect(commonPropertySupport(project(), [])).toEqual({ opacity: false, volume: false, effects: false });
  });
});
