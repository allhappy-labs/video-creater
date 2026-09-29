import { describe, expect, it } from "vitest";
import type { GeneratedAsset, MediaAsset, VideoProject } from "@/lib/project";
import { fixtureGeneratedAsset, fixtureProject } from "@/test-utils/editor-fixtures";
import {
  generatedLineageText,
  generatedOutputReplacementBlocker,
  generationCostLabel,
  generationReferenceTiles,
  upscaleRequestPlan,
  variationChoices,
  variationDraftsForAsset,
  videoAudioRequestPlan,
} from "./clip-actions";

const seedance = { provider: "replicate", id: "bytedance/seedance-2.0-fast" };

function asset(overrides: Partial<GeneratedAsset>): GeneratedAsset {
  return { ...fixtureGeneratedAsset(fixtureProject()), ...overrides };
}

function splitProject(patch: Partial<VideoProject> = {}): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, ...patch };
}

const still: MediaAsset = { id: "still", relativePath: "media/still.png", kind: "image", durationSeconds: 0, width: 800, height: 600, fps: null, folderId: null };

describe("clip generation requests", () => {
  it("upscales generated media by its content type and refuses audio", () => {
    const project = { ...fixtureProject(), media: [...fixtureProject().media, still] };
    const generated = upscaleRequestPlan(project, "sample-generated-output");
    expect(generated).toMatchObject({ request: { model: { id: "fal-ai/video-upscaler" }, placementIntent: "library", settings: { width: 1280, height: 720 } } });
    expect(upscaleRequestPlan(project, "still")).toMatchObject({ request: { model: { id: "fal-ai/aura-sr" } } });
    expect(upscaleRequestPlan(project, "media-voiceover")).toEqual({ blocked: "Only images and videos can be upscaled." });
    expect(upscaleRequestPlan(project, "gone")).toEqual({ blocked: "That media is no longer in the project." });
  });

  it("generates music or sound only from video media", () => {
    const project = { ...fixtureProject(), media: [...fixtureProject().media, still] };
    const context = { itemId: "item-1", timelineStartSeconds: 2, durationSeconds: 4, sourceIn: 0, sourceOut: 4 };
    expect(videoAudioRequestPlan(project, "media-1", "sfx", context)).toMatchObject({
      request: { kind: "audio", placementIntent: "timeline", model: { id: "mirelo-ai/sfx-v1.5/video-to-audio" }, settings: { timelineStartSeconds: 2 } },
    });
    expect(videoAudioRequestPlan(project, "still", "music", context)).toEqual({ blocked: "Music and sound effects need a video clip." });
  });

  it("varies the original prompt once, or uses distinct default drafts for a set", () => {
    const original = asset({ name: "Hero", prompt: "A red kite" });
    expect(variationDraftsForAsset(original, 1)).toEqual([{ name: "Hero", prompt: "A red kite" }]);
    expect(variationDraftsForAsset(original, 2).map((draft) => draft.name)).toEqual(["Storm Clouds", "Radiant Backlight"]);
    expect(variationDraftsForAsset(original, 4)).toHaveLength(4);
    expect(variationDraftsForAsset(asset({ prompt: "  " }), 1)).toEqual([]);
  });
});

describe("generation cost", () => {
  it("prices the asset's model and settings, multiplied by the run count", () => {
    const input = { kind: "generated" as const, model: seedance, prompt: "A kite", settings: { width: 1280, height: 720, durationSeconds: 5, fps: 24, aspectRatio: "16:9", resolution: "720p", generateAudio: false } };
    expect(generationCostLabel(input, null)).toBe("Est. 4 credits");
    expect(generationCostLabel(input, null, 2)).toBe("Est. 8 credits");
    expect(generationCostLabel({ ...input, model: { provider: "fal.ai", id: "fal-ai/video-upscaler" } }, null)).toBe("Est. varies");
  });
});

