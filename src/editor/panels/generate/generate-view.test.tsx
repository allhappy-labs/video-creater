import "@testing-library/jest-dom/vitest";
import { act, cleanup, fireEvent, screen, waitFor, within } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { defaultAppPreferences } from "@/lib/app-settings";
import type { VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";
import { renderWithEditorStore } from "@/test-utils/editor-render";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { installMediaAndFrameStubs } from "../../preview/media-element-stubs";
import { EditorEnvironmentProvider } from "../../services/editor-environment";
import { MediaPanel } from "../media/media-panel";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

function mockBackend(handlers: Record<string, Handler> = {}) {
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
}

function project(): VideoProject {
  const base = fixtureProject();
  return {
    ...base,
    media: [...base.media, { id: "media-still", name: "Poster", relativePath: "media/poster.png", kind: "image", durationSeconds: 0, width: 10, height: 10, fps: null, folderId: null }],
  };
}

async function settle() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

/** The Media tab with the Generate view open; `environment` wraps it in an editor environment. */
async function renderGenerate(options: { environment?: (ui: ReactElement) => ReactElement } = {}) {
  const ui = <MediaPanel />;
  const rendered = renderWithEditorStore(options.environment ? options.environment(ui) : ui, { project: project(), projectDir: "" });
  act(() => rendered.store.getState().setGenerateView({ open: true, mode: "video" }));
  await settle();
  return rendered;
}

function prompt() {
  return screen.getByRole("textbox", { name: "Prompt" });
}

function generateButton() {
  return screen.getByRole("button", { name: /^Generate$|^Starting/ });
}

function optionGroup(name: string) {
  return screen.queryByRole("radiogroup", { name });
}

describe("GenerateView", () => {
  beforeAll(() => installMediaAndFrameStubs());

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    mockBackend();
  });

  afterEach(() => {
    // Unmount first so resetting the runtime mode doesn't re-render a mounted panel.
    cleanup();
    installRuntimeMode("browser");
    delete window.__EDITOR_FIXTURE_RUNTIME__;
    vi.unstubAllEnvs();
  });

  it("disables Generate on an empty prompt and shows the reason, cost and network notice", async () => {
    await renderGenerate();
    expect(prompt()).toHaveAttribute("placeholder", "Describe the video");
    expect(generateButton()).toBeDisabled();
    expect(generateButton()).toHaveAccessibleDescription("Prompt required");
    expect(screen.getByText("Uses Replicate · network")).toBeInTheDocument();
    expect(screen.getByText(/^Est\. \d+ credits?$/)).toBeInTheDocument();

    fireEvent.change(prompt(), { target: { value: "A brass phonograph" } });
    expect(generateButton()).toBeEnabled();
    expect(screen.queryByText("Prompt required")).not.toBeInTheDocument();
  });

  it("shows only the options the selected mode and model offer", async () => {
    await renderGenerate();
    expect(optionGroup("Duration")).toBeInTheDocument();
    expect(optionGroup("Aspect")).toBeInTheDocument();
    expect(optionGroup("Variations")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("radio", { name: "Image" }));
    expect(prompt()).toHaveAttribute("placeholder", "Describe the image");
    expect(optionGroup("Duration")).not.toBeInTheDocument();
    expect(within(optionGroup("Variations")!).getAllByRole("radio").map((radio) => radio.textContent)).toEqual(["1", "2", "3", "4"]);
    // The default image model takes no reference media, so there is no slot for it.
    expect(screen.queryByRole("button", { name: "Add reference" })).not.toBeInTheDocument();
  });

  it("updates the cost estimate when the duration changes", async () => {
    await renderGenerate();
    const cost = () => screen.getByText(/^Est\. /).textContent;
    const before = cost();
    fireEvent.click(within(optionGroup("Duration")!).getByRole("radio", { name: "10s" }));
    expect(cost()).not.toBe(before);
    expect(within(optionGroup("Duration")!).getByRole("radio", { name: "10s" })).toHaveAttribute("aria-checked", "true");
  });

  it("closes on submit and shows the queued generation with a progress tile", async () => {
    const { store } = await renderGenerate();
    fireEvent.change(prompt(), { target: { value: "A brass phonograph" } });
    await act(async () => {
      fireEvent.click(generateButton());
    });
    await waitFor(() => expect(store.getState().generateView).toBeNull());
    const tile = screen.getByRole("listitem", { name: "Generated media, queued" });
    expect(tile).toBeInTheDocument();
    expect(store.getState().project.generatedAssets.at(-1)).toMatchObject({ status: "queued", prompt: "A brass phonograph", model: { provider: "replicate" } });
  });

  it("names the clip a replacement generation will replace and records that placement", async () => {
    const { store } = await renderGenerate();
    act(() => store.getState().setGenerateView({ open: true, mode: "video", placementIntent: "replace:item-1" }));
    expect(screen.getByRole("status")).toHaveTextContent("Will replace Opening clip.");
    fireEvent.change(prompt(), { target: { value: "A brass phonograph" } });
    await act(async () => {
      fireEvent.click(generateButton());
    });
    await waitFor(() => expect(store.getState().generateView).toBeNull());
    expect(store.getState().project.generatedAssets.at(-1)).toMatchObject({ prompt: "A brass phonograph", placementIntent: "replace:item-1" });
  });

  it("asks before uploading referenced local media to the provider", async () => {
    const { store } = await renderGenerate();
    fireEvent.change(prompt(), { target: { value: "Open on the poster" } });
    fireEvent.keyDown(screen.getByRole("button", { name: "Choose first frame" }), { key: "Enter" });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Poster" }));
    expect(screen.getByRole("button", { name: "Remove first frame Poster" })).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(generateButton());
    });
    const dialog = await screen.findByRole("dialog", { name: "Upload referenced media?" });
    expect(dialog).toHaveTextContent("This generation references local project media.");
    expect(store.getState().project.generatedAssets.some((asset) => asset.prompt === "Open on the poster")).toBe(false);

    await act(async () => {
      fireEvent.click(within(dialog).getByRole("button", { name: "Upload and generate" }));
    });
    await waitFor(() => expect(store.getState().generateView).toBeNull());
    expect(store.getState().project.generatedAssets.at(-1)?.references).toMatchObject({ firstFrameMediaId: "media-still" });
  });

  it("blocks generation with a Settings link when the provider key is missing", async () => {
    const onOpenSettings = vi.fn();
    mockBackend({
      list_generation_model_catalog: () => ({
        loaded: true,
        providerCredentialsExposed: true,
        generationModels: [{ provider: "fal.ai", id: "fal-ai/kling-video/v2.1/standard/text-to-video", kind: "video", displayName: "Kling 2.1", allowedEndpoints: [], responseShape: "video", uiCapabilities: { durations: [5, 10], resolutions: null, aspectRatios: ["16:9", "9:16", "1:1"], supportsFirstFrame: false, supportsLastFrame: false, maxReferenceImages: 0 }, paidOnly: false }],
      }),
      list_provider_credential_statuses: () => [{ provider: "fal.ai", displayName: "fal", configured: false, source: "missing" }],
      get_temporal_worker_environment_report: () => null,
    });
    const preferences = { ...defaultAppPreferences, enabledGenerationModelIds: ["fal.ai:fal-ai/kling-video/v2.1/standard/text-to-video"] };
    await renderGenerate({
      environment: (ui) => (
        <EditorEnvironmentProvider transcriptionModelReady onOpenModelSettings={() => undefined} appPreferences={preferences} configurationRefreshId={1} onOpenSettings={onOpenSettings}>
          {ui}
        </EditorEnvironmentProvider>
      ),
    });
    await screen.findByText("fal video generation needs a fal provider key.");
    fireEvent.change(prompt(), { target: { value: "A brass phonograph" } });
    expect(generateButton()).toHaveAccessibleDescription("Configuration required");
    expect(screen.getByText("Uses fal · network")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Configure fal in Settings" }));
    expect(onOpenSettings).toHaveBeenCalledWith({ category: "integrations", provider: "fal.ai" }, expect.any(HTMLElement));
  });
});

