import { describe, expect, it } from "vitest";
import type { ProjectActionEffect, VideoProject, VisualEffectDescriptor } from "@/lib/project";
import type { TimelineItem, TimelineItemKind } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import {
  effectParamControls,
  effectParamValue,
  effectRowActions,
  effectRows,
  withEffectParam,
} from "./effect-rows";
import { linkedAudioItem } from "./linked-audio";
import { lookPresetAction, lookPresetForGrade } from "./look-presets";
import { combineResults } from "./use-property-commit";

function item(id: string, kind: TimelineItemKind, properties: Record<string, unknown> = {}): TimelineItem {
  return { id, kind, startSeconds: 0, durationSeconds: 4, source: { type: "media", mediaId: "media-1" }, label: id, properties };
}

const blur: ProjectActionEffect = { effectInstanceId: "blur-1", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } };
const blurDescriptor: VisualEffectDescriptor = {
  id: "blur.gaussian",
  displayName: "Gaussian Blur",
  category: "Blur & Sharpen",
  params: [{ key: "radius", label: "Radius", min: 0, max: 50, defaultValue: 0, unit: "px" }],
  colorEffect: false,
};

describe("look presets", () => {
  it("recognizes the neutral grade as None and a stored preset by its values", () => {
    expect(lookPresetForGrade({ exposure: 0, contrast: 1, saturation: 1, temperature: 6500, tint: 0 })).toBe("none");
    expect(lookPresetForGrade({ exposure: 0, contrast: 1.15, saturation: 0, temperature: 6500, tint: 0 })).toBe("mono");
    expect(lookPresetForGrade({ exposure: 0.4, contrast: 1, saturation: 1, temperature: 6500, tint: 0 })).toBeNull();
  });

  it("resets for None and replaces the whole grade for a preset", () => {
    expect(lookPresetAction(["a"], "none")).toEqual({ type: "updateItemColorGrade", itemIds: ["a"], reset: true, grade: {} });
    expect(lookPresetAction(["a"], "warm")).toEqual({
      type: "updateItemColorGrade",
      itemIds: ["a"],
      reset: true,
      grade: { exposure: 0, contrast: 1, saturation: 1.1, temperature: 8000, tint: 0 },
    });
  });
});

describe("effect rows", () => {
  it("lists each instance for one item, skipping denoise", () => {
    const rows = effectRows([
      item("a", "video_clip", { effects: [blur, { effectInstanceId: "d", effectType: "audio.denoise", enabled: true, params: {} }] }),
    ]);
    expect(rows.map((row) => row.effectType)).toEqual(["blur.gaussian"]);
  });

  it("intersects effect types across a multiple selection", () => {
    const other = { effectInstanceId: "mono-1", effectType: "stylize.vignette", enabled: true, params: {} };
    const rows = effectRows([
      item("a", "video_clip", { effects: [blur, other] }),
      item("b", "image_clip", { effects: [{ ...blur, effectInstanceId: "blur-b", params: { radius: 9 } }] }),
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0]?.targets.map((target) => target.effect.effectInstanceId)).toEqual(["blur-1", "blur-b"]);
  });

  it("builds one updateItemEffects per item, replacing or removing the target instance", () => {
    const keep: ProjectActionEffect = { effectInstanceId: "keep", effectType: "stylize.vignette", enabled: true, params: {} };
    const a = item("a", "video_clip", { effects: [keep, blur] });
    const b = item("b", "video_clip", { effects: [{ ...blur, effectInstanceId: "blur-b" }] });
    const [row] = effectRows([a, b]);
    if (!row) throw new Error("row");
    expect(effectRowActions(row, (effect) => withEffectParam(effect, "radius", 12))).toEqual([
      { type: "updateItemEffects", itemIds: ["a"], effects: [keep, { ...blur, params: { radius: 12 } }] },
      { type: "updateItemEffects", itemIds: ["b"], effects: [{ ...blur, effectInstanceId: "blur-b", params: { radius: 12 } }] },
    ]);
    expect(effectRowActions(row, () => null)).toEqual([
      { type: "updateItemEffects", itemIds: ["a"], effects: [keep] },
      { type: "updateItemEffects", itemIds: ["b"], effects: [] },
    ]);
  });

  it("reads catalog params with a lane-like step and the legacy curve extras", () => {
    const [radius] = effectParamControls("blur.gaussian", blurDescriptor);
    expect(radius).toEqual({ key: "radius", label: "Radius (px)", min: 0, max: 50, step: 0.5, defaultValue: 0 });
    expect(effectParamValue(blur, "blur.gaussian", requiredControl(radius))).toBe(4);

    const curves = effectParamControls("color.curves", undefined);
    expect(curves.map((control) => [control.key, control.min, control.max])).toEqual([["curveMidpoint", 0, 1]]);
    const curveEffect: ProjectActionEffect = { effectInstanceId: "c", effectType: "color.curves", enabled: true, params: {} };
    const updated = withEffectParam(curveEffect, "curveMidpoint", 0.7);
    expect(updated.params).toEqual({ masterCurve: [[0, 0], [0.5, 0.7], [1, 1]] });
    expect(effectParamValue(updated, "color.curves", requiredControl(curves[0]))).toBe(0.7);

    const hue = effectParamControls("color.hueCurves", undefined);
    expect(hue.map((control) => [control.label, control.min, control.max])).toEqual([
      ["Target hue (°)", 0, 360],
      ["Hue shift (°)", -30, 30],
      ["Saturation", 0, 2],
      ["Luminance", -0.5, 0.5],
    ]);
    const hueEffect = withEffectParam({ ...curveEffect, effectType: "color.hueCurves" }, "hueShift", 10);
    expect(hueEffect.params).toEqual({ targets: [{ targetHue: 0, hueShift: 10, satScale: 1, lumShift: 0 }] });
  });
});

function requiredControl<T>(value: T | undefined): T {
  if (value === undefined) throw new Error("control");
  return value;
}

describe("linkedAudioItem", () => {
  function project(videoProps: Record<string, unknown>, audioProps: Record<string, unknown>): VideoProject {
    const base = fixtureProject();
    return {
      ...base,
      timeline: {
        durationSeconds: 4,
        tracks: [
          { id: "v", name: "Video", kind: "video", locked: false, enabled: true, items: [item("video", "video_clip", videoProps)] },
          { id: "a", name: "Audio", kind: "audio", locked: false, enabled: true, items: [item("audio", "audio_clip", audioProps)] },
        ],
      },
    };
  }

  it("finds the audio clip in the same link group", () => {
    const linked = project({ linkGroupId: "link-1" }, { linkGroupId: "link-1" });
    const video = linked.timeline.tracks[0]?.items[0];
    if (!video) throw new Error("video");
    expect(linkedAudioItem(linked, video)?.id).toBe("audio");
  });

  it("returns null without a shared link group", () => {
    const unlinked = project({ linkGroupId: "link-1" }, { linkGroupId: "link-2" });
    const video = unlinked.timeline.tracks[0]?.items[0];
    if (!video) throw new Error("video");
    expect(linkedAudioItem(unlinked, video)).toBeNull();
  });
});

describe("combineResults", () => {
  it("concatenates actions, or returns the first blocked message", () => {
    const action = { type: "updateVisualClipOpacity", itemId: "a", opacity: 1 } as const;
    expect(combineResults([{ actions: [action] }, { actions: [action] }])).toEqual({ actions: [action, action] });
    expect(combineResults([{ actions: [action] }, { blocked: "No" }, { blocked: "Later" }])).toEqual({ blocked: "No" });
  });
});
