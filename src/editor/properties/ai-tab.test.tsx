import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, screen, within } from "@testing-library/react";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { GeneratedAsset, ProjectAction, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureGeneratedAsset, fixtureProject } from "@/test-utils/editor-fixtures";
import { installMediaAndFrameStubs } from "../preview/media-element-stubs";
import { renderProperties } from "./properties-test-utils";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

const seedance = { provider: "replicate", id: "bytedance/seedance-2.0-fast" };

function output(mediaId: string) {
  return { mediaId, relativePath: `generated/${mediaId}.mp4`, width: 1280, height: 720, durationSeconds: 5, fps: 24 };
}

/** The sample project with the generated clip's asset patched, plus extra assets and media. */
function project(patch: Partial<GeneratedAsset> = {}, extra: GeneratedAsset[] = [], split = false): VideoProject {
  const base = fixtureProject();
  const extraMedia = extra.flatMap((asset) => asset.outputs.map((out) => ({ id: out.mediaId, relativePath: out.relativePath, kind: "generated" as const, durationSeconds: 5, width: 1280, height: 720, fps: 24, folderId: null })));
  return {
    ...base,
    schemaVersion: split ? 2 : base.schemaVersion,
    media: [...base.media, ...extraMedia],
    generatedAssets: [{ ...fixtureGeneratedAsset(base), ...patch }, ...extra],
  };
}

function storm(): GeneratedAsset {
  return { ...fixtureGeneratedAsset(fixtureProject()), id: "storm", name: "Storm Clouds", createdAt: "2026-08-01T00:00:00Z", parentAssetId: "sample-generated-shot", retryOfAssetId: "sample-generated-shot", outputs: [output("storm-out")] };
}

async function renderAi(value: VideoProject, itemId: string, options: { projectDir?: string } = {}) {
  const rendered = await renderProperties(value, [itemId], "AI", options);
  const batches: ProjectAction[][] = [];
  rendered.applyActions.mockImplementation(async (actions) => {
    batches.push([...actions]);
    return rendered.store.getState().project;
  });
  return { ...rendered, batches };
}

function actions() {
  return screen.getByRole("region", { name: "AI actions" });
}

