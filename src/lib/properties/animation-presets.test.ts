import { describe, expect, it } from "vitest";
import { applyProjectActionLocally, type ProjectAction, type VideoProject } from "@/lib/project";
import {
  animationPresetActions,
  animationPresets,
  appliedAnimationPreset,
  clearAnimationPresetActions,
  motionPresetAction,
} from "@/lib/properties/animation-presets";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function clip(properties: Record<string, unknown> = {}, durationSeconds = 4, kind: TimelineItem["kind"] = "video_clip"): TimelineItem {
  return {
    id: "clip",
    kind,
    startSeconds: 1,
    durationSeconds,
    source: { type: "media", mediaId: "media-1" },
    label: "Clip",
    properties,
  };
}

function actionsOf(result: CommandResult) {
  if ("blocked" in result) throw new Error(`Unexpectedly blocked: ${result.blocked}`);
  return result.actions;
}

function blockedOf(result: CommandResult) {
  if (!("blocked" in result)) throw new Error("Expected the edit to be blocked");
  return result.blocked;
}

function applied(item: TimelineItem, actions: readonly ProjectAction[]): TimelineItem {
  const base = fixtureProject();
  const project: VideoProject = {
    ...base,
    timeline: {
      durationSeconds: item.startSeconds + item.durationSeconds,
      tracks: [{ id: "video", name: "Video", kind: "video", locked: false, enabled: true, items: [item] }],
    },
  };
  const next = actions.reduce(applyProjectActionLocally, project).timeline.tracks[0]?.items[0];
  if (!next) throw new Error("clip missing");
  return next;
}

describe("animationPresets", () => {
  it("groups catalog presets into in, out and loop phases with catalog labels", () => {
    expect(animationPresets("in").map((preset) => preset.id)).toEqual([
      "slide-fade-up-v1",
      "spring-pop-v2",
      "slide-rotate-settle-v2",
    ]);
    expect(animationPresets("out").map((preset) => preset.id)).toEqual(["exit-snap-v2"]);
    expect(animationPresets("loop")).toEqual([
      {
        id: "pulse-emphasis-v2",
        phase: "loop",
        label: "Pulse Emphasis V2",
        description: "Restrained emphasis pulse for callouts and accent elements.",
      },
    ]);
  });
});