describe("generation tiles", () => {
  beforeAll(() => installMediaAndFrameStubs());

  beforeEach(() => {
    window.localStorage.clear();
    vi.mocked(backendRequest).mockReset();
    mockBackend();
  });

  afterEach(() => {
    // Unmount first so resetting the runtime mode doesn't re-render a mounted panel.
    cleanup();
    installRuntimeMode("browser");
    delete window.__EDITOR_FIXTURE_RUNTIME__;
    vi.unstubAllEnvs();
  });

  function withGenerations(): VideoProject {
    const base = project();
    const template = base.generatedAssets[0]!;
    return {
      ...base,
      generatedAssets: [
        ...base.generatedAssets,
        { ...template, id: "gen-running", name: "Lab bench", status: "running", outputs: [] },
        { ...template, id: "gen-failed", name: "Phonograph", status: "failed", outputs: [{ ...template.outputs[0]!, mediaId: "media-lost", sourceUrl: "https://cdn.example/out.mp4" }] },
      ],
    };
  }

  async function openTileMenu(name: string) {
    await act(async () => {
      fireEvent.keyDown(screen.getByRole("button", { name: `${name} generation actions` }), { key: "Enter" });
    });
    return within(await screen.findByRole("menu"));
  }

  it("marks failures with Retry and Retry download, and hides mock controls outside the fixture runtime", async () => {
    installRuntimeMode("desktop");
    renderWithEditorStore(<MediaPanel />, { project: withGenerations(), projectDir: "" });
    await settle();
    expect(screen.getByRole("listitem", { name: "Lab bench, generating" })).toBeInTheDocument();
    expect(screen.getByRole("listitem", { name: "Phonograph, failed" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Retry Phonograph" })).toBeInTheDocument();

    const menu = await openTileMenu("Phonograph");
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Retry", "Retry download"]);
    expect(screen.queryByRole("button", { name: "Lab bench generation actions" })).not.toBeInTheDocument();
    expect(screen.queryByText("Complete (fixture)")).not.toBeInTheDocument();
  });

  it("offers Complete and Fail (fixture) on generating tiles in the fixture runtime", async () => {
    vi.stubEnv("DEV", true);
    window.__EDITOR_FIXTURE_RUNTIME__ = { enabled: true };
    installRuntimeMode("fixture");
    renderWithEditorStore(<MediaPanel />, { project: withGenerations(), projectDir: "" });
    await settle();

    const menu = await openTileMenu("Lab bench");
    expect(menu.getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Complete (fixture)", "Fail (fixture)"]);
  });
});
