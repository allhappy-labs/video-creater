import { describe, expect, it, vi } from "vitest";
import { fixtureMedia, fixtureProject } from "@/test-utils/editor-fixtures";
import { createEditorStore } from "./editor-store";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function setup() {
  const project = fixtureProject();
  return { project, store: createEditorStore({ projectDir: "/p", project }) };
}

describe("playback slice", () => {
  it("restarts from 0 when play is toggled at the end of the timeline", () => {
    const { project, store } = setup();
    store.getState().seek(project.timeline.durationSeconds);
    store.getState().togglePlaying();
    expect(store.getState()).toMatchObject({ playing: true, playheadSeconds: 0 });
    store.getState().togglePlaying();
    expect(store.getState()).toMatchObject({ playing: false, playheadSeconds: 0 });
  });

  it("keeps the playhead when play starts mid-timeline and refuses an empty timeline", () => {
    const { store } = setup();
    store.getState().seek(2);
    store.getState().togglePlaying();
    expect(store.getState()).toMatchObject({ playing: true, playheadSeconds: 2 });

    const empty = fixtureProject();
    empty.timeline.durationSeconds = 0;
    const emptyStore = createEditorStore({ projectDir: "/p", project: empty });
    emptyStore.getState().togglePlaying();
    expect(emptyStore.getState().playing).toBe(false);
  });

  it("stops playback when switching between timeline and asset preview", () => {
    const { store } = setup();
    store.getState().togglePlaying();
    store.getState().previewAsset("media-1");
    expect(store.getState().playing).toBe(false);
    store.getState().togglePlaying();
    expect(store.getState().playing).toBe(true);
    store.getState().previewTimeline();
    expect(store.getState()).toMatchObject({ playing: false, previewSource: { kind: "timeline" } });
  });

  it("only toggles asset playback for playable media kinds", () => {
    const project = fixtureProject();
    project.media.push({ ...fixtureMedia(project, "video"), id: "still", kind: "image", relativePath: "media/still.png" });
    const store = createEditorStore({ projectDir: "/p", project });
    store.getState().previewAsset("still");
    store.getState().togglePlaying();
    expect(store.getState().playing).toBe(false);
  });

  it("tracks canonical preparation and bumps the retry token", () => {
    const { project, store } = setup();
    store.getState().setCanonicalPreparation({ sourceProject: project, status: "failed", message: "boom" });
    expect(store.getState().canonicalPreparation).toEqual({ sourceProject: project, status: "failed", message: "boom" });
    const token = store.getState().canonicalRetryToken;
    store.getState().retryCanonicalPreparation();
    expect(store.getState().canonicalRetryToken).toBe(token + 1);
  });

  it("tracks the item whose text is edited inline", () => {
    const { store } = setup();
    expect(store.getState().editingTextItemId).toBeNull();
    store.getState().setEditingTextItemId("caption-1");
    expect(store.getState().editingTextItemId).toBe("caption-1");
    store.getState().setEditingTextItemId(null);
    expect(store.getState().editingTextItemId).toBeNull();
  });
});