describe("animationPresetActions", () => {
  it("sets keyframe lanes over the in window and records the preset", () => {
    expect(actionsOf(animationPresetActions(clip({ opacity: 0.8 }), "slide-fade-up-v1"))).toEqual([
      {
        type: "setItemKeyframes",
        itemId: "clip",
        property: "opacity",
        keyframes: [
          { atSeconds: 0, value: 0, easing: "easeOut" },
          { atSeconds: 0.5, value: 0.8, easing: "easeOut" },
        ],
      },
      {
        type: "setItemKeyframes",
        itemId: "clip",
        property: "positionY",
        keyframes: [
          { atSeconds: 0, value: 80, easing: "easeOut" },
          { atSeconds: 0.5, value: 0, easing: "easeOut" },
        ],
      },
      {
        type: "updateItemProperties",
        updates: [{ itemId: "clip", set: { animationInPresetId: "slide-fade-up-v1" }, remove: [] }],
      },
    ]);
  });

  it("keeps keyframes outside the window and settles on the value at the window edge", () => {
    const item = clip({ keyframes: { opacity: [{ atSeconds: 2, value: 0.5 }, { atSeconds: 0.2, value: 1 }] } });
    const [opacity] = actionsOf(animationPresetActions(item, "exit-snap-v2"));
    expect(opacity).toEqual({
      type: "setItemKeyframes",
      itemId: "clip",
      property: "opacity",
      keyframes: [
        { atSeconds: 0.2, value: 1 },
        { atSeconds: 2, value: 0.5 },
        { atSeconds: 3.5, value: 0.5, easing: "easeIn" },
        { atSeconds: 4, value: 0, easing: "easeIn" },
      ],
    });
  });

  it("repeats the loop preset over the whole clip", () => {
    const [scale] = actionsOf(animationPresetActions(clip({ scale: 2 }, 2), "pulse-emphasis-v2"));
    expect(scale).toEqual({
      type: "setItemKeyframes",
      itemId: "clip",
      property: "scale",
      keyframes: [
        { atSeconds: 0, value: 2, easing: "easeInOut" },
        { atSeconds: 0.5, value: 2.12, easing: "easeInOut" },
        { atSeconds: 1, value: 2, easing: "easeInOut" },
        { atSeconds: 1.5, value: 2.12, easing: "easeInOut" },
        { atSeconds: 2, value: 2, easing: "easeInOut" },
      ],
    });
  });

  it("uses half of a short clip as the window", () => {
    const [opacity] = actionsOf(animationPresetActions(clip({}, 0.6), "slide-fade-up-v1"));
    expect(opacity).toMatchObject({ keyframes: [{ atSeconds: 0 }, { atSeconds: 0.3 }] });
  });

  it("clears lanes that only the replaced preset of the same phase animated", () => {
    const withRotate = applied(clip(), actionsOf(animationPresetActions(clip(), "slide-rotate-settle-v2")));
    expect(appliedAnimationPreset(withRotate, "in")).toBe("slide-rotate-settle-v2");
    const actions = actionsOf(animationPresetActions(withRotate, "spring-pop-v2"));
    expect(actions.map((action) => (action.type === "setItemKeyframes" ? action.property : action.type))).toEqual([
      "opacity",
      "scale",
      "positionX",
      "rotationDegrees",
      "updateItemProperties",
    ]);
    const next = applied(withRotate, actions);
    expect(Object.keys(next.properties.keyframes as object).sort()).toEqual(["opacity", "scale"]);
    expect(appliedAnimationPreset(next, "in")).toBe("spring-pop-v2");
  });

  it("applies cleanly to a project and blocks non-visual items", () => {
    const item = clip();
    expect(() => applied(item, actionsOf(animationPresetActions(item, "spring-pop-v2")))).not.toThrow();
    expect(blockedOf(animationPresetActions(clip({}, 4, "caption"), "spring-pop-v2"))).toMatch(/visual/i);
    expect(blockedOf(animationPresetActions(clip({}, 4, "audio_clip"), "exit-snap-v2"))).toMatch(/visual/i);
    expect(blockedOf(animationPresetActions(clip(), "snap-pop-v1"))).toMatch(/preset/i);
  });
});

describe("clearAnimationPresetActions", () => {
  it("removes the recorded preset's window keyframes and its id", () => {
    const item = clip({
      animationOutPresetId: "exit-snap-v2",
      keyframes: {
        opacity: [{ atSeconds: 1, value: 1 }, { atSeconds: 3.5, value: 1 }, { atSeconds: 4, value: 0 }],
        scale: [{ atSeconds: 3.5, value: 1 }, { atSeconds: 4, value: 0.9 }],
      },
    });
    const actions = actionsOf(clearAnimationPresetActions(item, "out"));
    expect(actions).toContainEqual({
      type: "setItemKeyframes",
      itemId: "clip",
      property: "opacity",
      keyframes: [{ atSeconds: 1, value: 1 }],
    });
    expect(actions).toContainEqual({ type: "setItemKeyframes", itemId: "clip", property: "scale", keyframes: [] });
    expect(actions.at(-1)).toEqual({
      type: "updateItemProperties",
      updates: [{ itemId: "clip", set: {}, remove: ["animationOutPresetId"] }],
    });
  });

  it("does nothing when no preset is recorded", () => {
    expect(actionsOf(clearAnimationPresetActions(clip(), "in"))).toEqual([]);
    expect(appliedAnimationPreset(clip({ animationInPresetId: "nope" }), "in")).toBeNull();
  });
});

describe("motionPresetAction", () => {
  it("sets motionPresetId on every item, like the pre-cut caption group motion", () => {
    expect(motionPresetAction(["a", "b"], "pulse-emphasis-v2")).toEqual({
      type: "updateItemProperties",
      updates: [
        { itemId: "a", set: { motionPresetId: "pulse-emphasis-v2" }, remove: [] },
        { itemId: "b", set: { motionPresetId: "pulse-emphasis-v2" }, remove: [] },
      ],
    });
  });
});
