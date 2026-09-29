import { describe, expect, it } from "vitest";
import type { TimelineItem, TimelineItemKind } from "@/lib/timeline";
import {
  addKeyframeAction,
  draggedKeyframe,
  keyframeEditActions,
  keyframeKeyboardEdit,
  keyframeLaneFor,
} from "@/lib/timeline-ops/keyframe-lane";

function item(kind: TimelineItemKind, properties: Record<string, unknown> = {}): TimelineItem {
  return { id: "clip", kind, startSeconds: 2, durationSeconds: 4, source: { type: "media", mediaId: "media-1" }, label: "Clip", properties };
}

const opacityKeyframes = { keyframes: { opacity: [{ atSeconds: 3, value: 0.2, easing: "easeIn" }, { atSeconds: 1, value: 1 }] } };

describe("keyframeLaneFor", () => {
  it("uses the preferred property when the clip supports it, else the first config, with sorted keyframes", () => {
    const video = keyframeLaneFor(item("video_clip", opacityKeyframes), "scale");
    expect(video?.config.property).toBe("scale");
    expect(video?.configs.map((config) => config.label)).toContain("Opacity");

    const opacity = keyframeLaneFor(item("video_clip", opacityKeyframes), "opacity");
    expect(opacity?.keyframes.map((keyframe) => keyframe.atSeconds)).toEqual([1, 3]);

    const audio = keyframeLaneFor(item("audio_clip"), "opacity");
    expect(audio?.config.property).toBe("volumeDb");
    expect(audio?.configs.map((config) => config.label)).toEqual(["Volume dB"]);
  });
});

describe("keyframe edits", () => {
  const clip = item("video_clip", opacityKeyframes);
  const lane = keyframeLaneFor(clip, "opacity");
  if (!lane) throw new Error("expected an opacity lane");
  const from = { atSeconds: 3, value: 0.2, easing: "easeIn" as const };

  it("clamps and rounds a dragged keyframe to the clip and the value bounds", () => {
    expect(draggedKeyframe(clip, lane.config, from, 0.12345, 0.3)).toEqual({ atSeconds: 3.123, value: 0.5 });
    expect(draggedKeyframe(clip, lane.config, from, 9, -5)).toEqual({ atSeconds: 4, value: 0 });
    expect(draggedKeyframe(clip, lane.config, from, -9, 5)).toEqual({ atSeconds: 0, value: 1 });
  });

  it("moves in time with moveItemKeyframe and changes value with upsertItemKeyframe, keeping the easing", () => {
    expect(keyframeEditActions("clip", "opacity", from, { atSeconds: 3.5, value: 0.6 })).toEqual([
      { type: "moveItemKeyframe", itemId: "clip", property: "opacity", fromSeconds: 3, toSeconds: 3.5 },
      { type: "upsertItemKeyframe", itemId: "clip", property: "opacity", keyframe: { atSeconds: 3.5, value: 0.6, easing: "easeIn" } },
    ]);
    expect(keyframeEditActions("clip", "opacity", from, { atSeconds: 3.5, value: 0.2 })).toEqual([
      { type: "moveItemKeyframe", itemId: "clip", property: "opacity", fromSeconds: 3, toSeconds: 3.5 },
    ]);
    expect(keyframeEditActions("clip", "opacity", from, { atSeconds: 3, value: 0.2 })).toEqual([]);
  });

  it("maps keys to delete, time steps of 0.1 s (1 s with Shift) and value steps (x10 with Shift)", () => {
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "Delete", shiftKey: false })).toBe("delete");
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "Backspace", shiftKey: false })).toBe("delete");
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "ArrowLeft", shiftKey: false })).toEqual({ atSeconds: 2.9, value: 0.2 });
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "ArrowRight", shiftKey: true })).toEqual({ atSeconds: 4, value: 0.2 });
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "ArrowUp", shiftKey: false })).toEqual({ atSeconds: 3, value: 0.25 });
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "ArrowDown", shiftKey: true })).toEqual({ atSeconds: 3, value: 0 });
    expect(keyframeKeyboardEdit(clip, lane.config, from, { key: "Enter", shiftKey: false })).toBeNull();
  });

  it("adds a keyframe at a clip-local time with the interpolated suggested value", () => {
    expect(addKeyframeAction(clip, lane.config, lane.keyframes, 2.0004)).toEqual({
      type: "upsertItemKeyframe",
      itemId: "clip",
      property: "opacity",
      keyframe: { atSeconds: 2, value: 0.6, easing: "linear" },
    });
    expect(addKeyframeAction(clip, lane.config, lane.keyframes, 12)).toMatchObject({ keyframe: { atSeconds: 4, value: 0.2 } });
  });
});
