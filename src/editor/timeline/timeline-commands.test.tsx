import { renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { fixtureProject } from "@/test-utils/editor-fixtures";

const backendRequest = vi.fn();
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (...args: unknown[]) => backendRequest(...args),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("../store/editor-store");
const { EditorStoreProvider } = await import("../store/editor-store-context");
const { useTimelineCommands } = await import("./timeline-commands");

function clip(id: string, startSeconds: number, durationSeconds: number): TimelineItem {
  return {
    id,
    kind: "video_clip",
    startSeconds,
    durationSeconds,
    source: { type: "media", mediaId: "media-1" },
    label: id,
    properties: {},
  };
}

function videoTrack(id: string, items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind: "video", locked: false, enabled: true, items };
}

function projectWith(tracks: TimelineTrack[]): VideoProject {
  return {
    ...fixtureProject(),
    schemaVersion: 2,
    contentRevision: 1,
    timeline: { durationSeconds: 8, tracks },
  };
}

function setup(project: VideoProject) {
  const store = createEditorStore({ projectDir: "/p", project });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <EditorStoreProvider store={store}>{children}</EditorStoreProvider>
  );
  const { result, rerender } = renderHook(() => useTimelineCommands(), { wrapper });
  return { store, commands: result.current, result, rerender };
}

function itemIds(project: VideoProject): string[] {
  return project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id));
}

