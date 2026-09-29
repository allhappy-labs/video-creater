import { describe, expect, it } from "vitest";
import type { GeneratedAsset, VideoProject } from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { addedRecords, restoredProject, undoContent } from "./conversation-fixture-undo";

// Parity with `src-tauri/tests/codex_conversation/undo_background_generations.rs` (rule version 4).

const asset = "background-shot-1";
const outputMedia = "background-shot-1-output";

function generation(status: GeneratedAsset["status"]): GeneratedAsset {
  return {
    schemaVersion: 1,
    id: asset,
    kind: "generated",
    status,
    name: "Lab bench wide shot",
    placementIntent: "timeline",
    prompt: "A lab bench in a bright laboratory",
    model: { provider: "fal.ai", id: "fal-ai/veo3/fast" },
    references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null },
    settings: { width: 1280, height: 720, durationSeconds: 4, fps: 24, aspectRatio: "16:9" },
    outputs: [],
    createdAt: "2026-09-16T10:00:00Z",
    parentAssetId: null,
    retryOfAssetId: null,
  };
}

/** Batch 1 recorded the generation, and a runner marked it running. */
function running(): VideoProject {
  const project = createSampleProject();
  return { ...project, generatedAssets: [...project.generatedAssets, { ...generation("running"), references: { mediaIds: [], firstFrameMediaId: null, lastFrameMediaId: null, providerInputUrls: ["https://fal.media/files/input.png"] } }] };
}

/** Batch 2: a safe volume change on the first clip. */
function withVolumeChange(project: VideoProject): VideoProject {
  const next = structuredClone(project);
  const item = next.timeline.tracks[0]?.items[0];
  if (!item) throw new Error("sample project has a clip");
  item.properties = { ...item.properties, volumeDb: -2 };
  return next;
}

/** A runner write completing the generation, with the library media completion adds. */
function completed(project: VideoProject): VideoProject {
  const output = { mediaId: outputMedia, relativePath: "generated/background-shot-1/output.mp4", sourceUrl: "https://fal.media/files/output.mp4", width: 1280, height: 720, durationSeconds: 4, fps: 24 };
  return {
    ...project,
    generatedAssets: project.generatedAssets.map((candidate) => (candidate.id === asset ? { ...candidate, status: "completed", outputs: [output] } : candidate)),
    media: [...project.media, { id: outputMedia, name: null, relativePath: output.relativePath, kind: "generated", durationSeconds: 4, width: 1280, height: 720, fps: 24, folderId: null }],
  };
}

describe("conversation fixture undo, rule version 4", () => {
  it("lets an earlier generation's progress through a later batch's undo and keeps that progress", () => {
    const before = running();
    const after = withVolumeChange(before);
    const records = addedRecords(before, after);
    expect(records.backgroundGeneratedAssetIds).toContain(asset);
    const afterContent = undoContent(after, records.addedGeneratedAssetIds, records.backgroundGeneratedAssetIds);
    const current = completed(after);

    expect(undoContent(current, records.addedGeneratedAssetIds, records.backgroundGeneratedAssetIds)).toBe(afterContent);
    const restored = restoredProject(before, records, current);
    expect(restored.timeline).toEqual(before.timeline);
    const generationAfterUndo = restored.generatedAssets.find((candidate) => candidate.id === asset);
    expect(generationAfterUndo?.status).toBe("completed");
    expect(generationAfterUndo?.outputs.map((output) => output.mediaId)).toEqual([outputMedia]);
    expect(restored.media.map((media) => media.id)).toContain(outputMedia);
  });

  it("still conflicts when the earlier generation's output was placed on the timeline", () => {
    const before = running();
    const after = withVolumeChange(before);
    const records = addedRecords(before, after);
    const afterContent = undoContent(after, records.addedGeneratedAssetIds, records.backgroundGeneratedAssetIds);
    const placed = structuredClone(completed(after));
    placed.timeline.tracks[0]?.items.push({ id: "placed", kind: "video_clip", startSeconds: 8, durationSeconds: 4, source: { type: "media", mediaId: outputMedia }, label: "Lab bench wide shot", properties: {} });

    expect(undoContent(placed, records.addedGeneratedAssetIds, records.backgroundGeneratedAssetIds)).not.toBe(afterContent);
  });

  it("keeps a later removal of an earlier generation's unused output media", () => {
    const before = completed(running());
    const after = withVolumeChange(before);
    const records = addedRecords(before, after);
    const current = { ...after, media: after.media.filter((media) => media.id !== outputMedia) };

    expect(restoredProject(before, records, current).media).toEqual(current.media);
  });

  it("keeps the version 3 rule when no background ids are given", () => {
    const before = running();
    const after = withVolumeChange(before);
    const records = addedRecords(before, after);

    expect(undoContent(completed(after), records.addedGeneratedAssetIds)).not.toBe(undoContent(after, records.addedGeneratedAssetIds));
  });
});
