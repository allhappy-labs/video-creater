import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject, VisualEffectDescriptor } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { projectWithTracks, testItem, track } from "../../properties/properties-test-utils";
import { resetEffectCatalogForTests } from "../../properties/use-effect-catalog";
import { EffectsPanel } from "./effects-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const grain: VisualEffectDescriptor = {
  id: "stylize.grain",
  displayName: "Film Grain",
  category: "Stylize",
  params: [
    { key: "amount", label: "Amount", min: 0, max: 1, defaultValue: 0, unit: "" },
    { key: "size", label: "Size", min: 0.5, max: 4, defaultValue: 1.5, unit: "" },
  ],
  colorEffect: false,
};
const blur: VisualEffectDescriptor = {
  id: "blur.gaussian",
  displayName: "Gaussian Blur",
  category: "Blur & Sharpen",
  params: [{ key: "radius", label: "Radius", min: 0, max: 50, defaultValue: 0, unit: "px" }],
  colorEffect: false,
};
const exposure: VisualEffectDescriptor = { id: "color.exposure", displayName: "Exposure", category: "Color", params: [], colorEffect: true };
const texture: VisualEffectDescriptor = { id: "stylize.texture", displayName: "Texture", category: "Stylize", params: [], resourceKey: "path", colorEffect: false };

function mockBackend(effects: readonly VisualEffectDescriptor[] = [grain, blur, exposure, texture]) {
  vi.mocked(backendRequest).mockImplementation(async (command: string) => {
    if (command === "list_visual_effect_catalog") return { source: "test", effectCount: effects.length, canonicalOrder: [], effects };
    throw new Error(`unexpected backend call ${command}`);
  });
}

function project(locked = false): VideoProject {
  return projectWithTracks([
    track("v1", "video", [testItem("clip", "video_clip")], locked),
    track("a1", "audio", [testItem("voice", "audio_clip")]),
  ]);
}

async function renderPanel(base = project(), selected: readonly string[] = []) {
  const rendered = renderWithEditorStore(<EffectsPanel />, { project: base, projectDir: "" });
  act(() => rendered.store.getState().selectItems(selected));
  await act(async () => undefined);
  return rendered;
}

function effectsOf(state: { project: VideoProject }, itemId: string) {
  return state.project.timeline.tracks.flatMap((entry) => entry.items).find((item) => item.id === itemId)?.properties.effects;
}

describe("EffectsPanel effects", () => {
  beforeEach(() => {
    window.localStorage.clear();
    resetEffectCatalogForTests();
    vi.mocked(backendRequest).mockReset();
    mockBackend();
  });

  it("lists clip effects with name and category, without color grade entries", async () => {
    await renderPanel();
    const list = screen.getByRole("list", { name: "Effects" });
    expect(within(list).getAllByRole("listitem").map((tile) => tile.textContent)).toEqual([
      "Film GrainStylize",
      "Gaussian BlurBlur & Sharpen",
      "TextureStylize",
    ]);
    expect(screen.queryByText("Exposure")).not.toBeInTheDocument();
  });

  it("filters by search and category", async () => {
    await renderPanel();
    fireEvent.change(screen.getByRole("searchbox", { name: "Search effects" }), { target: { value: " BLUR " } });
    expect(within(screen.getByRole("list", { name: "Effects" })).getAllByRole("listitem")).toHaveLength(1);
    fireEvent.change(screen.getByRole("searchbox", { name: "Search effects" }), { target: { value: "" } });

    const categories = screen.getByRole("group", { name: "Effect categories" });
    fireEvent.click(within(categories).getByRole("button", { name: "Stylize" }));
    expect(within(categories).getByRole("button", { name: "Stylize" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByText("Gaussian Blur")).not.toBeInTheDocument();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search effects" }), { target: { value: "nothing" } });
    expect(screen.getByText("No effects match this filter")).toBeInTheDocument();
  });

  it("applies an effect to the selected clip in one undo step and marks it applied", async () => {
    const { store } = await renderPanel(project(), ["clip"]);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Apply Film Grain" }));
    });
    expect(effectsOf(store.getState(), "clip")).toEqual([
      { effectInstanceId: "legacy:stylize.grain:1", effectType: "stylize.grain", enabled: true, params: { amount: 0, size: 1.5 } },
    ]);
    expect(store.getState().history.past).toHaveLength(1);

    const applied = screen.getByRole("button", { name: "Film Grain applied" });
    expect(applied).toHaveAttribute("aria-disabled", "true");
    fireEvent.focus(applied);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Already applied to this clip");
    await act(async () => {
      fireEvent.click(applied);
    });
    expect(store.getState().history.past).toHaveLength(1);
    // Applying a plain effect leaves Properties where it was.
    expect(store.getState().propertiesTabByKind).toEqual({});
  });

  it("keeps + focusable but blocked with the reason when no visual clip is selected", async () => {
    const { store } = await renderPanel(project(), ["voice"]);
    const add = screen.getByRole("button", { name: "Apply Film Grain" });
    expect(add).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("status")).toHaveTextContent("Select a video or image clip.");
    fireEvent.focus(add);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Select a video or image clip");
    await act(async () => {
      fireEvent.click(add);
    });
    expect(store.getState().history.past).toHaveLength(0);
    expect(effectsOf(store.getState(), "voice")).toBeUndefined();
  });

  it("blocks a clip on a locked track with the reason", async () => {
    await renderPanel(project(true), ["clip"]);
    expect(screen.getByRole("button", { name: "Apply Gaussian Blur" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("status")).toHaveTextContent("Unlock the track to apply effects.");
  });

  it("opens the Properties Video tab after applying a resource-backed effect", async () => {
    const { store } = await renderPanel(project(), ["clip"]);
    act(() => store.getState().openSheet("effects"));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Apply Texture" }));
    });
    expect(effectsOf(store.getState(), "clip")).toEqual([expect.objectContaining({ effectType: "stylize.texture", params: {} })]);
    expect(store.getState().propertiesTabByKind).toEqual({ visual: "video" });
    expect(store.getState().openSheetId).toBe("property:effects");
  });

  it("says when the catalog is unavailable", async () => {
    vi.mocked(backendRequest).mockRejectedValue(new Error("offline"));
    await renderPanel();
    expect(screen.getByText("Effect catalog unavailable.")).toBeInTheDocument();
  });
});

