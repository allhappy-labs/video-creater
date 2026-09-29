import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { visualOpacity } from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import type { EditorStore } from "../store/editor-store";
import { enterValue, renderProperties, testItem, testProject } from "./properties-test-utils";
import { resetEffectCatalogForTests } from "./use-effect-catalog";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

function projectItem(store: EditorStore, itemId: string): TimelineItem {
  const item = store
    .getState()
    .project.timeline.tracks.flatMap((track) => track.items)
    .find((candidate) => candidate.id === itemId);
  if (!item) throw new Error(`missing ${itemId}`);
  return item;
}

describe("multiple selection property tab", () => {
  beforeEach(() => {
    window.localStorage.clear();
    resetEffectCatalogForTests();
  });

  it("shows Mixed opacity and applies one batch that undoes in one step", async () => {
    const project = testProject([
      testItem("a", "video_clip", { opacity: 1 }),
      { ...testItem("b", "image_clip", { opacity: 0.5 }), startSeconds: 4 },
    ]);
    const { store } = await renderProperties(project, ["a", "b"], undefined, { spyApply: false });
    const applySpy = vi.fn(store.getState().applyActions);
    act(() => store.setState({ applyActions: applySpy }));
    expect(screen.getByRole("textbox", { name: "Opacity" })).toHaveValue("Mixed");

    await enterValue("Opacity", "30");
    expect(applySpy).toHaveBeenCalledTimes(1);
    expect(applySpy).toHaveBeenCalledWith([
      { type: "updateVisualClipOpacity", itemId: "a", opacity: 0.3 },
      { type: "updateVisualClipOpacity", itemId: "b", opacity: 0.3 },
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(visualOpacity(projectItem(store, "a"))).toBe(0.3);
    expect(visualOpacity(projectItem(store, "b"))).toBe(0.3);
    expect(screen.getByRole("textbox", { name: "Opacity" })).toHaveValue("30%");

    await act(() => store.getState().undo());
    expect(visualOpacity(projectItem(store, "a"))).toBe(1);
    expect(visualOpacity(projectItem(store, "b"))).toBe(0.5);
    expect(screen.getByRole("textbox", { name: "Opacity" })).toHaveValue("Mixed");
  });

  it("sets volume on every selected audio clip in one batch", async () => {
    const project = testProject([], [
      testItem("x", "audio_clip", { volumeDb: -3 }),
      { ...testItem("y", "audio_clip"), startSeconds: 4 },
    ]);
    const { applyActions } = await renderProperties(project, ["x", "y"]);
    expect(screen.getByRole("textbox", { name: "Volume" })).toHaveValue("Mixed");
    expect(screen.queryByRole("textbox", { name: "Opacity" })).not.toBeInTheDocument();
    await enterValue("Volume", "");
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateAudioVolume", itemId: "x", volumeDb: null },
      { type: "updateAudioVolume", itemId: "y", volumeDb: null },
    ]);
  });

  it("lists only the effects every item shares and removes them in one batch", async () => {
    const blur = { effectInstanceId: "blur-a", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } };
    const vignette = { effectInstanceId: "vig-a", effectType: "stylize.vignette", enabled: true, params: {} };
    const project = testProject([
      testItem("a", "video_clip", { effects: [blur, vignette] }),
      { ...testItem("b", "video_clip", { effects: [{ ...blur, effectInstanceId: "blur-b" }] }), startSeconds: 4 },
    ]);
    const { applyActions } = await renderProperties(project, ["a", "b"]);
    const shared = screen.getByRole("list", { name: "Shared effects" });
    expect(within(shared).getAllByRole("listitem")).toHaveLength(1);
    fireEvent.click(within(shared).getByRole("button", { name: "Remove blur.gaussian" }));
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateItemEffects", itemIds: ["a"], effects: [vignette] },
      { type: "updateItemEffects", itemIds: ["b"], effects: [] },
    ]);
  });

  it("explains a selection without common properties", async () => {
    const project = testProject([testItem("a", "video_clip")], [testItem("x", "audio_clip")]);
    await renderProperties(project, ["a", "x"]);
    expect(screen.getByText("These items share no editable properties.")).toBeInTheDocument();
  });
});
