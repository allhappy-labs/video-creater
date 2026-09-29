import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  appSettingsStorageKey,
  defaultAppPreferences,
  loadAppSettingsPreferences,
  updateAppPreferencesFromIntent,
} from "@/lib/app-settings";
import { listGenerationModelCatalog } from "@/lib/project";
import {
  listProviderCredentialStatuses,
  type ProviderCredentialStatus,
} from "@/lib/provider-credentials";
import { getAgentHealth } from "@/lib/settings/agent";
import { getSettingsHealthSnapshot } from "@/lib/settings/health";
import { getProviderHealth } from "@/lib/settings/providers";
import { getRenderSystemHealth } from "@/lib/settings/render-system";
import { getSkillsHealth } from "@/lib/settings/skills";
import { getStorageHealth } from "@/lib/settings/storage";
import { useSettingsOperations } from "@/lib/settings/use-settings-operations";
import {
  getActiveTranscriptionModel,
  getProductionSpeechModelStatus,
  getTranscriptionRuntimeStatus,
  listTranscriptionModels,
} from "@/lib/transcription-models";
import { Settings } from "./settings";

vi.mock("@/lib/settings/health", () => ({
  getSettingsHealthSnapshot: vi.fn(),
}));

vi.mock("@/lib/app-settings", async () => {
  const actual = await vi.importActual<typeof import("@/lib/app-settings")>(
    "@/lib/app-settings",
  );
  return {
    ...actual,
    loadAppSettingsPreferences: vi.fn(() => actual.defaultAppPreferences),
    updateAppPreferencesFromIntent: vi.fn(),
  };
});

vi.mock("@/lib/settings/agent", () => ({
  getAgentHealth: vi.fn(),
}));

vi.mock("@/lib/settings/providers", () => ({
  getProviderHealth: vi.fn(),
}));

vi.mock("@/lib/settings/render-system", () => ({
  getRenderSystemHealth: vi.fn(),
}));

vi.mock("@/lib/settings/skills", () => ({
  getSkillsHealth: vi.fn(),
}));

vi.mock("@/lib/settings/use-settings-operations", () => ({
  useSettingsOperations: vi.fn(),
}));

vi.mock("@/lib/transcription-models", () => ({
  downloadProductionSpeechModels: vi.fn(),
  downloadTranscriptionModel: vi.fn(),
  getActiveTranscriptionModel: vi.fn(),
  getProductionSpeechModelStatus: vi.fn(),
  getTranscriptionRuntimeStatus: vi.fn(),
  importTranscriptionModel: vi.fn(),
  listTranscriptionModels: vi.fn(),
  removeProductionSpeechModels: vi.fn(),
  removeTranscriptionModel: vi.fn(),
  setActiveTranscriptionModel: vi.fn(),
  verifyProductionSpeechModels: vi.fn(),
  verifyTranscriptionModel: vi.fn(),
}));

vi.mock("@/lib/project", async () => {
  const actual = await vi.importActual<typeof import("@/lib/project")>("@/lib/project");
  return { ...actual, listGenerationModelCatalog: vi.fn().mockResolvedValue({ loaded: false }) };
});

vi.mock("@/lib/provider-credentials", () => ({
  listProviderCredentialStatuses: vi.fn(),
}));

