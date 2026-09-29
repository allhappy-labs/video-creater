import { beforeEach, describe, expect, it, vi } from "vitest";
import { copyItems } from "@/lib/timeline-ops/clipboard";
import { fixtureItem, fixtureProject, fixtureTrack } from "@/test-utils/editor-fixtures";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: vi.fn(),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => path,
}));

const { createEditorStore } = await import("./editor-store");

describe("editor slices", () => {
  beforeEach(() => window.localStorage.clear());

  it("selects, toggles, and clears timeline items", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    const id = fixtureItem(project, "video").id;
    store.getState().selectItems([id]);
    expect(store.getState().selectedItemIds).toEqual([id]);
    expect(store.getState().hasSelection()).toBe(true);
    store.getState().toggleItemSelection(id);
    expect(store.getState().selectedItemIds).toEqual([]);
    store.getState().selectItems([id]);
    store.getState().clearSelection();
    expect(store.getState().hasSelection()).toBe(false);
  });

  it("drops selected ids that no longer exist after a project replacement", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().selectItems(["missing", fixtureItem(project, "video").id]);
    store.getState().replaceProject(project);
    expect(store.getState().selectedItemIds).toEqual([fixtureItem(project, "video").id]);
  });

  it("prunes selection when the project changes without looping", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    const id = fixtureItem(project, "video").id;
    store.getState().selectItems([id, "missing"]);
    const listener = vi.fn();
    store.subscribe(listener);
    store.setState({ project: fixtureProject() });
    expect(store.getState().selectedItemIds).toEqual([id]);
    expect(listener.mock.calls.length).toBeLessThanOrEqual(2);
  });

  it("clamps the playhead to the timeline duration and switches preview source", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().seek(-5);
    expect(store.getState().playheadSeconds).toBe(0);
    store.getState().seek(10_000);
    expect(store.getState().playheadSeconds).toBe(project.timeline.durationSeconds);
    store.getState().previewAsset("media-1");
    expect(store.getState().previewSource).toEqual({ kind: "asset", mediaId: "media-1" });
    store.getState().previewTimeline();
    expect(store.getState().previewSource).toEqual({ kind: "timeline" });
  });

  it("persists active tab and left width, and clamps width", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().setActiveTab("effects");
    store.getState().setLeftWidth(100);
    expect(store.getState().activeTab).toBe("effects");
    expect(store.getState().leftWidth).toBe(300);
    const reopened = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    expect(reopened.getState().activeTab).toBe("effects");
  });

  it("opens one mobile sheet at a time", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().openSheet("media");
    store.getState().openSheet("ai");
    expect(store.getState().openSheetId).toBe("ai");
    store.getState().closeSheet();
    expect(store.getState().openSheetId).toBeNull();
  });

  it("clamps and persists the timeline split height per project", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().setSplitHeight(10);
    expect(store.getState().splitHeight).toBe(160);
    store.getState().setZoomPercent(5000);
    expect(store.getState().zoomPercent).toBe(1000);
    const reopened = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    expect(reopened.getState().splitHeight).toBe(160);
    expect(createEditorStore({ projectDir: "/other", project: fixtureProject() }).getState().splitHeight).toBe(300);
  });
  it("stores the clipboard, selected track, marquee, and range marks, and clears them together", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    const item = fixtureItem(project, "video");
    const clipboard = copyItems(project.timeline, [item.id]);
    store.getState().setClipboard(clipboard);
    store.getState().selectTrack(fixtureTrack(project, "video").id);
    store.getState().setMarquee({ startX: 0, startY: 0, endX: 10, endY: 10 });
    store.getState().setRangeIn(1);
    store.getState().setRangeOut(2);
    store.getState().selectItems([item.id]);
    expect(store.getState()).toMatchObject({
      clipboard,
      selectedTrackId: fixtureTrack(project, "video").id,
      marquee: { endX: 10 },
      rangeIn: 1,
      rangeOut: 2,
    });

    store.getState().clearRange();
    expect(store.getState()).toMatchObject({ rangeIn: null, rangeOut: null });

    store.getState().setRangeIn(1);
    store.getState().clearTimelineContext();
    expect(store.getState()).toMatchObject({
      selectedItemIds: [],
      clipboard: null,
      selectedTrackId: null,
      marquee: null,
      rangeIn: null,
      rangeOut: null,
    });
  });

  it("stores the replace target, revealed media and pending agent request", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    const item = fixtureItem(project, "video");
    expect(store.getState()).toMatchObject({ replaceTargetItemId: null, revealMediaId: null, pendingAgentRequest: null });
    store.getState().setReplaceTargetItemId(item.id);
    store.getState().setRevealMediaId("media-1");
    store.getState().setPendingAgentRequest({ itemIds: [item.id, item.id] });
    expect(store.getState()).toMatchObject({
      replaceTargetItemId: item.id,
      revealMediaId: "media-1",
      pendingAgentRequest: { itemIds: [item.id] },
    });

    store.getState().clearTimelineContext();
    expect(store.getState().replaceTargetItemId).toBeNull();
    store.getState().setReplaceTargetItemId("missing-item");
    store.getState().replaceProject(project);
    expect(store.getState().replaceTargetItemId).toBeNull();
  });

  it("selects a gap until items are selected or the gap closes", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    const gap = { trackId: "track-video", startSeconds: 8, endSeconds: 10 };
    const video = fixtureTrack(project, "video");
    video.items.push({ ...fixtureItem(project, "video"), id: "later", startSeconds: 10, durationSeconds: 2 });
    store.getState().replaceProject(project);
    store.getState().selectGap(gap);
    expect(store.getState().selectedGap).toEqual(gap);
    store.getState().replaceProject(project);
    expect(store.getState().selectedGap).toEqual(gap);
    store.getState().selectItems(["later"]);
    expect(store.getState().selectedGap).toBeNull();

    store.getState().selectGap(gap);
    video.items = video.items.filter((item) => item.id !== "later");
    store.getState().replaceProject(project);
    expect(store.getState().selectedGap).toBeNull();
  });

  it("selects a transition exclusively and drops it when the transition is gone", () => {
    const project = fixtureProject();
    const video = fixtureTrack(project, "video");
    const [left, right] = video.items;
    if (!left || !right) throw new Error("sample video clips");
    video.transitions = [{ id: "fade", leftItemId: left.id, rightItemId: right.id, kind: "crossfade", durationSeconds: 0.5 }];
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().selectItems([left.id]);
    store.getState().selectTransition("fade");
    expect(store.getState()).toMatchObject({ selectedTransitionId: "fade", selectedItemIds: [], selectedGap: null });
    store.getState().selectItems([right.id]);
    expect(store.getState().selectedTransitionId).toBeNull();

    store.getState().selectTransition("fade");
    store.getState().clearSelection();
    expect(store.getState().selectedTransitionId).toBeNull();

    store.getState().selectTransition("fade");
    store.getState().replaceProject({ ...project, timeline: { ...project.timeline, tracks: project.timeline.tracks.map(({ transitions: _gone, ...track }) => track) } });
    expect(store.getState().selectedTransitionId).toBeNull();
  });

  it("drops the selected track when the project no longer has it", () => {
    const project = fixtureProject();
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().selectTrack("missing-track");
    store.getState().replaceProject(project);
    expect(store.getState().selectedTrackId).toBeNull();
  });

  it("remembers and forgets playheads per timeline", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    store.getState().rememberTimelinePlayhead("main", 2.5);
    store.getState().rememberTimelinePlayhead("timeline-2", Number.NaN);
    expect(store.getState().timelinePlayheads).toEqual({ main: 2.5, "timeline-2": 0 });
    store.getState().forgetTimelinePlayhead("main");
    expect(store.getState().timelinePlayheads).toEqual({ "timeline-2": 0 });
  });

  it("persists the timeline tool and lane property but not the scroll position", () => {
    const store = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    expect(store.getState()).toMatchObject({ tool: "select", scrollLeft: 0, laneProperty: "opacity" });
    store.getState().setTool("blade");
    store.getState().setLaneProperty("scale");
    store.getState().setScrollLeft(240);
    store.getState().setScrollLeft(-5);
    expect(store.getState().scrollLeft).toBe(0);
    store.getState().setScrollLeft(240);
    const reopened = createEditorStore({ projectDir: "/p", project: fixtureProject() });
    expect(reopened.getState()).toMatchObject({ tool: "blade", laneProperty: "scale", scrollLeft: 0 });
  });
});
