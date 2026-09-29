import { describe, expect, it } from "vitest";
import type { VisualEffectDescriptor } from "@/lib/project";
import { projectWithTracks, testItem, track } from "../../properties/properties-test-utils";
import { effectCategories, effectMatchesSearch, effectTarget, panelEffects, planEffectApply } from "./effect-apply";

function descriptor(id: string, patch: Partial<VisualEffectDescriptor> = {}): VisualEffectDescriptor {
  return { id, displayName: id, category: "Stylize", params: [], colorEffect: false, ...patch };
}

const grain = descriptor("stylize.grain", {
  displayName: "Film Grain",
  params: [
    { key: "amount", label: "Amount", min: 0, max: 1, defaultValue: 0, unit: "" },
    { key: "size", label: "Size", min: 0.5, max: 4, defaultValue: 1.5, unit: "" },
  ],
});

function project(locked = false) {
  return projectWithTracks([
    track("v1", "video", [testItem("clip", "video_clip"), testItem("still", "image_clip")], locked),
    track("a1", "audio", [testItem("voice", "audio_clip")]),
    track("t1", "overlay", [{ ...testItem("title", "overlay"), source: { type: "text", text: "Hi" } }]),
  ]);
}

describe("effects panel catalog", () => {
  it("keeps clip effects and drops color grade and audio entries", () => {
    const catalog = [
      descriptor("color.exposure", { category: "Color", colorEffect: true }),
      descriptor("color.lut", { category: "Color", colorEffect: true, resourceKey: "path" }),
      descriptor("audio.denoise", { category: "Audio" }),
      grain,
      descriptor("blur.gaussian", { category: "Blur & Sharpen" }),
    ];
    expect(panelEffects(catalog).map((effect) => effect.id)).toEqual(["stylize.grain", "blur.gaussian"]);
    expect(effectCategories(panelEffects(catalog))).toEqual(["Blur & Sharpen", "Stylize"]);
  });

  it("searches id, name, category and resource key, trimmed and case-insensitive", () => {
    const lut = descriptor("stylize.look", { displayName: "Look", category: "Stylize", resourceKey: "lutPath" });
    expect(effectMatchesSearch(grain, "  film ")).toBe(true);
    expect(effectMatchesSearch(grain, "STYLIZE.GR")).toBe(true);
    expect(effectMatchesSearch(grain, "stylize")).toBe(true);
    expect(effectMatchesSearch(lut, "lutpath")).toBe(true);
    expect(effectMatchesSearch(grain, "blur")).toBe(false);
    expect(effectMatchesSearch(grain, "   ")).toBe(true);
  });
});

describe("effect target", () => {
  it("targets exactly one visual clip on an unlocked track", () => {
    expect(effectTarget(project(), ["clip"])).toEqual({ item: expect.objectContaining({ id: "clip" }) });
    expect(effectTarget(project(), ["still"])).toEqual({ item: expect.objectContaining({ id: "still" }) });
  });

  it("explains why there is no target", () => {
    expect(effectTarget(project(), [])).toEqual({ blocked: "Select a video or image clip" });
    expect(effectTarget(project(), ["voice"])).toEqual({ blocked: "Select a video or image clip" });
    expect(effectTarget(project(), ["title"])).toEqual({ blocked: "Select a video or image clip" });
    expect(effectTarget(project(), ["clip", "still"])).toEqual({ blocked: "Select one video or image clip at a time" });
    expect(effectTarget(project(true), ["clip"])).toEqual({ blocked: "Unlock the track to apply effects" });
  });
});

describe("planEffectApply", () => {
  it("appends a new instance with the descriptor defaults in one updateItemEffects", () => {
    const existing = { effectInstanceId: "legacy:blur.gaussian:1", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } };
    const base = project();
    const clip = base.timeline.tracks[0]?.items[0];
    if (clip) clip.properties = { effects: [existing] };
    expect(planEffectApply(base, "clip", grain)).toEqual({
      actions: [
        {
          type: "updateItemEffects",
          itemIds: ["clip"],
          effects: [
            existing,
            { effectInstanceId: "legacy:stylize.grain:1", effectType: "stylize.grain", enabled: true, params: { amount: 0, size: 1.5 } },
          ],
        },
      ],
    });
  });

  it("blocks an effect that is already applied, a locked track and a non-visual clip", () => {
    const base = project();
    const clip = base.timeline.tracks[0]?.items[0];
    if (clip) clip.properties = { effects: [{ effectInstanceId: "fx", effectType: "stylize.grain", enabled: true, params: {} }] };
    expect(planEffectApply(base, "clip", grain)).toEqual({ blocked: "Film Grain is already applied" });
    expect(planEffectApply(project(true), "clip", grain)).toEqual({ blocked: "Unlock the track to apply effects" });
    expect(planEffectApply(project(), "voice", grain)).toEqual({ blocked: "Select a video or image clip" });
    expect(planEffectApply(project(), "missing", grain)).toEqual({ blocked: "Select a video or image clip" });
  });
});
