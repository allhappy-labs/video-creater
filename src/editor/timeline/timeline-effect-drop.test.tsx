import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { fakeDataTransfer, installDragEventPolyfill } from "@/test-utils/data-transfer";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { effectDragMimeType, readEffectDragData, writeEffectDragData } from "./effect-drag-data";
import { TimelinePanel } from "./timeline-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const grain = { id: "stylize.grain", displayName: "Film Grain", params: [{ key: "amount", defaultValue: 0.2 }], resourceKey: null };

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[], locked = false): TimelineTrack {
  return { id, name: id, kind, locked, enabled: true, items };
}

/** Rows after the 118 px header at 80 px/s: Video 1 0–58 px holds "a" 0–2 s; Audio 1 58–92 px holds "m" 0–3 s. */
function projectWith(videoLocked = false): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [track("v1", "video", [clip("a", 0, 2)], videoLocked), track("a1", "audio", [clip("m", 0, 3, { kind: "audio_clip" })])],
  };
  return project;
}

function rows(): HTMLElement {
  const element = screen.getByRole("listbox", { name: "Video 1" }).parentElement?.parentElement?.parentElement;
  if (!element) throw new Error("no rows container");
  return element;
}

function effectTransfer(effect: object = grain) {
  return fakeDataTransfer({ [effectDragMimeType]: JSON.stringify(effect) });
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
  });
}

function effectsOf(project: VideoProject, itemId: string) {
  return project.timeline.tracks.flatMap((entry) => entry.items).find((item) => item.id === itemId)?.properties.effects;
}

describe("effect drag data", () => {
  it("round-trips the fields applying needs and rejects malformed payloads", () => {
    const dataTransfer = fakeDataTransfer();
    writeEffectDragData(dataTransfer, {
      ...grain,
      params: [{ key: "amount", label: "Amount", min: 0, max: 1, defaultValue: 0.2, unit: "" }],
    } as never);
    expect(readEffectDragData(dataTransfer)).toEqual(grain);
    expect(dataTransfer.effectAllowed).toBe("copy");
    expect(readEffectDragData(effectTransfer({ id: "x", displayName: "X", params: [{ key: "a", defaultValue: "1" }] }))).toBeNull();
    expect(readEffectDragData(fakeDataTransfer({ [effectDragMimeType]: "{" }))).toBeNull();
  });
});

describe("timeline effect drops", () => {
  beforeAll(() => installDragEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
  });

  it("applies a dropped effect to the visual clip under the pointer in one undo step and selects it", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const dataTransfer = effectTransfer();
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 118 + 80, clientY: 29 });
    expect(dataTransfer.dropEffect).toBe("copy");
    expect(screen.queryByTestId("timeline-drop-marker")).not.toBeInTheDocument();

    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 80, clientY: 29 });
    await settle();
    expect(effectsOf(store.getState().project, "a")).toEqual([
      { effectInstanceId: "legacy:stylize.grain:1", effectType: "stylize.grain", enabled: true, params: { amount: 0.2 } },
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().selectedItemIds).toEqual(["a"]);
  });

  it("refuses audio clips, locked tracks and empty lane space", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith() });
    const overAudio = effectTransfer();
    fireEvent.dragOver(rows(), { dataTransfer: overAudio, clientX: 118 + 80, clientY: 70 });
    expect(overAudio.dropEffect).toBe("none");
    fireEvent.drop(rows(), { dataTransfer: overAudio, clientX: 118 + 80, clientY: 70 });
    await settle();
    expect(store.getState().lastError).toBe("Select a video or image clip");

    fireEvent.drop(rows(), { dataTransfer: effectTransfer(), clientX: 118 + 400, clientY: 29 });
    await settle();
    expect(store.getState().lastError).toBe("Drop the effect onto a video or image clip.");
    expect(store.getState().history.past).toHaveLength(0);
  });

  it("explains a drop onto a locked track", async () => {
    const { store } = renderWithEditorStore(<TimelinePanel />, { project: projectWith(true) });
    const dataTransfer = effectTransfer();
    fireEvent.dragOver(rows(), { dataTransfer, clientX: 118 + 80, clientY: 29 });
    expect(dataTransfer.dropEffect).toBe("none");
    fireEvent.drop(rows(), { dataTransfer, clientX: 118 + 80, clientY: 29 });
    await settle();
    expect(store.getState().lastError).toBe("Unlock the track to apply effects");
    expect(effectsOf(store.getState().project, "a")).toBeUndefined();
  });
});
