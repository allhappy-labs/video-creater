import { describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { createSampleProject } from "@/lib/sample-project";
import { createFixtureProjectStore, revisionConflictMessage } from "./fixture-project-store";

const projectDir = "/tmp/video-creater-editor-project";

function clipCount(project: VideoProject): number {
  return project.timeline.tracks.reduce((count, track) => count + track.items.length, 0);
}

describe("fixture project store", () => {
  it("saves a fresh folder as schema v2 at revision 1, running its seeds once", () => {
    const store = createFixtureProjectStore();
    const seed = vi.fn((project: VideoProject) => ({ ...project, name: "Seeded" }));
    store.addSeed(seed);
    expect(() => store.require()).toThrow();

    const saved = store.save(createSampleProject(), projectDir, 0);
    expect(saved).toMatchObject({ schemaVersion: 2, contentRevision: 1, name: "Seeded" });
    store.save({ ...saved, name: "Renamed" }, projectDir, 1);
    expect(seed).toHaveBeenCalledTimes(1);
    expect(store.require()).toMatchObject({ name: "Renamed", contentRevision: 2 });
  });

  it("increments the content revision on every applied batch, applying the actions locally", () => {
    const store = createFixtureProjectStore();
    const saved = store.save(createSampleProject(), projectDir, 0);
    const clip = saved.timeline.tracks.find((track) => track.kind === "video")?.items[0];
    if (!clip) throw new Error("the sample has a video clip");

    const split = store.apply([{ type: "splitItems", splits: [{ itemId: clip.id, newItemId: `${clip.id}-b`, splitSeconds: clip.startSeconds + 1 }] }]);
    expect(split.contentRevision).toBe(2);
    expect(clipCount(split)).toBe(clipCount(saved) + 1);
    const faded = store.apply([{ type: "updateVisualClipOpacity", itemId: clip.id, opacity: 0.5 }]);
    expect(faded.contentRevision).toBe(3);
  });

  it("rejects a stale expectedRevision with the native revision conflict message", () => {
    const store = createFixtureProjectStore();
    const saved = store.save(createSampleProject(), projectDir, 0);
    store.apply([{ type: "renameMedia", mediaId: "media-1", name: "Reel" }]);

    let rejection: unknown = null;
    try {
      store.save(saved, projectDir, 1);
    } catch (error) {
      rejection = error;
    }
    expect(rejection).toBe("project revision conflict: expected revision 1, but canonical revision is 2");
    expect(revisionConflictMessage(1, 2)).toBe(rejection);
    expect(store.require().contentRevision).toBe(2);
  });

  it("keeps the folder's worker-owned state over a saved copy, like preserve_worker_owned_state", () => {
    const store = createFixtureProjectStore();
    const saved = store.save({ ...createSampleProject(), transcripts: [] }, projectDir, 0);
    const job = { id: "job-1", kind: "transcribe_media", status: "completed" as const, updatedAt: "2026-09-15T10:00:00.000Z" };
    const transcript = { id: "transcript-media-1", mediaId: "media-1", repairs: [], segments: [], words: [] };
    store.record({ ...store.require(), jobs: [...saved.jobs, job], transcripts: [transcript] });
    expect(store.require().contentRevision).toBe(1);

    const restored = store.save({ ...saved, name: "Undo snapshot" }, projectDir, 1);
    expect(restored.name).toBe("Undo snapshot");
    expect(restored.jobs.map((candidate) => candidate.id)).toContain("job-1");
    expect(restored.transcripts).toEqual([transcript]);
  });

  it("advances registered job steps on each reload, then returns the folder's project", () => {
    const store = createFixtureProjectStore();
    store.save(createSampleProject(), projectDir, 0);
    const step = vi.fn(() => {
      store.record({ ...store.require(), name: `Tick ${step.mock.calls.length.toString()}` });
    });
    const stop = store.onReload(step);

    expect(store.reload().name).toBe("Tick 1");
    expect(store.reload().name).toBe("Tick 2");
    stop();
    expect(store.reload().name).toBe("Tick 2");
    expect(step).toHaveBeenCalledTimes(2);
  });
});
