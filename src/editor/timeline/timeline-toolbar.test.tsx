import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { TimelineToolbar } from "./timeline-toolbar";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function clip(id: string, startSeconds: number, durationSeconds: number, patch: Partial<TimelineItem> = {}): TimelineItem {
  return { id, kind: "video_clip", startSeconds, durationSeconds, source: { type: "media", mediaId: "media-1" }, label: id, properties: {}, ...patch };
}

function track(id: string, kind: TimelineTrack["kind"], items: TimelineItem[]): TimelineTrack {
  return { id, name: id, kind, locked: false, enabled: true, items };
}

/** 24 fps; "a" 0–2 s and "b" 4–6 s on Video 1, "m" 0–3 s on Audio 1. */
function projectWith(): VideoProject {
  const project = fixtureProject();
  project.timeline = {
    durationSeconds: 30,
    tracks: [track("v1", "video", [clip("a", 0, 2), clip("b", 4, 2)]), track("a1", "audio", [clip("m", 0, 3, { kind: "audio_clip" })])],
  };
  return project;
}

function renderToolbar() {
  return renderWithEditorStore(<TimelineToolbar viewportWidth={880} />, { project: projectWith() });
}

function toolbar() {
  return within(screen.getByRole("toolbar", { name: "Timeline tools" }));
}

async function click(name: string) {
  await act(async () => {
    fireEvent.click(toolbar().getByRole("button", { name }));
    await Promise.resolve();
  });
}

function items(project: VideoProject) {
  return project.timeline.tracks.flatMap((entry) => entry.items);
}

describe("TimelineToolbar", () => {
  beforeEach(() => window.localStorage.clear());

  it("switches tools and toggles snap and keyframes", async () => {
    const { store } = renderToolbar();
    expect(toolbar().getByRole("button", { name: "Select" })).toHaveAttribute("aria-pressed", "true");
    await click("Blade");
    expect(store.getState().tool).toBe("blade");
    expect(toolbar().getByRole("button", { name: "Blade" })).toHaveAttribute("aria-pressed", "true");
    await click("Select");
    expect(store.getState().tool).toBe("select");

    await click("Snap");
    expect(store.getState().snapEnabled).toBe(false);
    await click("Keyframes");
    expect(store.getState().keyframesVisible).toBe(true);
    expect(toolbar().getByRole("button", { name: "Keyframes" })).toHaveAttribute("aria-pressed", "true");
  });

  it("splits, nudges, links, unlinks, deletes and ripple deletes the selection", async () => {
    const { store } = renderToolbar();
    act(() => {
      store.getState().selectItems(["a"]);
      store.getState().seek(1);
    });
    await click("Split");
    expect(items(store.getState().project).map((item) => item.id)).toContain("a-split-1000");
    expect(store.getState().selectedItemIds).toEqual(["a-split-1000"]);

    act(() => store.getState().selectItems(["b"]));
    await click("Nudge right");
    expect(items(store.getState().project).find((item) => item.id === "b")?.startSeconds).toBeCloseTo(4 + 1 / 24, 3);
    await click("Nudge left");
    expect(items(store.getState().project).find((item) => item.id === "b")?.startSeconds).toBeCloseTo(4, 3);

    act(() => store.getState().selectItems(["b", "m"]));
    await click("Link");
    expect(items(store.getState().project).find((item) => item.id === "m")?.properties.linkGroupId).toEqual(expect.any(String));
    await click("Unlink");
    expect(items(store.getState().project).find((item) => item.id === "m")?.properties.linkGroupId).toBeUndefined();

    act(() => store.getState().selectItems(["m"]));
    await click("Delete");
    expect(items(store.getState().project).map((item) => item.id)).not.toContain("m");

    act(() => store.getState().selectItems(["a"]));
    await click("Ripple delete");
    expect(items(store.getState().project).find((item) => item.id === "a-split-1000")?.startSeconds).toBe(0);
    expect(store.getState().history.past).toHaveLength(7);
  });

  it("zooms with the buttons, the slider and Fit", async () => {
    const { store } = renderToolbar();
    await click("Zoom in");
    expect(store.getState().zoomPercent).toBe(125);
    await click("Zoom out");
    expect(store.getState().zoomPercent).toBe(100);

    const slider = toolbar().getByRole("slider", { name: "Zoom" });
    expect(slider).toHaveAttribute("aria-valuetext", "100%");
    fireEvent.keyDown(slider, { key: "End" });
    expect(store.getState().zoomPercent).toBe(1000);
    expect(toolbar().getByRole("button", { name: "Zoom in" })).toHaveAttribute("aria-disabled", "true");

    act(() => store.getState().setScrollLeft(400));
    await click("Fit");
    // (880 - 160 px trailing space) / 30 s = 24 px/s = 30% of the 80 px/s base.
    expect(store.getState().zoomPercent).toBe(30);
    expect(store.getState().scrollLeft).toBe(0);
  });

  it("keeps blocked commands focusable and explains them in the tooltip", async () => {
    const { store } = renderToolbar();
    const split = toolbar().getByRole("button", { name: "Split" });
    expect(split).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(split);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Select a clip to split.");
    await click("Split");
    expect(store.getState().history.past).toHaveLength(0);

    expect(toolbar().getByRole("button", { name: "Link" })).toHaveAttribute("aria-disabled", "true");
    act(() => store.getState().selectItems(["a"]));
    expect(toolbar().getByRole("button", { name: "Nudge left" })).toHaveAttribute("aria-disabled", "true");
    expect(toolbar().getByRole("button", { name: "Delete" })).not.toHaveAttribute("aria-disabled");
  });

  it("shows the label and shortcut in enabled tooltips", async () => {
    const { store } = renderToolbar();
    act(() => store.getState().selectItems(["a"]));
    fireEvent.focus(toolbar().getByRole("button", { name: "Ripple delete" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Ripple delete (⇧Delete)");
  });
});
