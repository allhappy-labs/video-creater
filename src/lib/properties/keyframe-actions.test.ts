import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";
import {
  clipLocalPlayhead,
  keyframedEditAction,
  keyframeStateAtPlayhead,
  toggleKeyframeAction,
} from "@/lib/properties/keyframe-actions";
import type { TimelineItem } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function clip(properties: Record<string, unknown> = {}): TimelineItem {
  return {
    id: "clip",
    kind: "video_clip",
    startSeconds: 2,
    durationSeconds: 4,
    source: { type: "media", mediaId: "media-1" },
    label: "Clip",
    properties,
  };
}

function withClip(item: TimelineItem): VideoProject {
  const project = fixtureProject();
  return {
    ...project,
    timeline: {
      durationSeconds: 6,
      tracks: [{ id: "video", name: "Video", kind: "video", locked: false, enabled: true, items: [item] }],
    },
  };
}

function itemAfter(project: VideoProject, action: ProjectAction): TimelineItem {
  const next = applyProjectActionLocally(project, action);
  const found = next.timeline.tracks[0]?.items[0];
  if (!found) throw new Error("clip missing");
  return found;
}

describe("clipLocalPlayhead", () => {
  it("clamps the playhead into the clip and rounds to milliseconds", () => {
    expect(clipLocalPlayhead(clip(), 3.12345)).toBe(1.123);
    expect(clipLocalPlayhead(clip(), 0)).toBe(0);
    expect(clipLocalPlayhead(clip(), 99)).toBe(4);
    expect(clipLocalPlayhead(clip(), Number.NaN)).toBe(0);
  });
});

describe("keyframeStateAtPlayhead", () => {
  it("reports an unkeyframed property with its static value", () => {
    expect(keyframeStateAtPlayhead(clip({ opacity: 0.4 }), "opacity", 3)).toEqual({
      keyframed: false,
      localSeconds: 1,
      atPlayhead: null,
      value: 0.4,
    });
  });

  it("finds the keyframe at the playhead and interpolates between keyframes", () => {
    const item = clip({
      keyframes: { opacity: [{ atSeconds: 0, value: 0 }, { atSeconds: 2, value: 1, easing: "hold" }] },
    });
    expect(keyframeStateAtPlayhead(item, "opacity", 4)).toMatchObject({
      keyframed: true,
      localSeconds: 2,
      atPlayhead: { atSeconds: 2, value: 1, easing: "hold" },
      value: 1,
    });
    expect(keyframeStateAtPlayhead(item, "opacity", 3)).toMatchObject({ atPlayhead: null, value: 0.5 });
  });
});

describe("toggleKeyframeAction", () => {
  it("adds a keyframe at the item-relative playhead and removes it again", () => {
    const project = withClip(clip({ scale: 1.5 }));
    const add = toggleKeyframeAction(clip({ scale: 1.5 }), "scale", 3.5);
    expect(add).toEqual({
      type: "upsertItemKeyframe",
      itemId: "clip",
      property: "scale",
      keyframe: { atSeconds: 1.5, value: 1.5, easing: "linear" },
    });

    const keyed = itemAfter(project, add);
    const remove = toggleKeyframeAction(keyed, "scale", 3.5);
    expect(remove).toEqual({ type: "deleteItemKeyframe", itemId: "clip", property: "scale", atSeconds: 1.5 });
    expect(itemAfter(withClip(keyed), remove).properties.keyframes).toBeUndefined();
  });

  it("uses an explicit value clamped to the property bounds", () => {
    expect(toggleKeyframeAction(clip(), "opacity", 2, 1.7)).toMatchObject({
      keyframe: { atSeconds: 0, value: 1 },
    });
  });
});

describe("keyframedEditAction", () => {
  it("returns null when the property has no keyframes", () => {
    expect(keyframedEditAction(clip(), "rotationDegrees", 3, 45)).toBeNull();
  });

  it("upserts at the playhead when the property is keyframed, keeping the easing", () => {
    const item = clip({ keyframes: { rotationDegrees: [{ atSeconds: 1, value: 10, easing: "easeIn" }] } });
    expect(keyframedEditAction(item, "rotationDegrees", 3, 45)).toEqual({
      type: "upsertItemKeyframe",
      itemId: "clip",
      property: "rotationDegrees",
      keyframe: { atSeconds: 1, value: 45, easing: "easeIn" },
    });
    expect(keyframedEditAction(item, "rotationDegrees", 5, 90)).toEqual({
      type: "upsertItemKeyframe",
      itemId: "clip",
      property: "rotationDegrees",
      keyframe: { atSeconds: 3, value: 90 },
    });
  });
});
