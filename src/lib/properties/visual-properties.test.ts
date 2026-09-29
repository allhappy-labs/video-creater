import { describe, expect, it } from "vitest";
import type { VideoProject } from "@/lib/project";
import {
  blendModeAction,
  colorGrade,
  colorGradeAction,
  colorGradeRanges,
  cropActions,
  effectsAction,
  fadesAction,
  itemEffects,
  motionActions,
  opacityActions,
  removeEffectAction,
  resetColorGradeAction,
  speedActions,
  transformAction,
  visualBlendMode,
  visualCrop,
  visualFades,
  visualMotion,
  visualOpacity,
  visualPropertyRanges,
  visualSpeed,
  visualTransform,
} from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { fixtureProject } from "@/test-utils/editor-fixtures";

function clip(properties: Record<string, unknown> = {}, kind: TimelineItem["kind"] = "video_clip"): TimelineItem {
  return {
    id: "clip",
    kind,
    startSeconds: 2,
    durationSeconds: 4,
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

describe("visual readers", () => {
  it("return defaults for missing properties", () => {
    const item = clip();
    expect(visualTransform(item)).toEqual({
      centerX: 0.5,
      centerY: 0.5,
      width: 1,
      height: 1,
      flipHorizontal: false,
      flipVertical: false,
    });
    expect(visualMotion(item)).toEqual({ positionX: 0, positionY: 0, scale: 1, rotationDegrees: 0 });
    expect(visualOpacity(item)).toBe(1);
    expect(visualBlendMode(item)).toBe("over");
    expect(visualCrop(item)).toEqual({ cropTop: 0, cropRight: 0, cropBottom: 0, cropLeft: 0 });
    expect(visualFades(item)).toEqual({ fadeInSeconds: 0, fadeOutSeconds: 0 });
    expect(visualSpeed(item)).toBe(1);
    expect(colorGrade(item)).toEqual({ exposure: 0, contrast: 1, saturation: 1, temperature: 6500, tint: 0 });
    expect(itemEffects(item)).toEqual([]);
  });

  it("return defaults for invalid values", () => {
    const item = clip({
      transform: { centerX: 2, centerY: "x", width: 0, height: -1, flipHorizontal: "yes" },
      scale: 0,
      rotationDegrees: 720,
      positionX: Number.NaN,
      opacity: 1.5,
      blendMode: "glow",
      cropTop: 1,
      cropLeft: -0.2,
      fadeInSeconds: -1,
      fadeOutSeconds: "2",
      speed: 0,
      colorGrade: { exposure: 9, contrast: "1.2", temperature: 100 },
      effects: [{ effectType: 3 }],
    });
    expect(visualTransform(item)).toMatchObject({ centerX: 0.5, centerY: 0.5, width: 1, height: 1, flipHorizontal: false });
    expect(visualMotion(item)).toEqual({ positionX: 0, positionY: 0, scale: 1, rotationDegrees: 0 });
    expect(visualOpacity(item)).toBe(1);
    expect(visualBlendMode(item)).toBe("over");
    expect(visualCrop(item)).toEqual({ cropTop: 0, cropRight: 0, cropBottom: 0, cropLeft: 0 });
    expect(visualFades(item)).toEqual({ fadeInSeconds: 0, fadeOutSeconds: 0 });
    expect(visualSpeed(item)).toBe(1);
    expect(colorGrade(item)).toEqual({ exposure: 0, contrast: 1, saturation: 1, temperature: 6500, tint: 0 });
    expect(itemEffects(item)).toEqual([]);
  });

  it("read stored values", () => {
    const item = clip({
      transform: { centerX: 0.25, width: 0.5, flipVertical: true },
      scale: 1.2,
      opacity: 0.3,
      blendMode: "screen",
      cropRight: 0.1,
      fadeInSeconds: 0.5,
      speed: 2,
      colorGrade: { saturation: 0.4 },
      effects: [{ effectType: "blur.gaussian", enabled: true, params: { radius: 3 } }],
    });
    expect(visualTransform(item)).toMatchObject({ centerX: 0.25, width: 0.5, flipVertical: true });
    expect(visualMotion(item).scale).toBe(1.2);
    expect(visualOpacity(item)).toBe(0.3);
    expect(visualBlendMode(item)).toBe("screen");
    expect(visualCrop(item).cropRight).toBe(0.1);
    expect(visualFades(item).fadeInSeconds).toBe(0.5);
    expect(visualSpeed(item)).toBe(2);
    expect(colorGrade(item).saturation).toBe(0.4);
    expect(itemEffects(item)).toEqual([
      { effectInstanceId: "legacy:blur.gaussian:1", effectType: "blur.gaussian", enabled: true, params: { radius: 3 } },
    ]);
  });
});

describe("visual ranges", () => {
  it("match the legacy inspector inputs and backend grade bounds", () => {
    expect(visualPropertyRanges.opacity).toEqual({ min: 0, max: 1, step: 0.05 });
    expect(visualPropertyRanges.crop).toEqual({ min: 0, max: 0.99, step: 0.01 });
    expect(visualPropertyRanges.speed).toEqual({ min: 0.1, max: 8, step: 0.1 });
    expect(colorGradeRanges.exposure).toMatchObject({ min: -3, max: 3, step: 0.05 });
    expect(colorGradeRanges.contrast).toMatchObject({ min: 0.5, max: 1.5, step: 0.05 });
    expect(colorGradeRanges.saturation).toMatchObject({ min: 0, max: 2, step: 0.05 });
    expect(colorGradeRanges.temperature).toMatchObject({ min: 2000, max: 11000, defaultValue: 6500 });
  });
});

describe("transformAction", () => {
  it("emits updateVisualClipTransform", () => {
    expect(actionsOf(transformAction("clip", { centerX: 0.4, width: 0.8, flipHorizontal: true }))).toEqual([
      { type: "updateVisualClipTransform", itemId: "clip", transform: { centerX: 0.4, width: 0.8, flipHorizontal: true } },
    ]);
  });

  it("rejects empty and out-of-range transforms", () => {
    expect(blockedOf(transformAction("clip", {}))).toMatch(/transform/i);
    expect(blockedOf(transformAction("clip", { centerY: 1.2 }))).toMatch(/center/i);
    expect(blockedOf(transformAction("clip", { height: 0 }))).toMatch(/size/i);
  });
});

describe("motionActions", () => {
  it("sets the static property and removes it at the default, like legacy rotation", () => {
    expect(actionsOf(motionActions(clip(), "rotationDegrees", 30, 3))).toEqual([
      { type: "updateItemProperties", updates: [{ itemId: "clip", set: { rotationDegrees: 30 }, remove: [] }] },
    ]);
    expect(actionsOf(motionActions(clip({ rotationDegrees: 30 }), "rotationDegrees", 0, 3))).toEqual([
      { type: "updateItemProperties", updates: [{ itemId: "clip", set: {}, remove: ["rotationDegrees"] }] },
    ]);
    expect(actionsOf(motionActions(clip({ scale: 2 }), "scale", 1, 3))).toEqual([
      { type: "updateItemProperties", updates: [{ itemId: "clip", set: {}, remove: ["scale"] }] },
    ]);
  });

  it("upserts at the clip-local playhead when keyframed", () => {
    const item = clip({ keyframes: { rotationDegrees: [{ atSeconds: 0, value: 0 }] } });
    expect(actionsOf(motionActions(item, "rotationDegrees", 45, 10))).toEqual([
      {
        type: "upsertItemKeyframe",
        itemId: "clip",
        property: "rotationDegrees",
        keyframe: { atSeconds: 4, value: 45 },
      },
    ]);
  });

  it("rejects out-of-bounds values and non-visual items", () => {
    expect(blockedOf(motionActions(clip(), "scale", 0, 0))).toMatch(/scale/i);
    expect(blockedOf(motionActions(clip({}, "audio_clip"), "positionX", 5, 0))).toMatch(/visual/i);
  });
});

describe("opacityActions", () => {
  it("emits updateVisualClipOpacity when not keyframed", () => {
    expect(actionsOf(opacityActions(clip(), 0.5, 3))).toEqual([
      { type: "updateVisualClipOpacity", itemId: "clip", opacity: 0.5 },
    ]);
  });

  it("upserts when opacity is keyframed", () => {
    const item = clip({ keyframes: { opacity: [{ atSeconds: 0, value: 1 }] } });
    expect(actionsOf(opacityActions(item, 0.5, 3))).toEqual([
      { type: "upsertItemKeyframe", itemId: "clip", property: "opacity", keyframe: { atSeconds: 1, value: 0.5 } },
    ]);
  });

  it("rejects values outside 0 to 1", () => {
    expect(blockedOf(opacityActions(clip(), 1.1, 3))).toMatch(/opacity/i);
  });
});

describe("blendModeAction", () => {
  it("sets a blend mode and removes the property for over", () => {
    expect(blendModeAction("clip", "multiply")).toEqual({
      type: "updateItemProperties",
      updates: [{ itemId: "clip", set: { blendMode: "multiply" }, remove: [] }],
    });
    expect(blendModeAction("clip", "over")).toEqual({
      type: "updateItemProperties",
      updates: [{ itemId: "clip", set: {}, remove: ["blendMode"] }],
    });
  });
});

describe("cropActions", () => {
  it("emits updateVisualClipCrop when no side is keyframed", () => {
    expect(actionsOf(cropActions(clip(), { cropTop: 0.1, cropLeft: 0.2 }, 3))).toEqual([
      { type: "updateVisualClipCrop", itemId: "clip", crop: { cropTop: 0.1, cropLeft: 0.2 } },
    ]);
  });

  it("upserts all four sides when any side is keyframed", () => {
    const item = clip({ cropRight: 0.3, keyframes: { cropTop: [{ atSeconds: 0, value: 0 }] } });
    expect(actionsOf(cropActions(item, { cropTop: 0.1 }, 4.5))).toEqual([
      { type: "upsertItemKeyframe", itemId: "clip", property: "cropTop", keyframe: { atSeconds: 2.5, value: 0.1 } },
      { type: "upsertItemKeyframe", itemId: "clip", property: "cropRight", keyframe: { atSeconds: 2.5, value: 0.3 } },
      { type: "upsertItemKeyframe", itemId: "clip", property: "cropBottom", keyframe: { atSeconds: 2.5, value: 0 } },
      { type: "upsertItemKeyframe", itemId: "clip", property: "cropLeft", keyframe: { atSeconds: 2.5, value: 0 } },
    ]);
  });

  it("rejects crops that hide the frame", () => {
    expect(blockedOf(cropActions(clip(), { cropTop: 1 }, 0))).toMatch(/crop/i);
    expect(blockedOf(cropActions(clip({ cropLeft: 0.6 }), { cropRight: 0.4 }, 0))).toMatch(/visible/i);
  });
});

describe("fadesAction", () => {
  it("emits updateVisualClipFades", () => {
    expect(actionsOf(fadesAction(clip(), 1, 0.5))).toEqual([
      { type: "updateVisualClipFades", itemId: "clip", fadeInSeconds: 1, fadeOutSeconds: 0.5 },
    ]);
  });

  it("rejects negative fades and fades longer than the clip", () => {
    expect(blockedOf(fadesAction(clip(), -1, 0))).toMatch(/fade/i);
    expect(blockedOf(fadesAction(clip(), 3, 2))).toMatch(/fade/i);
  });
});

describe("speedActions", () => {
  it("reuses setItemSpeed: speed plus the rescaled duration", () => {
    const project: VideoProject = fixtureProject();
    expect(actionsOf(speedActions(project, "item-1", 2))).toEqual([
      { type: "updateVisualClipSpeed", itemId: "item-1", speed: 2 },
      { type: "resizeItems", resizes: [{ itemId: "item-1", durationSeconds: 2 }] },
    ]);
  });
});

describe("color grade and effects", () => {
  it("emits updateItemColorGrade with reset false", () => {
    expect(actionsOf(colorGradeAction(["clip"], { exposure: 0.5, temperature: 5000 }))).toEqual([
      { type: "updateItemColorGrade", itemIds: ["clip"], reset: false, grade: { exposure: 0.5, temperature: 5000 } },
    ]);
    expect(resetColorGradeAction(["clip"])).toEqual({
      type: "updateItemColorGrade",
      itemIds: ["clip"],
      reset: true,
      grade: {},
    });
  });

  it("rejects out-of-range grade values", () => {
    expect(blockedOf(colorGradeAction(["clip"], { contrast: 2 }))).toMatch(/contrast/i);
    expect(blockedOf(colorGradeAction(["clip"], { temperature: 100 }))).toMatch(/temperature/i);
  });

  it("emits updateItemEffects and removes one effect instance", () => {
    const effect = { effectInstanceId: "fx-1", effectType: "blur.gaussian", enabled: true, params: { radius: 2 } };
    const other = { effectInstanceId: "fx-2", effectType: "color.vignette", enabled: true, params: {} };
    expect(effectsAction(["clip"], [effect])).toEqual({ type: "updateItemEffects", itemIds: ["clip"], effects: [effect] });
    expect(removeEffectAction(clip({ effects: [effect, other] }), "fx-1")).toEqual({
      type: "updateItemEffects",
      itemIds: ["clip"],
      effects: [other],
    });
  });
});
