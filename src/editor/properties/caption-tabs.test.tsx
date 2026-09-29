import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { buildCaptionItems } from "@/lib/captions/caption-items";
import type { ProjectAction, Transcript } from "@/lib/project";
import { captionGroupItems } from "@/lib/properties/caption-properties";
import type { TimelineItem } from "@/lib/timeline";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";
import { captionRegroup } from "./caption-edits";
import { enterValue, projectWithTracks, renderProperties, track } from "./properties-test-utils";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const transcript: Transcript = {
  id: "transcript-1",
  mediaId: "media-1",
  repairs: [],
  segments: [],
  words: ["The", "first", "recorded", "voice", "in", "history"].map((text, index) => ({ text, startSeconds: index, endSeconds: index + 0.8 })),
};

const cueIds = ["caption-group-a-1", "caption-group-a-2", "caption-group-a-3"];

function cues(): TimelineItem[] {
  return buildCaptionItems({ transcript, range: { startSeconds: 0, endSeconds: 6 }, wordsPerCue: 2, stylePreset: "boldReadableLower", groupId: "group-a" });
}

function captionProject() {
  return projectWithTracks([track("captions", "caption", cues())], { transcripts: [transcript] });
}

function styleUpdate(itemIds: readonly string[], set: Record<string, unknown>): ProjectAction[] {
  return [{ type: "updateItemProperties", updates: itemIds.map((itemId) => ({ itemId, set, remove: [] })) }];
}

