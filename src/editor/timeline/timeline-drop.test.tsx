import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { fakeDataTransfer, installDragEventPolyfill } from "@/test-utils/data-transfer";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { assetDragMimeType } from "./drag-data";
import { TimelinePanel } from "./timeline-panel";
import { useTimelineCommands } from "./timeline-commands";

const { appliedBatches } = vi.hoisted(() => ({ appliedBatches: [] as ProjectAction[][] }));

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));
vi.mock("@/lib/agent/project-merge", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/agent/project-merge")>();
  return {
    ...actual,
    applyProjectActionsLocally: (project: VideoProject, actions: ProjectAction[]) => {
      appliedBatches.push(actions);
      return actual.applyProjectActionsLocally(project, actions);
    },
  };
});

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

/** Rows after the 118 px header at 80 px/s: Video 1 0–58 px holds "a" 0–2 s; Audio 1 58–92 px. */
function projectWith(videoLocked = false): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [track("v1", "video", [clip("a", 0, 2)], videoLocked), track("a1", "audio", [clip("m", 0, 3, { kind: "audio_clip" })])],
  };
  return project;
}

function rows(): HTMLElement {
  const lane = screen.getByRole("listbox", { name: "Video 1" });
  const element = lane.parentElement?.parentElement?.parentElement;
  if (!element) throw new Error("no rows container");
  return element;
}

function assetTransfer(payload: object) {
  return fakeDataTransfer({ [assetDragMimeType]: JSON.stringify(payload) });
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

function items(project: VideoProject) {
  return project.timeline.tracks.map((entry) => [entry.id, entry.items.map((item) => item.id)]);
}

describe("timeline asset drops", () => {
  beforeAll(() => installDragEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    appliedBatches.length = 0;
  });

  it("shows the target while dragging and adds media on a free track", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    store.getState().setSnapEnabled(false);
    const dataTransfer = assetTransfer({ kind: "media", id: "media-1" });
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 118 + 480, clientY: 29 });
    expect(screen.getByTestId("timeline-drop-marker")).toHaveStyle({ left: "480px", top: "0px", height: "58px" });
    expect(screen.getByRole("listbox", { name: "Video 1" }).parentElement).toHaveAttribute("data-drop-target", "accepted");
    expect(dataTransfer.dropEffect).toBe("copy");

    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 480, clientY: 29 });
    await settle();
    expect(screen.queryByTestId("timeline-drop-marker")).not.toBeInTheDocument();
    expect(appliedBatches).toEqual([[expect.objectContaining({ type: "addItems", targetTrackId: "v1" })]]);
    expect(items(store.getState().project)).toEqual([["v1", ["a", "timeline-media-1"]], ["a1", ["m"]]]);
    expect(store.getState().selectedItemIds).toEqual(["timeline-media-1"]);
  });

  it("creates a track and the item in one undo step when the drop collides", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    store.getState().setSnapEnabled(false);
    const dataTransfer = assetTransfer({ kind: "media", id: "media-1" });
    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 40, clientY: 29 });
    await settle();
    expect(appliedBatches.map((batch) => batch.map((action) => action.type))).toEqual([["createTrack", "reorderTrack", "addItems"]]);
    expect(store.getState().project.timeline.tracks.map((entry) => entry.id)).toEqual(["track-video-3", "v1", "a1"]);
    expect(store.getState().history.past).toHaveLength(1);

    await act(() => store.getState().undo());
    expect(items(store.getState().project)).toEqual([["v1", ["a"]], ["a1", ["m"]]]);
  });

  it("draws the new-track line between rows and honours an explicit start time", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const dataTransfer = fakeDataTransfer({
      [assetDragMimeType]: JSON.stringify({ kind: "template", id: "kinetic-lower-third-v1" }),
      "application/x-video-creater-start-seconds": "5",
    });
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 118 + 40, clientY: 1 });
    expect(screen.getByTestId("timeline-drop-line")).toHaveStyle({ top: "0px" });
    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 40, clientY: 1 });
    await settle();
    const text = store.getState().project.timeline.tracks.find((entry) => entry.kind === "overlay");
    expect(text?.items).toEqual([expect.objectContaining({ kind: "overlay", startSeconds: 5 })]);
  });

  it("refuses a drop onto a locked track with the reason and commits nothing", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith(true) });
    store.getState().setSnapEnabled(false);
    const dataTransfer = assetTransfer({ kind: "media", id: "media-1" });
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 118 + 480, clientY: 29 });
    expect(screen.getByTestId("timeline-drop-marker")).toHaveAttribute("data-rejected", "true");
    expect(dataTransfer.dropEffect).toBe("none");
    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 480, clientY: 29 });
    await settle();
    expect(appliedBatches).toEqual([]);
    expect(store.getState().history.past).toHaveLength(0);
    expect(store.getState().lastError).toBe("Track is locked");
    expect(screen.getByRole("status")).toHaveTextContent("Track is locked");
  });

  it("ignores drags without an asset payload and clears the indicator on leave", () => {
    renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const plain = fakeDataTransfer({ "text/plain": "hello" });
    expect(fireEvent.dragOver(rows(), { dataTransfer: plain, clientX: 200, clientY: 29 })).toBe(true);
    expect(screen.queryByTestId("timeline-drop-marker")).not.toBeInTheDocument();

    fireEvent.dragOver(rows(), { dataTransfer: assetTransfer({ kind: "media", id: "media-1" }), clientX: 200, clientY: 29 });
    expect(screen.getByTestId("timeline-drop-marker")).toBeInTheDocument();
    fireEvent.dragLeave(rows(), { relatedTarget: document.body });
    expect(screen.queryByTestId("timeline-drop-marker")).not.toBeInTheDocument();
  });
});

describe("insertAssetAtPlayhead", () => {
  beforeEach(() => {
    window.localStorage.clear();
    appliedBatches.length = 0;
  });

  it("places the asset at the playhead on the first track that accepts it", async () => {
    let commands: ReturnType<typeof useTimelineCommands> | null = null;
    function Probe() {
      commands = useTimelineCommands();
      return null;
    }
    const { store } = renderWithEditorStore(<Probe />, { project: projectWith() });
    act(() => store.getState().seek(4));
    await act(async () => {
      await commands?.insertAssetAtPlayhead({ kind: "media", id: "media-voiceover" });
    });
    expect(appliedBatches).toEqual([[expect.objectContaining({ type: "addItems", targetTrackId: "a1" })]]);
    expect(store.getState().project.timeline.tracks[1]?.items.at(-1)).toMatchObject({ kind: "audio_clip", startSeconds: 4 });
  });
});