describe("EffectsPanel backgrounds", () => {
  beforeEach(() => {
    window.localStorage.clear();
    resetEffectCatalogForTests();
    vi.mocked(backendRequest).mockReset();
    mockBackend();
  });

  it("inserts a background as a hyperframe_scene clip on a new graphics track in one batch", async () => {
    const { store } = await renderPanel();
    act(() => store.getState().seek(1));
    fireEvent.click(within(screen.getByRole("group", { name: "Effects tab content" })).getByRole("button", { name: "Backgrounds" }));
    expect(screen.queryByRole("list", { name: "Effects" })).not.toBeInTheDocument();
    const tiles = within(screen.getByRole("list", { name: "Backgrounds" })).getAllByRole("listitem");
    expect(tiles[0]).toHaveTextContent("OctagramsProcedural");

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Add Octagrams to the timeline" }));
    });
    const graphics = store.getState().project.timeline.tracks.filter((entry) => entry.kind === "hyperframe_scene");
    expect(graphics).toHaveLength(1);
    expect(graphics[0]?.items).toEqual([
      expect.objectContaining({
        kind: "hyperframe_scene",
        startSeconds: 1,
        properties: expect.objectContaining({ shaderBackgroundTemplateId: "shadertoy-octagrams-v1" }),
      }),
    ]);
    expect(store.getState().history.past).toHaveLength(1);
    expect(store.getState().selectedItemIds).toEqual([graphics[0]?.items[0]?.id]);
  });

  it("keeps the built-in backgrounds the backend lists for the project", async () => {
    vi.mocked(backendRequest).mockImplementation(async (command: string, args?: unknown) => {
      if (command === "list_shader_background_templates") {
        expect(args).toEqual({ projectDir: "/projects/demo" });
        return [
          { id: "shadertoy-phantom-star-v1", name: "Phantom Star", category: "geometry", preview: { accentColor: "#fff", secondaryColor: "#000", description: "" } },
          { id: "user-template", name: "Mine", category: "user", preview: { accentColor: "#fff", secondaryColor: "#000", description: "" } },
        ];
      }
      return { effects: [] };
    });
    renderWithEditorStore(<EffectsPanel />, { project: project(), projectDir: "/projects/demo" });
    fireEvent.click(screen.getByRole("button", { name: "Backgrounds" }));
    await act(async () => undefined);
    expect(within(screen.getByRole("list", { name: "Backgrounds" })).getAllByRole("listitem").map((tile) => tile.textContent)).toEqual([
      "Phantom StarGeometry",
    ]);
  });
});