describe("generated details", () => {
  it("labels lineage by generation title, never by id", () => {
    const root = asset({ id: "root", name: "Hero shot" });
    const child = asset({ id: "child", name: null, prompt: "Kite", parentAssetId: "root", retryOfAssetId: "root" });
    const grandchild = asset({ id: "grand", parentAssetId: "root", retryOfAssetId: "child" });
    const project = { ...fixtureProject(), generatedAssets: [root, child, grandchild] };
    expect(generatedLineageText(project, root)).toBeNull();
    expect(generatedLineageText(project, child)).toBe("Variation of “Hero shot”");
    expect(generatedLineageText(project, grandchild)).toBe("Variation of “Hero shot” · based on “Kite”");
    expect(generatedLineageText({ ...project, generatedAssets: [grandchild] }, grandchild)).toBe("Variation of an earlier generation · based on an earlier generation");
  });

  it("lists frames, source video and references once each", () => {
    const project = fixtureProject();
    const tiles = generationReferenceTiles(project, asset({ references: { mediaIds: ["media-1", "gone"], firstFrameMediaId: "media-1", lastFrameMediaId: null, sourceVideoMediaRef: "media-1" } }));
    expect(tiles.map((tile) => [tile.role, tile.media?.id ?? null])).toEqual([
      ["First frame", "media-1"],
      ["Source video", "media-1"],
      ["Reference", null],
    ]);
  });

  it("lists the variation family oldest first with readable labels", () => {
    const root = asset({ id: "root", name: "Hero", createdAt: "2026-01-01T00:00:00Z" });
    const retry = asset({ id: "retry", name: "Hero", parentAssetId: "root", retryOfAssetId: "root", createdAt: "2026-01-03T00:00:00Z", status: "running", outputs: [] });
    const storm = asset({ id: "storm", name: "Storm Clouds", parentAssetId: "root", retryOfAssetId: "root", createdAt: "2026-01-02T00:00:00Z", outputs: [{ mediaId: "storm-out", relativePath: "a.mp4", width: 1, height: 1, durationSeconds: 1, fps: 1 }] });
    const unrelated = asset({ id: "other", createdAt: "2026-01-01T00:00:00Z" });
    const choices = variationChoices({ ...fixtureProject(), generatedAssets: [retry, storm, root, unrelated] }, storm);
    expect(choices.map((choice) => [choice.label, choice.mediaId, choice.status])).toEqual([
      ["Hero", "sample-generated-output", "completed"],
      ["Storm Clouds", "storm-out", "completed"],
      ["Variation 2", null, "running"],
    ]);
  });
});

describe("generated output replacement", () => {
  it("allows a completed generated output on a visual clip in a split project", () => {
    expect(generatedOutputReplacementBlocker(splitProject(), "/p", "item-1", "sample-generated-output")).toBeNull();
  });

  it("explains each blocked case", () => {
    const project = splitProject();
    expect(generatedOutputReplacementBlocker(fixtureProject(), "/p", "item-1", "sample-generated-output")).toBe("Save the project to a folder to swap generated outputs.");
    expect(generatedOutputReplacementBlocker(project, "", "item-1", "sample-generated-output")).toBe("Save the project to a folder to swap generated outputs.");
    expect(generatedOutputReplacementBlocker(project, "/p", "gone", "sample-generated-output")).toBe("That clip is no longer on the timeline.");
    expect(generatedOutputReplacementBlocker(project, "/p", "music-bed", "sample-generated-output")).toBe("Audio clips can't swap generated outputs yet.");
    expect(generatedOutputReplacementBlocker(project, "/p", "item-1", "media-1")).toBe("Only generated outputs can replace this clip.");
    expect(generatedOutputReplacementBlocker(project, "/p", "item-1", "gone")).toBe("That output is no longer in the project.");
    const locked = splitProject({ timeline: { ...project.timeline, tracks: project.timeline.tracks.map((track) => ({ ...track, locked: track.kind === "video" })) } });
    expect(generatedOutputReplacementBlocker(locked, "/p", "item-1", "sample-generated-output")).toBe("Unlock the track to replace this clip.");
    const pending = splitProject({ generatedAssets: project.generatedAssets.map((candidate) => ({ ...candidate, status: "running" })) });
    expect(generatedOutputReplacementBlocker(pending, "/p", "item-1", "sample-generated-output")).toBe("That output isn't ready yet.");
  });
});
