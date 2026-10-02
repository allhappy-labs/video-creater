import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, ProjectJobSummary, VideoProject } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("./editor-store");

function splitProject(): VideoProject {
  return { ...fixtureProject(), schemaVersion: 2, contentRevision: 3 };
}

function opacityAction(project: VideoProject, opacity = 0.5): ProjectAction {
  return { type: "updateVisualClipOpacity", itemId: fixtureItem(project, "video").id, opacity };
}

async function flushAsyncWork(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe("project slice", () => {
  // Block body: returning the mock would make Vitest call it as a teardown function.
  beforeEach(() => {
    backendRequest.mockReset();
  });

  it("persists split-project actions through the backend and records one undo step", async () => {
    const project = splitProject();
    const saved = { ...project, contentRevision: 4, name: "saved" };
    backendRequest.mockResolvedValueOnce({ project: saved });
    const store = createEditorStore({ projectDir: "/p", project });

    const result = await store.getState().applyActions([opacityAction(project)]);

    expect(backendRequest).toHaveBeenCalledWith("apply_project_actions_to_split_project_folder", {
      projectDir: "/p",
      actions: [opacityAction(project)],
      expectedRevision: 3,
    });
    expect(result).toEqual(saved);
    expect(store.getState().project).toEqual(saved);
    expect(store.getState().saveStatus).toBe("saved");
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().canUndo()).toBe(true);
  });

  it("falls back to local application when the backend is unavailable", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValueOnce(new BackendUnavailableError());
    const store = createEditorStore({ projectDir: "/p", project });

    const result = await store.getState().applyActions([opacityAction(project)]);

    expect(result).not.toBeNull();
    expect(result).not.toBe(project);
    expect(store.getState().saveStatus).toBe("unsaved");
    expect(store.getState().history.past).toHaveLength(1);
  });

  it("does not record undo steps for job bookkeeping actions", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValueOnce(new BackendUnavailableError());
    const store = createEditorStore({ projectDir: "/p", project });
    const job: ProjectJobSummary = {
      id: "project-slice-test-job",
      kind: "render_draft",
      status: "queued",
      updatedAt: "2026-09-13T00:00:00Z",
    };

    const result = await store.getState().applyActions([{ type: "recordJob", job }]);

    expect(result?.jobs.some((entry) => entry.id === job.id)).toBe(true);
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("reports failure without mutating when the backend rejects the action", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValueOnce(new Error("track is locked"));
    const store = createEditorStore({ projectDir: "/p", project });

    const result = await store.getState().applyActions([opacityAction(project)]);

    expect(result).toBeNull();
    expect(store.getState().project).toBe(project);
    expect(store.getState().saveStatus).toBe("failed");
    expect(store.getState().lastError).toBe("track is locked");
  });

  it("undoes and redoes by persisting snapshots with the expected revision", async () => {
    const project = splitProject();
    const edited = { ...project, contentRevision: 4, name: "edited" };
    const restored = { ...project, contentRevision: 5 };
    const redone = { ...edited, contentRevision: 6 };
    backendRequest
      .mockResolvedValueOnce({ project: edited })
      .mockResolvedValueOnce({ project: restored })
      .mockResolvedValueOnce({ project: redone });
    const store = createEditorStore({ projectDir: "/p", project });

    await store.getState().applyActions([opacityAction(project)]);
    await store.getState().undo();

    expect(backendRequest).toHaveBeenLastCalledWith("save_split_project_to_folder", {
      projectDir: "/p",
      project,
      expectedRevision: 4,
      activateProject: false,
    });
    expect(store.getState().project).toEqual(restored);
    expect(store.getState().canRedo()).toBe(true);

    await store.getState().redo();
    expect(backendRequest).toHaveBeenLastCalledWith("save_split_project_to_folder", {
      projectDir: "/p",
      project: edited,
      expectedRevision: 5,
      activateProject: false,
    });
    expect(store.getState().project).toEqual(redone);
    expect(store.getState().canRedo()).toBe(false);
  });

  it("caps undo history at 100 snapshots", async () => {
    const project = splitProject();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    const store = createEditorStore({ projectDir: "/p", project });
    for (let index = 0; index < 105; index += 1) {
      await store.getState().applyActions([opacityAction(store.getState().project, (index % 10) / 10)]);
    }
    expect(store.getState().history.past).toHaveLength(100);
  });

  describe("canonical reconciliation", () => {
    it("keeps an acknowledged edit when an older snapshot waits behind its write", async () => {
      const project = splitProject();
      const edited = { ...project, contentRevision: 4, name: "acknowledged edit" };
      let resolveWrite: (value: unknown) => void = () => undefined;
      backendRequest.mockImplementationOnce(() => new Promise((resolve) => { resolveWrite = resolve; }));
      const store = createEditorStore({ projectDir: "/p", project });

      const write = store.getState().applyActions([opacityAction(project)]);
      const reconciliation = store.getState().installReconciledProject(project);
      await flushAsyncWork();
      resolveWrite({ project: edited });
      await write;
      const history = store.getState().history;
      await reconciliation;

      expect(store.getState().project).toBe(edited);
      expect(store.getState().history).toBe(history);
      expect(store.getState().saveStatus).toBe("saved");
    });

    it("preserves redo history when a newer canonical snapshot is already installed", async () => {
      const project = splitProject();
      const edited = { ...project, contentRevision: 4, name: "edited" };
      const restored = { ...project, contentRevision: 5 };
      backendRequest.mockResolvedValueOnce({ project: edited }).mockResolvedValueOnce({ project: restored });
      const store = createEditorStore({ projectDir: "/p", project });
      await store.getState().applyActions([opacityAction(project)]);
      await store.getState().undo();
      const history = store.getState().history;
      expect(store.getState().canRedo()).toBe(true);

      await store.getState().installReconciledProject(edited);

      expect(store.getState().project).toBe(restored);
      expect(store.getState().history).toBe(history);
      expect(store.getState().canRedo()).toBe(true);
    });

    it("rejects a snapshot for a different project without changing the editor", async () => {
      const project = splitProject();
      const store = createEditorStore({ projectDir: "/p", project });
      const state = store.getState();

      await expect(store.getState().installReconciledProject({ ...project, id: "replacement", contentRevision: 9 })).rejects.toThrow("different project");

      expect(store.getState()).toBe(state);
    });

    it.each(["completed", "cancelled"] as const)("keeps %s worker state over an older snapshot at the same edit revision", async (status) => {
      const project = splitProject();
      const job: ProjectJobSummary = {
        id: "admitted-render", kind: "export_media", status,
        updatedAt: "2026-10-01T12:00:02Z",
        workflow: { workflowId: "render", runId: "attempt-1", workflowType: "render", taskQueue: "render", activityTypes: [] },
      };
      const current = { ...project, jobs: [job] };
      // The native renderer stamps all transitions with the admission's updatedAt.
      const older = { ...project, jobs: [{ ...job, status: "queued" as const }] };
      const store = createEditorStore({ projectDir: "/p", project: current });

      await store.getState().installReconciledProject(older);

      expect(store.getState().project.jobs).toEqual([job]);
      expect(store.getState().project.contentRevision).toBe(project.contentRevision);
      expect(store.getState().project.timeline).toBe(current.timeline);
    });

    it.each([0, 1])("preserves redo only when reconciliation has no new content revision (+%s)", async (revisionIncrease) => {
      const project = splitProject();
      const edited = { ...project, contentRevision: 4, name: "edited" };
      const restored = { ...project, contentRevision: 5 };
      backendRequest.mockResolvedValueOnce({ project: edited }).mockResolvedValueOnce({ project: restored });
      const store = createEditorStore({ projectDir: "/p", project });
      await store.getState().applyActions([opacityAction(project)]);
      await store.getState().undo();
      const history = store.getState().history;
      const snapshot = {
        ...restored, contentRevision: 5 + revisionIncrease,
        name: revisionIncrease ? "new canonical content" : restored.name,
      };

      await store.getState().installReconciledProject(snapshot);

      expect(store.getState().project.name).toBe(snapshot.name);
      expect(store.getState().project.contentRevision).toBe(snapshot.contentRevision);
      expect(store.getState().canRedo()).toBe(revisionIncrease === 0);
      if (revisionIncrease === 0) expect(store.getState().history).toBe(history);
      else expect(store.getState().project).toBe(snapshot);
    });
  });

  it("serializes concurrent writes so each uses the latest project", async () => {
    const project = splitProject();
    const firstSaved = { ...project, contentRevision: 4, name: "first" };
    let resolveFirst: (value: unknown) => void = () => undefined;
    backendRequest
      .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; }))
      .mockResolvedValueOnce({ project: { ...project, contentRevision: 5, name: "second" } });
    const store = createEditorStore({ projectDir: "/p", project });

    const first = store.getState().applyActions([opacityAction(project)]);
    const second = store.getState().applyActions([opacityAction(project)]);
    await flushAsyncWork();
    // The first write is in flight; the second must wait for it rather than start.
    expect(backendRequest).toHaveBeenCalledTimes(1);
    resolveFirst({ project: firstSaved });
    await first;
    await second;

    expect(backendRequest).toHaveBeenCalledTimes(2);
    expect(store.getState().project.name).toBe("second");
    expect(store.getState().history.past).toHaveLength(2);
    // The second write's undo snapshot is the first write's result, not the stale initial project.
    expect(store.getState().history.past[1]).toEqual(firstSaved);
  });

  describe("mergeExternalState", () => {
    it("commits merged worker state without an undo step, after the pending write settles", async () => {
      const project = splitProject();
      const edited = { ...project, contentRevision: 4, name: "edited" };
      let resolveWrite: (value: unknown) => void = () => undefined;
      backendRequest.mockImplementationOnce(() => new Promise((resolve) => { resolveWrite = resolve; }));
      const store = createEditorStore({ projectDir: "/p", project });

      const write = store.getState().applyActions([opacityAction(project)]);
      const merged = { ...project, jobs: [] };
      let settled = false;
      const merge = store.getState().mergeExternalState(merged, { externalChange: false }).then((result) => {
        settled = true;
        return result;
      });
      await flushAsyncWork();
      expect(settled).toBe(false);

      resolveWrite({ project: edited });
      await write;
      await expect(merge).resolves.toBe(true);
      expect(store.getState().project).toBe(merged);
      expect(store.getState().history.past).toHaveLength(1);
    });

    it("skips a merge computed from a project a write has since replaced", async () => {
      const project = splitProject();
      const edited = { ...project, contentRevision: 4, name: "edited" };
      let resolveWrite: (value: unknown) => void = () => undefined;
      backendRequest.mockImplementationOnce(() => new Promise((resolve) => { resolveWrite = resolve; }));
      const store = createEditorStore({ projectDir: "/p", project });

      const write = store.getState().applyActions([opacityAction(project)]);
      const merge = store.getState().mergeExternalState({ ...project, jobs: [] }, { externalChange: false, base: project });
      await flushAsyncWork();
      resolveWrite({ project: edited });
      await write;

      await expect(merge).resolves.toBe(false);
      expect(store.getState().project).toEqual(edited);
    });

    it("drops redo history and stale selection on an external change", async () => {
      const project = splitProject();
      backendRequest.mockRejectedValue(new BackendUnavailableError());
      const store = createEditorStore({ projectDir: "/p", project });
      await store.getState().applyActions([opacityAction(project, 0.4)]);
      await store.getState().applyActions([opacityAction(project, 0.3)]);
      await store.getState().undo();
      expect(store.getState().canRedo()).toBe(true);
      const removedId = fixtureItem(project, "video").id;
      store.getState().selectItems([removedId]);

      const external: VideoProject = {
        ...project,
        contentRevision: 9,
        timeline: { ...project.timeline, tracks: project.timeline.tracks.map((track) => ({ ...track, items: track.items.filter((item) => item.id !== removedId) })) },
      };
      await expect(store.getState().mergeExternalState(external, { externalChange: true })).resolves.toBe(true);

      expect(store.getState().project).toBe(external);
      expect(store.getState().canRedo()).toBe(false);
      // Undo history stays: only redo would replay edits over the external change.
      expect(store.getState().history.past).toHaveLength(1);
      expect(store.getState().selectedItemIds).toEqual([]);
    });
  });
});
