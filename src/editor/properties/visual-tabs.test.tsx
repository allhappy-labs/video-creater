import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { backendRequest } from "@/lib/runtime/backend-client";
import { installPointerEventPolyfill, stubRect } from "@/test-utils/editor-render";
import { enterValue, projectWithTracks, renderProperties, testItem, testProject, track } from "./properties-test-utils";
import { resetEffectCatalogForTests } from "./use-effect-catalog";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const blurDescriptor = {
  id: "blur.gaussian",
  displayName: "Gaussian Blur",
  category: "Blur & Sharpen",
  params: [{ key: "radius", label: "Radius", min: 0, max: 50, defaultValue: 0, unit: "px" }],
  colorEffect: false,
};

function tabNames(): string[] {
  return within(screen.getByRole("tablist", { name: "Property tabs" }))
    .getAllByRole("tab")
    .map((tab) => tab.textContent ?? "");
}

describe("visual property tabs", () => {
  beforeAll(() => {
    installPointerEventPolyfill();
    Element.prototype.hasPointerCapture = () => true;
    Element.prototype.setPointerCapture = () => undefined;
    Element.prototype.releasePointerCapture = () => undefined;
    Element.prototype.scrollIntoView = () => undefined;
  });

  beforeEach(() => {
    window.localStorage.clear();
    resetEffectCatalogForTests();
    vi.mocked(backendRequest).mockReset();
  });

  it("commits opacity once through the opacity builder", async () => {
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip")]), ["clip"]);
    await enterValue("Opacity", "50");
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateVisualClipOpacity", itemId: "clip", opacity: 0.5 }]);
  });

  it("upserts keyframed opacity at the playhead and toggles the ◇ keyframe", async () => {
    const keyframes = { opacity: [{ atSeconds: 0, value: 1 }, { atSeconds: 2, value: 0.5 }] };
    const project = testProject([testItem("clip", "video_clip", { keyframes })]);
    const { applyActions } = await renderProperties(project, ["clip"], undefined, { playheadSeconds: 1 });
    expect(screen.getByRole("textbox", { name: "Opacity" })).toHaveValue("75%");

    await enterValue("Opacity", "40");
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "upsertItemKeyframe", itemId: "clip", property: "opacity", keyframe: { atSeconds: 1, value: 0.4 } },
    ]);

    fireEvent.click(screen.getByRole("button", { name: "Add keyframe for Opacity" }));
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "upsertItemKeyframe", itemId: "clip", property: "opacity", keyframe: { atSeconds: 1, value: 0.75, easing: "linear" } },
    ]);
  });

  it("removes the keyframe under the playhead with a filled ◇", async () => {
    const keyframes = { scale: [{ atSeconds: 2, value: 1.2 }] };
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip", { keyframes })]), ["clip"], undefined, {
      playheadSeconds: 2,
    });
    const remove = screen.getByRole("button", { name: "Remove keyframe for Scale" });
    expect(remove).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(remove);
    expect(applyActions).toHaveBeenCalledWith([{ type: "deleteItemKeyframe", itemId: "clip", property: "scale", atSeconds: 2 }]);
  });

  it("sets static scale, position and rotation through motionActions", async () => {
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip")]), ["clip"]);
    await enterValue("Scale", "150%");
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "clip", set: { scale: 1.5 }, remove: [] }] },
    ]);
    await enterValue("Rotate", "0.5");
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "clip", set: { rotationDegrees: 0.5 }, remove: [] }] },
    ]);
  });

  it("previews a dragged slider without committing and clears the preview on release", async () => {
    const { store, applyActions } = await renderProperties(testProject([testItem("clip", "video_clip")]), ["clip"]);
    const thumb = screen.getByRole("slider", { name: "Opacity" });
    const root = thumb.closest<HTMLElement>("[data-orientation]:not([role])");
    if (!root) throw new Error("slider root");
    stubRect(root, { width: 100, height: 20 });
    fireEvent.pointerDown(root, { clientX: 40, pointerId: 1, button: 0 });
    expect(store.getState().propertyPreview).toEqual({ itemId: "clip", patch: { opacity: 0.4 } });
    expect(applyActions).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.pointerUp(root, { clientX: 40, pointerId: 1 });
    });
    expect(applyActions).toHaveBeenCalledTimes(1);
    expect(store.getState().propertyPreview).toBeNull();
  });

  it("changes the blend mode with the legacy labels", async () => {
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip", { blendMode: "screen" })]), ["clip"]);
    const trigger = screen.getByRole("combobox", { name: "Blend mode" });
    expect(trigger).toHaveTextContent("Screen");
    fireEvent.keyDown(trigger, { key: "Enter" });
    fireEvent.click(screen.getByRole("option", { name: "Normal" }));
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateItemProperties", updates: [{ itemId: "clip", set: {}, remove: ["blendMode"] }] },
    ]);
  });

  it("opens canvas crop mode and blocks crops that hide the frame", async () => {
    const { store, applyActions, setLastError } = await renderProperties(
      testProject([testItem("clip", "video_clip", { cropLeft: 0.6 })]),
      ["clip"],
    );
    fireEvent.click(screen.getByRole("button", { name: "Edit on canvas" }));
    expect(store.getState().cropModeItemId).toBe("clip");

    await enterValue("Crop top", "10");
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateVisualClipCrop", itemId: "clip", crop: { cropTop: 0.1 } }]);

    await enterValue("Crop right", "50");
    expect(setLastError).toHaveBeenCalledWith("Opposite crop sides must leave part of the frame visible.");
    expect(applyActions).toHaveBeenCalledTimes(1);
  });

  it("commits fades and blocks fades longer than the clip", async () => {
    const { applyActions, setLastError } = await renderProperties(
      testProject([testItem("clip", "video_clip", { fadeOutSeconds: 3 })]),
      ["clip"],
    );
    await enterValue("Fade in", "0.5");
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateVisualClipFades", itemId: "clip", fadeInSeconds: 0.5, fadeOutSeconds: 3 }]);
    await enterValue("Fade in", "2");
    expect(setLastError).toHaveBeenCalledWith("Fade in and fade out together can't be longer than the clip.");
    expect(applyActions).toHaveBeenCalledTimes(1);
  });

  it("applies look presets and color grade sliders", async () => {
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip")]), ["clip"]);
    const looks = screen.getByRole("group", { name: "Look preset" });
    expect(within(looks).getByRole("button", { name: "None" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(looks).getByRole("button", { name: "B&W" }));
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemColorGrade", itemIds: ["clip"], reset: true, grade: { exposure: 0, contrast: 1.15, saturation: 0, temperature: 6500, tint: 0 } },
    ]);

    const disclosure = screen.getByRole("button", { name: "Color" });
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(disclosure);
    await enterValue("Exposure", "0.5");
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateItemColorGrade", itemIds: ["clip"], reset: false, grade: { exposure: 0.5 } }]);
  });

  it("edits and removes applied effects with catalog parameters", async () => {
    vi.mocked(backendRequest).mockResolvedValue({ source: "test", effectCount: 1, canonicalOrder: [], effects: [blurDescriptor] });
    const blur = { effectInstanceId: "blur-1", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } };
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip", { effects: [blur] })]), ["clip"]);
    const effects = await screen.findByRole("list", { name: "Applied effects" });
    await waitFor(() => expect(within(effects).getByRole("textbox", { name: "Radius (px)" })).toHaveValue("4"));

    await enterValue("Radius (px)", "12");
    expect(applyActions).toHaveBeenLastCalledWith([
      { type: "updateItemEffects", itemIds: ["clip"], effects: [{ ...blur, params: { radius: 12 } }] },
    ]);
    fireEvent.click(within(effects).getByRole("button", { name: "Remove Gaussian Blur" }));
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateItemEffects", itemIds: ["clip"], effects: [] }]);
    expect(backendRequest).toHaveBeenCalledTimes(1);
  });

  it("treats a missing backend as an empty effect catalog", async () => {
    vi.mocked(backendRequest).mockRejectedValue(Object.assign(new Error("offline"), { code: "backend_unavailable" }));
    const blur = { effectInstanceId: "blur-1", effectType: "blur.gaussian", enabled: true, params: { radius: 4 } };
    await renderProperties(testProject([testItem("clip", "video_clip", { effects: [blur] })]), ["clip"]);
    expect(await screen.findByText("Effect catalog unavailable. Parameters can't be edited right now.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove blur.gaussian" })).toBeInTheDocument();
  });

  it("changes speed through the slider and presets as one batch", async () => {
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip")]), ["clip"], "Speed");
    fireEvent.click(within(screen.getByRole("group", { name: "Speed presets" })).getByRole("button", { name: "2×" }));
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateVisualClipSpeed", itemId: "clip", speed: 2 },
      { type: "resizeItems", resizes: [{ itemId: "clip", durationSeconds: 2 }] },
    ]);
  });

  it("reverses a clip and its linked audio clip from the Speed tab as one undo step", async () => {
    const project = testProject(
      [testItem("clip", "video_clip", { linkGroupId: "link-1" })],
      [testItem("clip-sound", "audio_clip", { linkGroupId: "link-1" })],
    );
    const { store } = await renderProperties(project, ["clip"], "Speed", { spyApply: false });
    const applySpy = vi.fn(store.getState().applyActions);
    act(() => store.setState({ applyActions: applySpy }));
    const reverse = screen.getByRole("switch", { name: "Reverse" });
    expect(reverse).not.toBeChecked();
    await act(async () => {
      fireEvent.click(reverse);
    });
    expect(applySpy).toHaveBeenCalledTimes(1);
    expect(applySpy).toHaveBeenCalledWith([
      { type: "updateClipReverse", itemId: "clip", reverse: true },
      { type: "updateClipReverse", itemId: "clip-sound", reverse: true },
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    const reversedIds = store.getState().project.timeline.tracks.flatMap((track) => track.items).filter((item) => item.properties.reverse === true);
    expect(reversedIds.map((item) => item.id)).toEqual(["clip", "clip-sound"]);
    expect(screen.getByRole("switch", { name: "Reverse" })).toBeChecked();

    await act(async () => {
      fireEvent.click(screen.getByRole("switch", { name: "Reverse" }));
    });
    expect(applySpy).toHaveBeenLastCalledWith([
      { type: "updateClipReverse", itemId: "clip", reverse: false },
      { type: "updateClipReverse", itemId: "clip-sound", reverse: false },
    ]);
  });

  it("offers Reverse only for clips that can play reversed, disabled with the reason on a locked track", async () => {
    const image = await renderProperties(testProject([testItem("still", "image_clip")]), ["still"], "Speed");
    expect(screen.getByRole("group", { name: "Speed presets" })).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Reverse" })).not.toBeInTheDocument();
    image.unmount();

    await renderProperties(projectWithTracks([track("video", "video", [testItem("clip", "video_clip")], true)]), ["clip"], "Speed");
    const reverse = screen.getByRole("switch", { name: "Reverse" });
    expect(reverse).toBeDisabled();
    expect(reverse).toHaveAccessibleDescription("Unlock the track to reverse this clip.");
  });

  it("applies animation presets and reveals keyframe lanes in the timeline", async () => {
    const keyframes = { opacity: [{ atSeconds: 0, value: 1 }] };
    const { store, applyActions } = await renderProperties(
      testProject([testItem("clip", "video_clip", { keyframes })]),
      ["clip"],
      "Animation",
    );
    const inGrid = screen.getByRole("group", { name: "In animation" });
    expect(within(inGrid).getByRole("button", { name: "None" })).toHaveAttribute("aria-pressed", "true");
    const [, firstPreset] = within(inGrid).getAllByRole("button");
    if (!firstPreset) throw new Error("preset");
    fireEvent.click(firstPreset);
    const batch = applyActions.mock.calls[0]?.[0] ?? [];
    expect(batch.some((action) => action.type === "setItemKeyframes")).toBe(true);
    expect(batch.at(-1)).toMatchObject({ type: "updateItemProperties" });

    fireEvent.click(screen.getByRole("button", { name: "Show Opacity keyframes in timeline" }));
    expect(store.getState().keyframesVisible).toBe(true);
    expect(store.getState().laneProperty).toBe("opacity");
  });

  it("hides the Audio tab for a silent video clip", async () => {
    await renderProperties(testProject([testItem("silent", "video_clip", {}, "missing-media")]), ["silent"]);
    expect(tabNames()).toEqual(["Video", "Speed", "Animation", "AI"]);
  });

  it("edits the linked audio clip from a video clip's Audio tab", async () => {
    const project = testProject(
      [testItem("clip", "video_clip", { linkGroupId: "link-clip" })],
      [testItem("clip-audio", "audio_clip", { linkGroupId: "link-clip" })],
    );
    const { applyActions } = await renderProperties(project, ["clip"], "Audio");
    await enterValue("Volume", "-6");
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateAudioVolume", itemId: "clip-audio", volumeDb: -6 }]);
    expect(screen.getByRole("switch", { name: "Denoise" })).toBeInTheDocument();
  });

  it("offers Detach audio on a video clip Audio tab without a linked audio clip", async () => {
    const { applyActions } = await renderProperties(testProject([testItem("clip", "video_clip")]), ["clip"], "Audio");
    expect(screen.getByText("This clip's sound isn't on its own audio clip yet.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Detach audio" }));
    await act(async () => undefined);
    expect(applyActions).toHaveBeenCalledWith([
      expect.objectContaining({ type: "detachAudio", itemId: "clip", audioItemId: "clip-audio", targetTrackId: "audio" }),
    ]);
  });
});