vi.mock("./general-settings", () => ({
  GeneralSettings: ({
    preferences,
    onPreferencesChange,
  }: {
    preferences: { renderCompletionNotifications: boolean };
    onPreferencesChange: (next: { renderCompletionNotifications: boolean }) => void;
  }) => (
    <div aria-label="General settings">
      <section data-testid="privacy-target" data-settings-target="general:privacy" tabIndex={-1}>
        Privacy
      </section>
      <section data-testid="notifications-target" data-settings-target="general:notifications" tabIndex={-1}>
        Notifications
      </section>
      General content {String(preferences.renderCompletionNotifications)}
      <button
        type="button"
        onClick={() => onPreferencesChange({ renderCompletionNotifications: true })}
      >
        Enable render notifications
      </button>
    </div>
  ),
}));
vi.mock("./models-settings", () => ({
  ModelsSettings: ({
    enabledGenerationModelIds,
    providerStatuses,
    onGenerationModelIdsChange,
    onConfigureGenerationProvider,
  }: {
    enabledGenerationModelIds: string[];
    providerStatuses: ProviderCredentialStatus[];
    onGenerationModelIdsChange: (modelIds: string[]) => void;
    onConfigureGenerationProvider: (provider: string) => void;
  }) => (
    <div>
      <section data-settings-target="aiModels:generationModels" tabIndex={-1}>
        <span data-testid="enabled-generation-model-ids">
          {enabledGenerationModelIds.join(",")}
        </span>
        <span data-testid="openai-credential-presence">
          {providerStatuses.find((status) => status.provider === "openai")
            ?.configured
            ? "OpenAI configured"
            : "OpenAI missing"}
        </span>
        <button
          type="button"
          onClick={() =>
            onGenerationModelIdsChange([
              ...enabledGenerationModelIds,
              "openai:gpt-image-2",
            ])
          }
        >
          Enable OpenAI image
        </button>
        <button
          type="button"
          onClick={() =>
            onGenerationModelIdsChange([
              ...enabledGenerationModelIds,
              "replicate:flux-schnell",
            ])
          }
        >
          Enable Replicate image
        </button>
        <button
          type="button"
          onClick={() =>
            onGenerationModelIdsChange([
              "openai:gpt-image-2",
              "replicate:flux-schnell",
            ])
          }
        >
          Select all image models
        </button>
        <button
          type="button"
          onClick={() => onConfigureGenerationProvider("openai")}
        >
          Configure OpenAI
        </button>
      </section>
      <section data-settings-target="aiModels:transcription" tabIndex={-1}>
        Models content
      </section>
    </div>
  ),
}));
vi.mock("./agent-mcp-settings", () => ({
  AgentMcpSettings: ({ projectRoot }: { projectRoot: string | null }) => (
    <div data-settings-target="advanced:mcp" tabIndex={-1}>
      Agent content {projectRoot ?? "No project"}
    </div>
  ),
}));
vi.mock("./storage-settings", () => ({
  StorageSettings: ({ activeProjectDir }: { activeProjectDir: string | null }) => (
    <div>
      <section data-settings-target="storage:projectLocation" tabIndex={-1}>
        Project location
      </section>
      <section data-settings-target="storage:cache" tabIndex={-1}>
        Storage content {activeProjectDir ?? "No project"}
      </section>
    </div>
  ),
}));
vi.mock("./providers-settings", () => ({
  ProvidersSettings: ({
    disabledGenerationModelIds = [],
    onCredentialStatusChange = () => undefined,
  }: {
    disabledGenerationModelIds?: string[];
    onCredentialStatusChange?: (status: ProviderCredentialStatus) => void;
  }) => (
    <div>
      <span>Providers content</span>
      <form
        aria-label="OpenAI credential"
        data-settings-target="integrations:openai"
        tabIndex={-1}
      >
        <input aria-label="Credential for OpenAI" />
        <button
          type="button"
          onClick={() =>
            onCredentialStatusChange({
              provider: "openai",
              displayName: "OpenAI",
              configured: true,
              source: "keychain",
            })
          }
        >
          Save OpenAI credential
        </button>
        <button
          type="button"
          onClick={() =>
            onCredentialStatusChange({
              provider: "openai",
              displayName: "OpenAI",
              configured: false,
              source: "missing",
            })
          }
        >
          Delete OpenAI credential
        </button>
      </form>
      <span>Disabled {disabledGenerationModelIds.join(",")}</span>
    </div>
  ),
}));
vi.mock("@/lib/settings/storage", () => ({
  getStorageHealth: vi.fn(),
}));

const mockGetSnapshot = vi.mocked(getSettingsHealthSnapshot);
const mockGetAgentHealth = vi.mocked(getAgentHealth);
const mockGetProviderHealth = vi.mocked(getProviderHealth);
const mockGetRenderSystemHealth = vi.mocked(getRenderSystemHealth);
const mockGetSkillsHealth = vi.mocked(getSkillsHealth);
const mockUseOperations = vi.mocked(useSettingsOperations);
const mockListModels = vi.mocked(listTranscriptionModels);
const mockGetActiveModel = vi.mocked(getActiveTranscriptionModel);
const mockGetRuntime = vi.mocked(getTranscriptionRuntimeStatus);
const mockGetSpeech = vi.mocked(getProductionSpeechModelStatus);
const mockGetStorageHealth = vi.mocked(getStorageHealth);
const mockListGenerationModelCatalog = vi.mocked(listGenerationModelCatalog);
const mockListProviderCredentialStatuses = vi.mocked(
  listProviderCredentialStatuses,
);
const mockLoadAppSettingsPreferences = vi.mocked(loadAppSettingsPreferences);
const mockUpdateAppPreferences = vi.mocked(updateAppPreferencesFromIntent);

