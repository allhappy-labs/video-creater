import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { backendRequest } from "@/lib/runtime/backend-client";
import { installPointerEventPolyfill } from "@/test-utils/editor-render";
import { enterValue, renderProperties, testItem, testProject } from "./properties-test-utils";
import { resetEffectCatalogForTests } from "./use-effect-catalog";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const audioProject = (properties: Record<string, unknown> = {}) => testProject([], [testItem("voice", "audio_clip", properties)]);

describe("audio property tabs", () => {
  beforeAll(() => installPointerEventPolyfill());

  beforeEach(() => {
    window.localStorage.clear();
    resetEffectCatalogForTests();
    vi.mocked(backendRequest).mockReset();
  });

  it("has Basic, Voice and Speed tabs", async () => {
    await renderProperties(audioProject(), ["voice"]);
    const tabs = within(screen.getByRole("tablist", { name: "Property tabs" })).getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent)).toEqual(["Basic", "Voice", "Speed"]);
  });

  it("retimes the audio clip from the Speed tab presets in one batch", async () => {
    const { applyActions } = await renderProperties(audioProject(), ["voice"], "Speed");
    fireEvent.click(within(screen.getByRole("group", { name: "Speed presets" })).getByRole("button", { name: "2×" }));
    expect(applyActions).toHaveBeenCalledWith([
      { type: "updateAudioClipSpeed", itemId: "voice", speed: 2 },
      { type: "resizeItems", resizes: [{ itemId: "voice", durationSeconds: 2 }] },
    ]);
  });

  it("reverses the audio clip from the Speed tab", async () => {
    const { applyActions } = await renderProperties(audioProject(), ["voice"], "Speed");
    await act(async () => {
      fireEvent.click(screen.getByRole("switch", { name: "Reverse" }));
    });
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateClipReverse", itemId: "voice", reverse: true }]);
  });

  it("sets volume in the legacy range and clears it when blank", async () => {
    const { applyActions } = await renderProperties(audioProject({ volumeDb: -3 }), ["voice"]);
    expect(screen.getByRole("textbox", { name: "Volume" })).toHaveValue("-3 dB");
    await enterValue("Volume", "-6.5");
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateAudioVolume", itemId: "voice", volumeDb: -6.5 }]);
    await enterValue("Volume", "");
    expect(applyActions).toHaveBeenLastCalledWith([{ type: "updateAudioVolume", itemId: "voice", volumeDb: null }]);
    await enterValue("Volume", "30");
    expect(screen.getByRole("textbox", { name: "Volume" })).toHaveAccessibleDescription("Enter a value from -60 dB to 24 dB.");
    expect(applyActions).toHaveBeenCalledTimes(2);
  });

  it("upserts keyframed volume at the playhead", async () => {
    const keyframes = { volumeDb: [{ atSeconds: 0, value: 0 }] };
    const { applyActions } = await renderProperties(audioProject({ keyframes }), ["voice"], undefined, { playheadSeconds: 2 });
    await enterValue("Volume", "-12");
    expect(applyActions).toHaveBeenCalledWith([
      { type: "upsertItemKeyframe", itemId: "voice", property: "volumeDb", keyframe: { atSeconds: 2, value: -12 } },
    ]);
  });

  it("commits audio fades and blocks fades longer than the clip", async () => {
    const { applyActions, setLastError } = await renderProperties(audioProject({ fadeInSeconds: 1 }), ["voice"]);
    await enterValue("Fade out", "2");
    expect(applyActions).toHaveBeenCalledWith([{ type: "updateAudioFades", itemId: "voice", fadeInSeconds: 1, fadeOutSeconds: 2 }]);
    await enterValue("Fade out", "3.5");
    expect(setLastError).toHaveBeenCalledWith("Fade in and fade out together can't be longer than the clip.");
    expect(applyActions).toHaveBeenCalledTimes(1);
  });

  it("enables denoise and sets its strength in one batch each", async () => {
    const { applyActions } = await renderProperties(audioProject(), ["voice"], "Voice");
    expect(screen.getByRole("slider", { name: "Strength" })).toHaveAttribute("data-disabled");
    fireEvent.click(screen.getByRole("switch", { name: "Denoise" }));
    expect(applyActions).toHaveBeenCalledTimes(1);
    const [batch] = applyActions.mock.calls[0] ?? [];
    expect(batch).toEqual([
      {
        type: "updateItemEffects",
        itemIds: ["voice"],
        effects: [{ effectInstanceId: expect.any(String), effectType: "audio.denoise", enabled: true, params: { amount: 0.6 } }],
      },
      {
        type: "updateItemProperties",
        updates: [
          {
            itemId: "voice",
            set: { audioDenoisePreparation: { status: "queued", progress: 0, retryable: true, algorithm: "adaptive-noise-gate-v1" } },
            remove: [],
          },
        ],
      },
    ]);
  });

  it("shows the denoise status and retries a failed preparation", async () => {
    const effects = [{ effectInstanceId: "d", effectType: "audio.denoise", enabled: true, params: { amount: 0.4 } }];
    const { applyActions } = await renderProperties(
      audioProject({ effects, audioDenoisePreparation: { status: "failed" } }),
      ["voice"],
      "Voice",
    );
    expect(screen.getByRole("switch", { name: "Denoise" })).toHaveAccessibleDescription("Failed");
    expect(screen.getByRole("textbox", { name: "Strength" })).toHaveValue("40%");
    await enterValue("Strength", "80");
    expect(applyActions.mock.calls[0]?.[0][0]).toMatchObject({ effects: [{ params: { amount: 0.8 } }] });
    fireEvent.click(screen.getByRole("button", { name: "Retry denoise" }));
    expect(applyActions).toHaveBeenLastCalledWith([
      {
        type: "updateItemProperties",
        updates: [{ itemId: "voice", set: { audioDenoisePreparation: { status: "queued", progress: 0, retryable: true, algorithm: "adaptive-noise-gate-v1" } }, remove: [] }],
      },
    ]);
  });

  it("routes Remove silences to the Audio tab's cleanup card", async () => {
    const { store, applyActions } = await renderProperties(audioProject(), ["voice"], "Voice");
    const button = screen.getByRole("button", { name: "Remove silences…" });
    expect(button).not.toHaveAttribute("aria-disabled");
    fireEvent.click(button);
    expect(store.getState().activeTab).toBe("audio");
    expect(store.getState().openSheetId).toBeNull();
    expect(store.getState().pendingCleanupFocus).toBe("removeSilences");
    expect(applyActions).not.toHaveBeenCalled();
  });

  it("opens the Audio sheet for Remove silences when Properties shows in a sheet", async () => {
    const { store } = await renderProperties(audioProject(), ["voice"], "Voice");
    act(() => store.getState().openSheet("property:volume"));
    fireEvent.click(screen.getByRole("button", { name: "Remove silences…" }));
    expect(store.getState().openSheetId).toBe("audio");
    expect(store.getState().pendingCleanupFocus).toBe("removeSilences");
  });

  it("renames speakers on blur with a trimmed, non-empty name", async () => {
    vi.mocked(backendRequest).mockImplementation(async (command: string, input?: unknown) => {
      if (command === "get_project_speaker_registry") return { speakers: [{ id: "s1", name: "Host", color: "#ff5a5a" }] };
      if (command === "rename_project_speaker") {
        const { name } = input as { name: string };
        return { speakers: [{ id: "s1", name, color: "#ff5a5a" }] };
      }
      return undefined;
    });
    const splitProject = testProject([], [testItem("voice", "audio_clip")], { schemaVersion: 2 });
    await renderProperties(splitProject, ["voice"], "Voice", { projectDir: "/projects/demo" });
    expect(await screen.findByRole("textbox", { name: "Name for Host" })).toHaveValue("Host");

    const input = screen.getByRole("textbox", { name: "Name for Host" });
    fireEvent.change(input, { target: { value: "   " } });
    await act(async () => {
      fireEvent.blur(input);
    });
    expect(input).toHaveValue("Host");
    expect(backendRequest).not.toHaveBeenCalledWith("rename_project_speaker", expect.anything());

    fireEvent.change(input, { target: { value: "  Anna " } });
    await act(async () => {
      fireEvent.blur(input);
    });
    expect(backendRequest).toHaveBeenCalledWith("rename_project_speaker", { projectDir: "/projects/demo", speakerId: "s1", name: "Anna" });
    expect(await screen.findByRole("textbox", { name: "Name for Anna" })).toHaveValue("Anna");
  });
});