describe("caption property tabs", () => {
  beforeAll(() => {
    installPointerEventPolyfill();
    Element.prototype.hasPointerCapture = () => true;
    Element.prototype.setPointerCapture = () => undefined;
    Element.prototype.releasePointerCapture = () => undefined;
    Element.prototype.scrollIntoView = () => undefined;
  });

  beforeEach(() => window.localStorage.clear());

  it("commits this cue's text untrimmed through editCaptionText", async () => {
    const { applyActions } = await renderProperties(captionProject(), ["caption-group-a-2"]);
    const field = screen.getByRole("textbox", { name: "Caption text" });
    expect(field).toHaveValue("recorded voice");
    await act(async () => {
      fireEvent.change(field, { target: { value: "recorded voice " } });
      fireEvent.blur(field);
    });
    expect(applyActions).toHaveBeenCalledWith([{ type: "editCaptionText", itemId: "caption-group-a-2", text: "recorded voice " }]);
  });

  it("edits a multi-word sample caption's text through editCaptionText and applies it", async () => {
    const { store } = await renderProperties(fixtureProject(), ["caption-1"], undefined, { spyApply: false });
    const applySpy = vi.fn(store.getState().applyActions);
    const setLastError = vi.fn();
    act(() => store.setState({ applyActions: applySpy, setLastError }));
    const field = screen.getByRole("textbox", { name: "Caption text" });
    expect(field).toHaveValue("Original caption text");
    await act(async () => {
      fireEvent.change(field, { target: { value: "Restored voice" } });
      fireEvent.blur(field);
    });
    expect(applySpy).toHaveBeenCalledWith([{ type: "editCaptionText", itemId: "caption-1", text: "Restored voice" }]);
    const state = store.getState();
    expect(state.history.past).toHaveLength(1);
    expect(fixtureItem(state.project, "caption").source).toEqual({ type: "text", text: "Restored voice" });
    expect(setLastError).not.toHaveBeenCalledWith(expect.stringMatching(/could not be applied/i));
  });

  it("applies style to every cue in the group by default and to one cue with Only this", async () => {
    const { applyActions } = await renderProperties(captionProject(), ["caption-group-a-2"], "Style");
    const scope = screen.getByRole("radiogroup", { name: "Applies to" });
    expect(within(scope).getByRole("radio", { name: "All captions" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText("3 cues receive these changes.")).toBeInTheDocument();

    await enterValue("Size", "56");
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions).toHaveBeenLastCalledWith(styleUpdate(cueIds, { fontSize: 56 }));

    fireEvent.click(within(scope).getByRole("radio", { name: "Only this" }));
    await enterValue("Size", "40");
    expect(applyActions).toHaveBeenCalledTimes(2);
    expect(applyActions).toHaveBeenLastCalledWith(styleUpdate(["caption-group-a-2"], { fontSize: 40 }));

    fireEvent.click(within(screen.getByRole("group", { name: "Highlight" })).getByRole("button", { name: "Pink" }));
    expect(applyActions).toHaveBeenLastCalledWith(styleUpdate(["caption-group-a-2"], { highlightColor: "#ff5c7a" }));
  });

  it("applies a style preset's full property set for the scope", async () => {
    const project = captionProject();
    const { applyActions } = await renderProperties(project, ["caption-group-a-1"], "Style");
    fireEvent.click(within(screen.getByRole("group", { name: "Caption style preset" })).getByRole("button", { name: "Kinetic focus" }));
    const [call] = applyActions.mock.calls;
    const updates = (call?.[0][0] as Extract<ProjectAction, { type: "updateItemProperties" }>).updates;
    expect(updates.map((update) => update.itemId)).toEqual(cueIds);
    expect(updates[0]?.set).toMatchObject({ stylePreset: "kineticFocus", motionPresetId: "pulse-emphasis-v2" });
  });

  it("keeps the Applies to choice across tabs and sets placement", async () => {
    const { applyActions } = await renderProperties(captionProject(), ["caption-group-a-3"], "Style");
    fireEvent.click(screen.getByRole("radio", { name: "Only this" }));
    fireEvent.mouseDown(screen.getByRole("tab", { name: "Position" }), { button: 0 });
    expect(screen.getByRole("radio", { name: "Only this" })).toHaveAttribute("aria-checked", "true");
    fireEvent.click(within(screen.getByRole("radiogroup", { name: "Placement" })).getByRole("radio", { name: "Upper" }));
    expect(applyActions).toHaveBeenCalledWith(styleUpdate(["caption-group-a-3"], { captionPlacement: "upper" }));
  });

  it("animates words for the group and toggles emphasis on this cue", async () => {
    const { applyActions } = await renderProperties(captionProject(), ["caption-group-a-1"], "Animation");
    fireEvent.click(within(screen.getByRole("group", { name: "Caption motion" })).getByRole("button", { name: "Soft depth" }));
    expect(applyActions).toHaveBeenLastCalledWith(styleUpdate(cueIds, { motionPresetId: "soft-depth-card-v2" }));

    fireEvent.click(within(screen.getByRole("group", { name: "Word animation preset" })).getByRole("button", { name: "Karaoke fade" }));
    const presetAction = applyActions.mock.lastCall?.[0][0] as Extract<ProjectAction, { type: "updateItemProperties" }>;
    expect(presetAction.updates.map((update) => update.itemId)).toEqual(cueIds);
    expect(presetAction.updates[0]?.set).toMatchObject({ captionWordAnimationPreset: "karaokeFade", captionWordStaggerSeconds: 0.06 });

    fireEvent.click(within(screen.getByRole("group", { name: "Emphasized words" })).getByRole("button", { name: "first" }));
    expect(applyActions).toHaveBeenLastCalledWith(styleUpdate(["caption-group-a-1"], { emphasizedWordIndices: [1] }));
    expect(applyActions).toHaveBeenCalledTimes(3);
  });

  it("blocks the max words regroup until confirmed, then regroups in one batch", async () => {
    const project = captionProject();
    const expected = captionRegroup(project, "caption-group-a-2", 3);
    if ("blocked" in expected) throw new Error(expected.blocked);
    const { store } = await renderProperties(project, ["caption-group-a-2"], "Style", { spyApply: false });
    const applySpy = vi.fn(store.getState().applyActions);
    act(() => store.setState({ applyActions: applySpy }));
    expect(screen.getByRole("textbox", { name: "Max words" })).toHaveValue("2");

    await enterValue("Max words", "3");
    const dialog = screen.getByRole("dialog", { name: "Regroup 3 cues into 2?" });
    expect(applySpy).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(applySpy).not.toHaveBeenCalled();
    expect(screen.getByRole("textbox", { name: "Max words" })).toHaveValue("2");

    await enterValue("Max words", "3");
    await act(async () => {
      fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Regroup" }));
    });
    expect(applySpy).toHaveBeenCalledTimes(1);
    expect(applySpy).toHaveBeenCalledWith(expected.actions);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    const state = store.getState();
    expect(state.history.past).toHaveLength(1);
    expect(captionGroupItems(state.project, "caption-group-a-1").map((item) => item.id)).toEqual(["caption-group-a-1", "caption-group-a-2"]);
    expect(state.selectedItemIds).toEqual([expected.selectItemId]);
  });

  it("shows why a cue without a transcript can't be regrouped", async () => {
    await renderProperties(projectWithTracks([track("captions", "caption", cues())]), ["caption-group-a-1"], "Style");
    expect(screen.getByRole("textbox", { name: "Max words" })).toBeDisabled();
    expect(screen.getByText("This caption group has no transcript to regroup from.")).toBeInTheDocument();
  });
});