const snapshot = {
  generatedAt: "2026-07-17T12:00:00Z",
  overall: "failed" as const,
  categories: {
    general: { id: "general", state: "ready" as const, items: [] },
    models: { id: "models", state: "actionRequired" as const, items: [] },
    agent: { id: "agent", state: "failed" as const, items: [] },
    skills: { id: "skills", state: "ready" as const, items: [] },
    storage: { id: "storage", state: "checking" as const, items: [] },
    providers: { id: "providers", state: "unavailable" as const, items: [] },
  },
};

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

describe("Settings integration owner", () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    window.localStorage.clear();
    mockLoadAppSettingsPreferences.mockReset().mockReturnValue(defaultAppPreferences);
    mockUpdateAppPreferences.mockReset().mockResolvedValue(defaultAppPreferences);
    mockListGenerationModelCatalog.mockReset().mockResolvedValue({
      loaded: false,
      generationModels: [],
      providerCredentialsExposed: false,
    });
    mockListProviderCredentialStatuses.mockReset().mockResolvedValue([
      {
        provider: "openai",
        displayName: "OpenAI",
        configured: false,
        source: "missing",
      },
    ]);
    mockGetSnapshot.mockReset().mockResolvedValue(snapshot);
    mockGetAgentHealth.mockReset().mockResolvedValue({ id: "agent", state: "ready", items: [] });
    mockGetProviderHealth.mockReset().mockResolvedValue([]);
    mockGetRenderSystemHealth.mockReset().mockRejectedValue(new Error("render diagnostics unavailable"));
    mockGetSkillsHealth.mockReset().mockRejectedValue(new Error("skills diagnostics unavailable"));
    mockUseOperations.mockReset().mockReturnValue({
      operations: [],
      error: null,
      hydrated: true,
      baselineOperationIds: new Set(),
      baselineTerminalOperationIds: new Set(),
    });
    mockListModels.mockReset().mockResolvedValue([]);
    mockGetActiveModel.mockReset().mockResolvedValue(null as never);
    mockGetRuntime.mockReset().mockResolvedValue("native");
    mockGetSpeech.mockReset().mockResolvedValue(null as never);
    mockGetStorageHealth.mockReset().mockResolvedValue({
      id: "storage",
      state: "ready",
      items: [],
    });
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("switches all seven categories without remounting the shell", async () => {
    render(
      <Settings
        initialCategory="general"
        projectRoot="/Projects/current"
        onBack={vi.fn()}
      />,
    );

    const shell = await screen.findByTestId("settings-shell");
    expect(mockGetSnapshot).not.toHaveBeenCalled();
    const tabs = within(shell).getAllByRole("tab");
    expect(tabs.map((tab) => tab.textContent?.replace(/\s+/g, " ").trim())).toEqual([
      "General",
      "Projects",
      "AI & Models",
      "Integrations",
      "Remote access",
      "Storage",
      "Advanced",
    ]);

    for (const [name, content] of [
      ["Projects", "New project defaults"],
      ["AI & Models", "Models content"],
      ["Integrations", "Providers content"],
      ["Remote access", "Open this editor from another device on your tailnet."],
      ["Storage", "Storage content /Projects/current"],
      ["Advanced", "Agent content /Projects/current"],
      ["General", "General content false"],
    ] as const) {
      fireEvent.click(within(shell).getByRole("tab", { name: new RegExp(`^${name}`) }));
      expect(screen.getByText(content, { exact: false })).toBeInTheDocument();
      expect(screen.getByTestId("settings-shell")).toBe(shell);
    }
  });

  it("focuses and announces a typed settings destination", async () => {
    const scrollIntoView = vi.fn();
    HTMLElement.prototype.scrollIntoView = scrollIntoView;

    render(
      <Settings
        target={{ category: "integrations", provider: "openai" }}
        navigationRequestId={41}
        projectRoot={null}
        onBack={vi.fn()}
      />,
    );

    const destination = await screen.findByRole("form", {
      name: "OpenAI credential",
    });
    await waitFor(() => expect(destination).toHaveFocus());
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
    expect(screen.getByRole("status")).toHaveTextContent(
      "Opened Openai integration",
    );
  });

  it("resolves distinct typed targets to distinct connected controls", async () => {
    const props = {
      navigationRequestId: 50,
      projectRoot: null,
      onBack: vi.fn(),
    };
    const view = render(
      <Settings {...props} target={{ category: "general", item: "privacy" }} />,
    );

    const privacy = await screen.findByTestId("privacy-target");
    await waitFor(() => expect(privacy).toHaveFocus());
    expect(document.body.contains(privacy)).toBe(true);

    view.rerender(
      <Settings
        {...props}
        navigationRequestId={51}
        target={{ category: "general", item: "notifications" }}
      />,
    );

    const notifications = screen.getByTestId("notifications-target");
    await waitFor(() => expect(notifications).toHaveFocus());
    expect(document.body.contains(notifications)).toBe(true);
    expect(notifications).not.toBe(privacy);
  });

  it("focuses a target without waiting for a global diagnostic snapshot", async () => {
    mockGetSnapshot.mockImplementation(() => new Promise(() => undefined));

    render(
      <Settings
        target={{ category: "integrations", provider: "openai" }}
        navigationRequestId={60}
        projectRoot={null}
        onBack={vi.fn()}
      />,
    );

    const destination = await screen.findByRole("form", {
      name: "OpenAI credential",
    });
    await waitFor(() => expect(destination).toHaveFocus());
    expect(
      screen.getByTestId("settings-shell").querySelector('[aria-live="polite"]'),
    ).toHaveTextContent(
      "Opened Openai integration",
    );
    expect(mockGetSnapshot).not.toHaveBeenCalled();
  });

  it("runs a repeated target for a new request id but not after manual tab changes", async () => {
    const props = {
      target: { category: "general", item: "privacy" } as const,
      navigationRequestId: 70,
      projectRoot: null,
      onBack: vi.fn(),
    };
    const view = render(<Settings {...props} />);
    const destination = await screen.findByTestId("privacy-target");
    const targetScroll = vi.fn();
    destination.scrollIntoView = targetScroll;

    view.rerender(<Settings {...props} navigationRequestId={71} />);
    await waitFor(() => expect(targetScroll).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    await waitFor(() => expect(screen.getByTestId("privacy-target")).toBeVisible());
    expect(targetScroll).toHaveBeenCalledTimes(1);
  });

  it("does not announce success when a typed target is missing", async () => {
    render(
      <Settings
        target={{ category: "integrations", provider: "missing-provider" }}
        navigationRequestId={80}
        projectRoot={null}
        onBack={vi.fn()}
      />,
    );

    await screen.findByText("Providers content");
    expect(screen.getByRole("status")).toHaveTextContent("");
    expect(
      screen.queryByRole("heading", { name: "Missing-provider integration" }),
    ).not.toBeInTheDocument();
  });

  it("lets a newer external request supersede internal provider navigation", async () => {
    const view = render(
      <Settings
        target={{ category: "aiModels", item: "generationModels" }}
        navigationRequestId={0}
        projectRoot={null}
        onBack={vi.fn()}
      />,
    );
    fireEvent.click(await screen.findByRole("button", { name: "Configure OpenAI" }));
    expect(await screen.findByText("Providers content")).toBeInTheDocument();

    view.rerender(
      <Settings
        target={{ category: "advanced", item: "mcp" }}
        navigationRequestId={1}
        projectRoot={null}
        onBack={vi.fn()}
      />,
    );

    const destination = await screen.findByText("Agent content No project");
    await waitFor(() => expect(destination).toHaveFocus());
  });

  it("renders only the accepted new-project defaults in Projects", async () => {
    mockLoadAppSettingsPreferences.mockReturnValueOnce({
      ...defaultAppPreferences,
      newProjectDefaults: {
        width: 3840,
        height: 2160,
        fps: 24,
        loudnessLufs: -16,
        captions: "mux",
      },
    });

    render(<Settings initialCategory="projects" projectRoot={null} onBack={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "New project defaults" })).toBeVisible();
    expect(screen.getByLabelText("Project width")).toHaveValue(3840);
    expect(screen.getByLabelText("Project height")).toHaveValue(2160);
    expect(screen.getByLabelText("Project frame rate")).toHaveValue("24");
    expect(screen.getByLabelText("Project loudness target")).toHaveValue(-16);
    expect(screen.getByLabelText("Project caption mode")).toHaveValue("mux");
    expect(screen.queryByLabelText("General settings")).not.toBeInTheDocument();
  });

  it("drives project controls from the native accepted preference value", async () => {
    mockUpdateAppPreferences.mockResolvedValueOnce({
      ...defaultAppPreferences,
      newProjectDefaults: {
        ...defaultAppPreferences.newProjectDefaults,
        fps: 24,
      },
    });
    render(<Settings initialCategory="projects" projectRoot={null} onBack={vi.fn()} />);
    const frameRate = await screen.findByLabelText("Project frame rate");

    fireEvent.change(frameRate, { target: { value: "24" } });

    expect(frameRate).toHaveValue("30");
    expect(mockUpdateAppPreferences).toHaveBeenCalledWith({
      newProjectDefaults: { fps: 24 },
    });
    await waitFor(() => expect(frameRate).toHaveValue("24"));
  });

  it("keeps the accepted project control state when Rust rejects a patch", async () => {
    mockUpdateAppPreferences.mockRejectedValueOnce(
      new Error("native validation rejected project defaults"),
    );
    render(<Settings initialCategory="projects" projectRoot={null} onBack={vi.fn()} />);
    const frameRate = await screen.findByLabelText("Project frame rate");

    fireEvent.change(frameRate, { target: { value: "24" } });

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "native validation rejected project defaults",
    );
    expect(frameRate).toHaveValue("30");
  });

  it("maps Advanced targets to truthful visible execution, MCP, and recovery sections", async () => {
    const props = {
      navigationRequestId: 90,
      projectRoot: null,
      onBack: vi.fn(),
    };
    const view = render(
      <Settings {...props} target={{ category: "advanced", item: "execution" }} />,
    );
    expect(await screen.findByRole("heading", { name: "Execution" })).toBeVisible();
    expect(screen.getByLabelText("Generation execution backend")).toHaveValue("inProcess");

    view.rerender(
      <Settings
        {...props}
        navigationRequestId={91}
        target={{ category: "advanced", item: "mcp" }}
      />,
    );
    await waitFor(() => expect(screen.getByText("Agent content No project")).toHaveFocus());

    view.rerender(
      <Settings
        {...props}
        navigationRequestId={92}
        target={{ category: "advanced", item: "recovery" }}
      />,
    );
    expect(await screen.findByRole("heading", { name: "Recovery" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Reset app preferences" })).toBeVisible();
  });

  it("drives Advanced execution from the native accepted value", async () => {
    mockUpdateAppPreferences.mockResolvedValueOnce({
      ...defaultAppPreferences,
      generationExecutionBackend: "temporal",
    });
    render(<Settings initialCategory="advanced" projectRoot={null} onBack={vi.fn()} />);
    const backend = await screen.findByLabelText("Generation execution backend");

    fireEvent.change(backend, { target: { value: "temporal" } });

    expect(backend).toHaveValue("inProcess");
    expect(mockUpdateAppPreferences).toHaveBeenCalledWith({
      generationExecutionBackend: "temporal",
    });
    await waitFor(() => expect(backend).toHaveValue("temporal"));
  });

  it("keeps preference pages usable when diagnostic APIs reject", async () => {
    mockGetSnapshot.mockRejectedValue(new Error("snapshot diagnostics unavailable"));
    mockGetAgentHealth.mockRejectedValue(new Error("agent diagnostics unavailable"));
    mockGetProviderHealth.mockRejectedValue(new Error("provider diagnostics unavailable"));
    mockGetStorageHealth.mockRejectedValue(new Error("storage diagnostics unavailable"));

    render(<Settings initialCategory="general" projectRoot="/Projects/current" onBack={vi.fn()} />);

    expect(await screen.findByText("General content false")).toBeVisible();
    for (const [name, content] of [
      ["Projects", "New project defaults"],
      ["Storage", "Storage content /Projects/current"],
      ["Advanced", "Agent content /Projects/current"],
      ["General", "General content false"],
    ] as const) {
      fireEvent.click(screen.getByRole("tab", { name }));
      expect(screen.getByText(content)).toBeVisible();
    }

    expect(mockGetSnapshot).not.toHaveBeenCalled();
    expect(mockGetAgentHealth).not.toHaveBeenCalled();
    expect(mockGetProviderHealth).not.toHaveBeenCalled();
    expect(mockGetRenderSystemHealth).not.toHaveBeenCalled();
    expect(mockGetSkillsHealth).not.toHaveBeenCalled();
    expect(mockGetStorageHealth).not.toHaveBeenCalled();
    expect(screen.queryByText(/diagnostics unavailable/)).not.toBeInTheDocument();
  });

  it("persists common preferences through Rust and reconciles its accepted value", async () => {
    mockUpdateAppPreferences.mockResolvedValueOnce({
      ...defaultAppPreferences,
      renderCompletionNotifications: true,
    });
    render(<Settings initialCategory="general" projectRoot={null} onBack={vi.fn()} />);
    await screen.findByText("General content false");

    fireEvent.click(screen.getByRole("button", { name: "Enable render notifications" }));
    await waitFor(() =>
      expect(mockUpdateAppPreferences).toHaveBeenCalledWith({
        renderCompletionNotifications: true,
      }),
    );
    expect(await screen.findByText("General content true")).toBeInTheDocument();
    expect(window.localStorage.getItem(appSettingsStorageKey)).toBeNull();
  });

  it("preserves the accepted preference after rejection and clears the error after retry", async () => {
    mockUpdateAppPreferences.mockRejectedValueOnce(
      new Error("native validation rejected the patch"),
    );
    mockUpdateAppPreferences.mockResolvedValueOnce({
      ...defaultAppPreferences,
      renderCompletionNotifications: true,
    });
    render(<Settings initialCategory="general" projectRoot={null} onBack={vi.fn()} />);
    await screen.findByText("General content false");

    fireEvent.click(screen.getByRole("button", { name: "Enable render notifications" }));

    await waitFor(() => expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(1));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "This change was not saved.",
    );
    expect(screen.getByText("General content false")).toBeInTheDocument();
    expect(screen.queryByText("General content true")).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Retry saving settings change" }),
    );

    await waitFor(() => expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("General content true")).toBeInTheDocument();
    expect(
      screen.queryByText("This change was not saved."),
    ).not.toBeInTheDocument();
    expect(window.localStorage.getItem(appSettingsStorageKey)).toBeNull();
  });

  it("adopts an earlier accepted write when the next serialized write fails", async () => {
    const firstWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(firstWrite.promise)
      .mockRejectedValueOnce(new Error("execution preference rejected"));
    render(<Settings initialCategory="general" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(await screen.findByRole("button", { name: "Enable render notifications" }));
    fireEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    fireEvent.change(screen.getByLabelText("Generation execution backend"), {
      target: { value: "temporal" },
    });

    expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(2);
    firstWrite.resolve({
      ...defaultAppPreferences,
      renderCompletionNotifications: true,
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("execution preference rejected");

    fireEvent.click(screen.getByRole("tab", { name: "General" }));
    expect(await screen.findByText("General content true")).toBeVisible();
  });

  it("rebases a later project field after an earlier field is rejected", async () => {
    const firstWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(firstWrite.promise)
      .mockResolvedValueOnce({
        ...defaultAppPreferences,
        newProjectDefaults: {
          ...defaultAppPreferences.newProjectDefaults,
          captions: "mux",
        },
      });
    render(<Settings initialCategory="projects" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.change(await screen.findByLabelText("Project frame rate"), {
      target: { value: "24" },
    });
    fireEvent.change(screen.getByLabelText("Project caption mode"), {
      target: { value: "mux" },
    });

    expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(2);
    firstWrite.reject(new Error("frame rate rejected"));
    expect(mockUpdateAppPreferences).toHaveBeenNthCalledWith(2, {
      newProjectDefaults: { captions: "mux" },
    });
    await waitFor(() => expect(screen.getByLabelText("Project caption mode")).toHaveValue("mux"));
    expect(screen.getByLabelText("Project frame rate")).toHaveValue("30");
  });

  it("serializes rapid project fields and rebases each full Rust patch on acceptance", async () => {
    const firstWrite = deferred<typeof defaultAppPreferences>();
    const secondWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(firstWrite.promise)
      .mockReturnValueOnce(secondWrite.promise);
    render(<Settings initialCategory="projects" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.change(await screen.findByLabelText("Project frame rate"), {
      target: { value: "24" },
    });
    fireEvent.change(screen.getByLabelText("Project caption mode"), {
      target: { value: "mux" },
    });

    expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(2);
    firstWrite.resolve({
      ...defaultAppPreferences,
      newProjectDefaults: {
        ...defaultAppPreferences.newProjectDefaults,
        fps: 24,
      },
    });
    expect(mockUpdateAppPreferences).toHaveBeenNthCalledWith(2, {
      newProjectDefaults: { captions: "mux" },
    });
    secondWrite.resolve({
      ...defaultAppPreferences,
      newProjectDefaults: {
        ...defaultAppPreferences.newProjectDefaults,
        fps: 24,
        captions: "mux",
      },
    });

    await waitFor(() => expect(screen.getByLabelText("Project frame rate")).toHaveValue("24"));
    expect(screen.getByLabelText("Project caption mode")).toHaveValue("mux");
  });

  it("persists compact model selection as a positive enabled allowlist", async () => {
    mockListGenerationModelCatalog.mockResolvedValueOnce({
      loaded: true,
      generationModels: [
        {
          provider: "openai",
          id: "gpt-image-2",
          kind: "image",
          displayName: "GPT Image 2",
          allowedEndpoints: [],
          responseShape: "images",
          uiCapabilities: {
            resolutions: null,
            aspectRatios: [],
            qualities: null,
            supportsImageReference: false,
            maxImages: 1,
          },
          paidOnly: false,
        },
        {
          provider: "replicate",
          id: "flux-schnell",
          kind: "image",
          displayName: "Flux Schnell",
          allowedEndpoints: [],
          responseShape: "images",
          uiCapabilities: {
            resolutions: null,
            aspectRatios: [],
            qualities: null,
            supportsImageReference: false,
            maxImages: 1,
          },
          paidOnly: false,
        },
      ],
      providerCredentialsExposed: false,
    });
    mockUpdateAppPreferences.mockResolvedValueOnce({
      ...defaultAppPreferences,
      enabledGenerationModelIds: ["openai:gpt-image-2"],
    });
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable OpenAI image" }),
    );

    await waitFor(() =>
      expect(mockUpdateAppPreferences).toHaveBeenCalledWith({
        enabledGenerationModelIds: ["openai:gpt-image-2"],
      }),
    );
    expect(window.localStorage.getItem(appSettingsStorageKey)).toBeNull();
  });

  it("keeps rapid generation selections on the queued desired allowlist", async () => {
    const firstWrite = deferred<typeof defaultAppPreferences>();
    const secondWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(firstWrite.promise)
      .mockReturnValueOnce(secondWrite.promise);
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable OpenAI image" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Enable Replicate image" }),
    );

    expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
      "openai:gpt-image-2,replicate:flux-schnell",
    );
    expect(mockUpdateAppPreferences).toHaveBeenNthCalledWith(2, {
      enabledGenerationModelIds: [
        "openai:gpt-image-2",
        "replicate:flux-schnell",
      ],
    });

    firstWrite.resolve({
      ...defaultAppPreferences,
      enabledGenerationModelIds: ["openai:gpt-image-2"],
    });
    await waitFor(() => expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(2));
    expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
      "openai:gpt-image-2,replicate:flux-schnell",
    );

    secondWrite.resolve({
      ...defaultAppPreferences,
      enabledGenerationModelIds: [
        "openai:gpt-image-2",
        "replicate:flux-schnell",
      ],
    });
    await waitFor(() =>
      expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
        "openai:gpt-image-2,replicate:flux-schnell",
      ),
    );
  });

  it("rolls back a rejected generation selection and rebases retry intent", async () => {
    const firstWrite = deferred<typeof defaultAppPreferences>();
    const secondWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(firstWrite.promise)
      .mockReturnValueOnce(secondWrite.promise)
      .mockResolvedValueOnce({
        ...defaultAppPreferences,
        enabledGenerationModelIds: [
          "openai:gpt-image-2",
          "replicate:flux-schnell",
        ],
      });
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable OpenAI image" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Enable Replicate image" }),
    );
    firstWrite.resolve({
      ...defaultAppPreferences,
      enabledGenerationModelIds: ["openai:gpt-image-2"],
    });
    secondWrite.reject(new Error("generation selection rejected"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "generation selection rejected",
    );
    expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
      "openai:gpt-image-2",
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Retry saving settings change" }),
    );
    await waitFor(() => expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(3));
    expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
      "openai:gpt-image-2,replicate:flux-schnell",
    );
  });

  it("keeps the latest generation failure retryable after an unrelated preference succeeds", async () => {
    const generationWrite = deferred<typeof defaultAppPreferences>();
    const backendWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(generationWrite.promise)
      .mockReturnValueOnce(backendWrite.promise)
      .mockResolvedValueOnce({
        ...defaultAppPreferences,
        enabledGenerationModelIds: ["openai:gpt-image-2"],
        generationExecutionBackend: "temporal",
      });
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable OpenAI image" }),
    );
    fireEvent.click(screen.getByRole("tab", { name: "Advanced" }));
    fireEvent.change(screen.getByLabelText("Generation execution backend"), {
      target: { value: "temporal" },
    });

    expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(2);
    await act(async () => {
      generationWrite.reject(new Error("generation selection rejected"));
      backendWrite.resolve({
        ...defaultAppPreferences,
        generationExecutionBackend: "temporal",
      });
    });

    fireEvent.click(screen.getByRole("tab", { name: "AI & Models" }));
    await waitFor(() =>
      expect(
        screen.getByTestId("enabled-generation-model-ids"),
      ).toBeEmptyDOMElement(),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "generation selection rejected",
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Retry saving settings change" }),
    );
    await waitFor(() => expect(mockUpdateAppPreferences).toHaveBeenCalledTimes(3));
    expect(mockUpdateAppPreferences).toHaveBeenNthCalledWith(3, {
      enabledGenerationModelIds: ["openai:gpt-image-2"],
    });
    expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
      "openai:gpt-image-2",
    );
  });

  it("clears an older generation failure when the newer full intent succeeds", async () => {
    const firstWrite = deferred<typeof defaultAppPreferences>();
    const secondWrite = deferred<typeof defaultAppPreferences>();
    mockUpdateAppPreferences
      .mockReturnValueOnce(firstWrite.promise)
      .mockReturnValueOnce(secondWrite.promise);
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable OpenAI image" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Enable Replicate image" }),
    );
    expect(mockUpdateAppPreferences).toHaveBeenNthCalledWith(2, {
      enabledGenerationModelIds: [
        "openai:gpt-image-2",
        "replicate:flux-schnell",
      ],
    });

    firstWrite.reject(new Error("older OpenAI write rejected"));
    secondWrite.resolve({
      ...defaultAppPreferences,
      enabledGenerationModelIds: [
        "openai:gpt-image-2",
        "replicate:flux-schnell",
      ],
    });

    await waitFor(() =>
      expect(screen.getByTestId("enabled-generation-model-ids")).toHaveTextContent(
        "openai:gpt-image-2,replicate:flux-schnell",
      ),
    );
    expect(
      screen.queryByText("This change was not saved."),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Retry saving settings change" }),
    ).not.toBeInTheDocument();
  });

  it("keeps Keychain presence current across configure, save, and delete navigation", async () => {
    const credentialList = deferred<ProviderCredentialStatus[]>();
    mockListProviderCredentialStatuses.mockReturnValueOnce(credentialList.promise);
    render(<Settings initialCategory="aiModels" projectRoot={null} onBack={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole("button", { name: "Configure OpenAI" }),
    );
    const credentialForm = await screen.findByRole("form", {
      name: "OpenAI credential",
    });
    fireEvent.click(
      within(credentialForm).getByRole("button", {
        name: "Save OpenAI credential",
      }),
    );
    await act(async () => {
      credentialList.resolve([
        {
          provider: "openai",
          displayName: "OpenAI",
          configured: false,
          source: "missing",
        },
      ]);
      await credentialList.promise;
    });

    fireEvent.click(screen.getByRole("tab", { name: "AI & Models" }));
    expect(await screen.findByTestId("openai-credential-presence")).toHaveTextContent(
      "OpenAI configured",
    );

    fireEvent.click(screen.getByRole("tab", { name: "Integrations" }));
    fireEvent.click(
      within(await screen.findByRole("form", { name: "OpenAI credential" })).getByRole(
        "button",
        { name: "Delete OpenAI credential" },
      ),
    );
    fireEvent.click(screen.getByRole("tab", { name: "AI & Models" }));
    expect(await screen.findByTestId("openai-credential-presence")).toHaveTextContent(
      "OpenAI missing",
    );
  });

});