describe("clip AI tab", () => {
  beforeAll(() => installMediaAndFrameStubs());

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    vi.mocked(backendRequest).mockRejectedValue(new BackendUnavailableError());
  });

  it("shows the generated details for the fixture generated clip", async () => {
    await renderAi(project(), "sample-generated-clip");
    const details = screen.getByRole("region", { name: "Generated" });
    expect(details).toHaveTextContent("Restore the public-domain newsreel with a warmer high-contrast treatment");
    expect(details).toHaveTextContent("local/bundled-edison-restoration");
    expect(within(details).getByRole("listitem", { name: "First frame: input.mp4" })).toBeInTheDocument();
    expect(details).not.toHaveTextContent("sample-generated-shot");
    // The bundled model can't be rerun, so variations explain why.
    expect(screen.getByRole("button", { name: "Create 2 variations" })).toHaveAttribute("aria-disabled", "true");
    expect(actions()).toHaveTextContent("This model can't make variations.");
  });

  it("offers AI only on generated audio, without visual actions", async () => {
    const value = project({ kind: "audio", outputs: [{ ...output("media-voiceover"), relativePath: "media/voiceover.m4a" }] });
    await renderAi(value, "music-bed");
    expect(screen.getByRole("region", { name: "Generated" })).toBeInTheDocument();
    for (const name of ["Upscale", "Replace with generated…", "Generate music from video"]) expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();

    cleanup();
    await renderProperties(fixtureProject(), ["music-bed"]);
    expect(screen.getByRole("tablist", { name: "Property tabs" })).not.toHaveTextContent("AI");
  });

  it("swaps a variation into the clip from the variation set", async () => {
    const { batches } = await renderAi(project({}, [storm()], true), "sample-generated-clip", { projectDir: "/p" });
    const set = screen.getByRole("list", { name: "Variation set" });
    expect(within(set).getByText("Current")).toBeInTheDocument();
    await act(async () => {
      fireEvent.click(within(set).getByRole("button", { name: "Use Storm Clouds" }));
    });
    expect(batches).toEqual([[{ type: "replaceTimelineItemWithGeneratedOutput", replacement: { itemId: "sample-generated-clip", mediaId: "storm-out" } }]]);
  });

  it("explains why a variation can't be swapped outside a project folder", async () => {
    const { batches } = await renderAi(project({}, [storm()]), "sample-generated-clip");
    const use = screen.getByRole("button", { name: "Use Storm Clouds" });
    expect(use).toHaveAccessibleDescription("Save the project to a folder to swap generated outputs.");
    fireEvent.click(use);
    expect(batches).toEqual([]);
  });

  it("shows the cost and network notice before creating variations, and starts only on confirm", async () => {
    const { batches } = await renderAi(project({ model: seedance, settings: { width: 1280, height: 720, durationSeconds: 5, fps: 24, aspectRatio: "16:9", resolution: "720p", generateAudio: false } }), "sample-generated-clip");
    fireEvent.click(screen.getByRole("button", { name: "Create 2 variations" }));
    const confirm = screen.getByRole("group", { name: "Create 2 variations" });
    expect(confirm).toHaveTextContent("Storm Clouds, Radiant Backlight, each added to Media when ready.");
    expect(confirm).toHaveTextContent("Est. 8 credits");
    expect(confirm).toHaveTextContent("Uses Replicate · network");
    expect(batches).toEqual([]);

    await act(async () => {
      fireEvent.click(within(confirm).getByRole("button", { name: "Upload and create 2 variations" }));
    });
    expect(batches[0]?.map((action) => action.type)).toEqual(["recordJob", "recordGeneratedAsset", "recordJob", "recordGeneratedAsset"]);
    expect(screen.getByRole("status")).toHaveTextContent("Create 2 variations started.");
  });

  it("confirms an upscale of an imported clip and cancels without starting", async () => {
    const { batches } = await renderAi(project(), "item-1");
    expect(screen.queryByRole("region", { name: "Generated" })).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Create variations" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Upscale" }));
    const confirm = screen.getByRole("group", { name: "Upscale" });
    expect(confirm).toHaveTextContent("Makes a 1280×720 copy of this media, added to Media when ready.");
    expect(confirm).toHaveTextContent("Est. varies");
    expect(confirm).toHaveTextContent("Uses fal.ai · network");
    expect(confirm).toHaveTextContent("This generation references local project media.");
    fireEvent.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("group", { name: "Upscale" })).not.toBeInTheDocument();
    expect(batches).toEqual([]);
  });

  it("starts music from a video clip after confirming", async () => {
    const { batches } = await renderAi(project(), "item-1");
    fireEvent.click(screen.getByRole("button", { name: "Generate music from video" }));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Upload and generate music" }));
    });
    expect(batches[0]?.[1]).toMatchObject({ type: "recordGeneratedAsset", asset: { kind: "audio", placementIntent: "timeline", name: "Generated music" } });
  });

  it("disables every action while the clip's generation is running", async () => {
    await renderAi(project({ status: "running" }), "sample-generated-clip");
    expect(screen.getByRole("region", { name: "Generated" })).toHaveTextContent("Generating…");
    for (const name of ["Create 1 variation", "Replace with generated…", "Upscale", "Generate music from video", "Generate sound effects from video"]) {
      expect(within(actions()).getByRole("button", { name })).toHaveAttribute("aria-disabled", "true");
    }
    expect(actions()).toHaveTextContent("Generation in progress");
  });

  it("shows the upscale limit for 4K video", async () => {
    const value = project();
    const media = value.media.map((asset) => (asset.id === "media-1" ? { ...asset, width: 3840, height: 2160 } : asset));
    await renderAi({ ...value, media }, "item-1");
    expect(screen.getByRole("button", { name: "Upscale" })).toHaveAccessibleDescription("Already 4K or higher");
  });

  it("opens the Generate view to replace the clip", async () => {
    const { store } = await renderAi(project(), "item-1");
    fireEvent.click(screen.getByRole("button", { name: "Replace with generated…" }));
    expect(store.getState().generateView).toEqual({ open: true, mode: "video", placementIntent: "replace:item-1" });
    expect(store.getState().activeTab).toBe("media");
  });

  it("asks the AI tab about the clip", async () => {
    const { store } = await renderAi(project(), "item-1");
    act(() => store.getState().openSheet("property:ai"));
    fireEvent.click(screen.getByRole("button", { name: "Ask AI about this clip" }));
    expect(store.getState().pendingAgentRequest).toEqual({ itemIds: ["item-1"] });
    expect(store.getState().activeTab).toBe("ai");
    expect(store.getState().openSheetId).toBe("ai");
  });
});