describe("useTimelineCommands", () => {
  beforeEach(() => {
    backendRequest.mockReset();
    backendRequest.mockRejectedValue(new BackendUnavailableError());
    window.localStorage.clear();
  });

  it("returns the same command functions across renders", () => {
    const { result, rerender, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 4)])]));
    rerender();
    expect(result.current).toBe(commands);
  });

  it("splits the selection at the playhead, selects the right half, and undoes to the original", async () => {
    const project = projectWith([videoTrack("v1", [clip("a", 0, 4)])]);
    const { store, commands } = setup(project);
    store.getState().selectItems(["a"]);
    store.getState().seek(2);

    await expect(commands.splitAtPlayhead()).resolves.toBe(true);

    expect(itemIds(store.getState().project)).toEqual(["a", "a-split-2000"]);
    expect(store.getState().selectedItemIds).toEqual(["a-split-2000"]);
    expect(store.getState().history.past).toHaveLength(1);

    await store.getState().undo();
    expect(store.getState().project).toEqual(project);
  });

  it("splits one clip at a given time and selects the right half", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 4)])]));
    await expect(commands.splitItemAt("a", 1.5)).resolves.toBe(true);
    expect(itemIds(store.getState().project)).toEqual(["a", "a-split-1500"]);
    expect(store.getState().selectedItemIds).toEqual(["a-split-1500"]);
  });

  it("applies a planned interaction as one undo step, selecting added items on request", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 2)])]));
    store.getState().selectItems(["a"]);
    await commands.applyPlan({ actions: [{ type: "moveItems", moves: [{ itemId: "a", targetTrackId: "v1", startSeconds: 1 }] }] });
    expect(store.getState().project.timeline.tracks[0]?.items[0]?.startSeconds).toBe(1);
    expect(store.getState().selectedItemIds).toEqual(["a"]);
    expect(store.getState().history.past).toHaveLength(1);

    await commands.applyPlan({ actions: [{ type: "addItems", targetTrackId: "v1", items: [clip("b", 4, 1)] }] }, "added");
    expect(store.getState().selectedItemIds).toEqual(["b"]);

    await expect(commands.applyPlan({ blocked: "Overlaps b" })).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Overlaps b");
    expect(store.getState().history.past).toHaveLength(2);
  });

  it("snaps the split point to the nearest frame", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 4)])]));
    store.getState().selectItems(["a"]);
    // 24 fps: 1.01 s rounds to frame 24 (1 s).
    store.getState().seek(1.01);
    await commands.splitAtPlayhead();
    expect(itemIds(store.getState().project)).toEqual(["a", "a-split-1000"]);
  });

  it("deletes the selection with the emptied track and restores both with one undo", async () => {
    const project = projectWith([videoTrack("v1", [clip("a", 0, 2)]), videoTrack("v2", [clip("b", 0, 2)])]);
    const { store, commands } = setup(project);
    store.getState().selectItems(["b"]);

    await expect(commands.deleteSelection()).resolves.toBe(true);

    expect(store.getState().project.timeline.tracks.map((track) => track.id)).toEqual(["v1"]);
    expect(store.getState().selectedItemIds).toEqual([]);
    expect(store.getState().history.past).toHaveLength(1);

    await store.getState().undo();
    expect(store.getState().project).toEqual(project);
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("detaches a video clip's audio as one undo step and selects the audio clip", async () => {
    const audioTrack: TimelineTrack = { id: "a1", name: "a1", kind: "audio", locked: false, enabled: true, items: [] };
    const project = projectWith([videoTrack("v1", [clip("a", 0, 2)]), audioTrack]);
    const { store, commands } = setup(project);

    await expect(commands.detachAudio("a")).resolves.toBe(true);

    const next = store.getState().project;
    const audio = next.timeline.tracks.find((track) => track.id === "a1")?.items[0];
    expect(audio).toMatchObject({ id: "a-audio", kind: "audio_clip", startSeconds: 0, durationSeconds: 2 });
    expect(audio?.properties.linkGroupId).toMatch(/^link-/);
    expect(store.getState().selectedItemIds).toEqual(["a-audio"]);
    expect(store.getState().history.past).toHaveLength(1);

    await store.getState().undo();
    expect(store.getState().project).toEqual(project);
  });

  it("reports a blocked command in lastError without touching the project or history", async () => {
    const project = projectWith([videoTrack("v1", [clip("a", 0, 4)])]);
    const { store, commands } = setup(project);

    await expect(commands.splitAtPlayhead()).resolves.toBe(false);

    expect(store.getState().lastError).toBe("Select a clip to split.");
    expect(store.getState().project).toBe(project);
    expect(store.getState().history.past).toHaveLength(0);
    expect(backendRequest).not.toHaveBeenCalled();
  });

  it("cuts and pastes as two undo steps that restore the items", async () => {
    const project = projectWith([videoTrack("v1", [clip("a", 0, 2), clip("b", 3, 2)])]);
    const { store, commands } = setup(project);
    store.getState().selectItems(["a"]);

    await expect(commands.cutSelection()).resolves.toBe(true);
    expect(itemIds(store.getState().project)).toEqual(["b"]);
    expect(store.getState().clipboard?.entries.map((entry) => entry.item.id)).toEqual(["a"]);
    expect(store.getState().selectedItemIds).toEqual([]);

    store.getState().seek(0);
    await expect(commands.paste()).resolves.toBe(true);

    const pasted = store.getState().project.timeline.tracks[0]?.items.find((item) => item.id === "a-copy");
    expect(pasted).toMatchObject({ startSeconds: 0, durationSeconds: 2, source: clip("a", 0, 2).source });
    expect(store.getState().selectedItemIds).toEqual(["a-copy"]);
    expect(store.getState().history.past).toHaveLength(2);

    await store.getState().undo();
    await store.getState().undo();
    expect(store.getState().project).toEqual(project);
  });

  it("pastes with ripple insert and selects the inserted clips", async () => {
    const project = projectWith([videoTrack("v1", [clip("a", 0, 2), clip("b", 2, 2)])]);
    const { store, commands } = setup(project);
    store.getState().selectItems(["a"]);
    await commands.copySelection();
    store.getState().seek(2);

    await expect(commands.pasteInsert()).resolves.toBe(true);

    const items = store.getState().project.timeline.tracks[0]?.items ?? [];
    expect(items.find((item) => item.id === "b")?.startSeconds).toBe(4);
    expect(store.getState().selectedItemIds).toEqual(["a-copy"]);
  });

  it("duplicates the selection and selects the duplicates", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 2)])]));
    store.getState().selectItems(["a"]);
    await expect(commands.duplicateSelection()).resolves.toBe(true);
    expect(store.getState().selectedItemIds).toEqual(["a-copy"]);
  });

  it("returns an asset preview to the timeline at the playhead when an insert places media", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 2)])]));
    store.getState().seek(1);
    store.getState().previewAsset("media-1");
    await expect(commands.insertAssetAtPlayhead({ kind: "media", id: "media-1" })).resolves.toBe(true);
    expect(store.getState()).toMatchObject({ previewSource: { kind: "timeline" }, playheadSeconds: 1 });

    store.getState().previewAsset("media-1");
    await expect(commands.insertAsset({ kind: "media", id: "media-1" }, { hoveredTrackId: null, insertIndex: 0, startSeconds: 6 })).resolves.toBe(true);
    expect(store.getState()).toMatchObject({ previewSource: { kind: "timeline" }, playheadSeconds: 1 });
  });

  it("keeps the asset preview when an insert is blocked", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 2)])]));
    store.getState().previewAsset("media-1");
    await expect(commands.insertAssetAtPlayhead({ kind: "media", id: "missing-media" })).resolves.toBe(false);
    expect(store.getState().previewSource).toEqual({ kind: "asset", mediaId: "media-1" });
  });

  it("blocks paste with an empty clipboard", async () => {
    const { store, commands } = setup(projectWith([videoTrack("v1", [clip("a", 0, 2)])]));
    await expect(commands.paste()).resolves.toBe(false);
    expect(store.getState().lastError).toBe("Copy a clip first.");
  });

  describe("timelines", () => {
    function multiTimelineProject(): VideoProject {
      const project = projectWith([videoTrack("v1", [clip("a", 0, 4)])]);
      return {
        ...project,
        activeTimelineId: "main",
        timelines: [
          { id: "main", name: "Main", timeline: project.timeline },
          { id: "timeline-2", name: "Intro", timeline: { durationSeconds: 6, tracks: [videoTrack("n1", [clip("n", 0, 6)])] } },
        ],
      };
    }

    it("switching stops playback, clears selection, range and clipboard, and restores each playhead", async () => {
      const { store, commands } = setup(multiTimelineProject());
      store.getState().selectItems(["a"]);
      await commands.copySelection();
      store.getState().setRangeIn(1);
      store.getState().seek(1.5);
      store.getState().setPlaying(true);

      await expect(commands.switchTimeline("timeline-2")).resolves.toBe(true);

      expect(store.getState()).toMatchObject({
        playing: false,
        selectedItemIds: [],
        clipboard: null,
        rangeIn: null,
        playheadSeconds: 0,
      });
      expect(store.getState().project.activeTimelineId).toBe("timeline-2");

      store.getState().seek(5);
      await commands.switchTimeline("main");
      expect(store.getState().playheadSeconds).toBe(1.5);
      await commands.switchTimeline("timeline-2");
      expect(store.getState().playheadSeconds).toBe(5);
    });

    it("switching to the active timeline changes nothing", async () => {
      const { store, commands } = setup(multiTimelineProject());
      store.getState().selectItems(["a"]);
      store.getState().setPlaying(true);
      await expect(commands.switchTimeline("main")).resolves.toBe(true);
      expect(store.getState()).toMatchObject({ playing: true, selectedItemIds: ["a"] });
      expect(store.getState().history.past).toHaveLength(0);
    });

    it("creates, renames, and deletes timelines", async () => {
      const { store, commands } = setup(multiTimelineProject());
      store.getState().seek(2);

      await expect(commands.createTimeline(true)).resolves.toBe(true);
      expect(store.getState().project.activeTimelineId).toBe("timeline-3");
      expect(store.getState().project.timelines?.at(-1)?.name).toBe("Copy of Main");
      expect(store.getState().playheadSeconds).toBe(0);

      await expect(commands.renameTimeline("timeline-3", "  Outro ")).resolves.toBe(true);
      expect(store.getState().project.timelines?.at(-1)?.name).toBe("Outro");

      await expect(commands.renameTimeline("timeline-3", " ")).resolves.toBe(false);
      expect(store.getState().lastError).toBe("Enter a timeline name.");

      await expect(commands.deleteTimeline("timeline-2")).resolves.toBe(true);
      expect(store.getState().project.timelines?.map((entry) => entry.id)).toEqual(["main", "timeline-3"]);
    });
  });
});
