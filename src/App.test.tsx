import "@testing-library/jest-dom/vitest";
import { useEffect, useState } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { appSettingsStorageKey } from "@/lib/app-settings";
import type { VideoProject } from "@/lib/project";
import { FixtureTransport } from "@/lib/runtime/adapters/fixture-transport";
import { RemoteTransport } from "@/lib/runtime/adapters/remote-transport";
import { setRemoteOutcomeReconciler } from "@/lib/runtime/adapters/remote-outcome-state";
import { BackendUnavailableError, RemoteOperationError } from "@/lib/runtime/backend-transport";
import { createRuntimeDescriptor } from "@/lib/runtime/runtime-descriptor";
import type { AppSettingsTarget } from "@/lib/settings/target";
import App, { createEmptySplitProject } from "./App";

const {
  appPreferencesMock,
  invokeMock,
  listenMock,
  nativeMenuHandler,
  settingsOperationHandler,
  editorMenuStateHandler,
  editorModuleLoadMock,
  editorMountMock,
} = vi.hoisted(() => ({
  appPreferencesMock: vi.fn(),
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  nativeMenuHandler: {
    current: null as null | ((event: { payload: unknown }) => void),
  },
  settingsOperationHandler: {
    current: null as null | ((event: { payload: unknown }) => void),
  },
  editorMenuStateHandler: {
    current: null as null | ((state: Record<string, unknown>) => void),
  },
  editorModuleLoadMock: vi.fn(),
  editorMountMock: vi.fn(),
}));

const connectedRuntime = createRuntimeDescriptor("fixture", {
  status: "connected",
  transport: new FixtureTransport(new Map()),
});

function renderConnectedApp() {
  return render(<App runtime={connectedRuntime} />);
}

function appPreferences(
  overrides: Partial<{
    schemaVersion: 2;
    projectLocation:
      | { mode: "ask" }
      | { mode: "suggestedParent"; parentPath: string };
    requireProviderUploadConfirmation: boolean;
    renderCompletionNotifications: boolean;
    newProjectDefaults: {
      width: number;
      height: number;
      fps: number;
      loudnessLufs: number;
      captions: "burn_in" | "mux" | "off";
    };
    enabledGenerationModelIds: string[];
    generationExecutionBackend: "inProcess" | "temporal";
  }> = {},
) {
  return {
    schemaVersion: 2 as const,
    projectLocation: { mode: "ask" as const },
    requireProviderUploadConfirmation: true,
    renderCompletionNotifications: false,
    newProjectDefaults: {
      width: 1920,
      height: 1080,
      fps: 30,
      loudnessLufs: -14,
      captions: "burn_in" as const,
    },
    enabledGenerationModelIds: [],
    generationExecutionBackend: "inProcess" as const,
    ...overrides,
  };
}

function savedProjectResult(input: unknown) {
  const { project, expectedRevision } = input as {
    project: VideoProject;
    expectedRevision: number;
  };
  expect(expectedRevision).toBe(project.contentRevision ?? 0);
  return {
    project: { ...project, contentRevision: expectedRevision + 1 },
    report: {
      manifestPath: "/tmp/project/video-creater.project.json",
      writtenFiles: [],
      removedFiles: [],
    },
  };
}

function settingsOrSavedProjectResult(command: string, input?: unknown) {
  return command === "save_split_project_to_folder"
    ? savedProjectResult(input)
    : settingsCommandResult(command);
}

vi.mock("@/lib/app-settings", async (importOriginal) => {
  const original = await importOriginal<typeof import("@/lib/app-settings")>();
  return {
    ...original,
    loadAppPreferences: appPreferencesMock,
  };
});

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (command: string, input?: unknown) => {
    const result =
      input === undefined
        ? invokeMock(command)
        : invokeMock(command, input);
    if (command !== "list_settings_operations") {
      return result;
    }
    return Promise.resolve(result).then((value) =>
      Array.isArray(value) ? value : [],
    );
  },
  backendListen: (
    eventName: string,
    handler: (payload: unknown) => void,
  ) =>
    listenMock(eventName, (event: { payload: unknown }) =>
      handler(event.payload),
    ),
}));

vi.mock("@/editor/editor-root", () => {
  editorModuleLoadMock();
  return {
  EditorRoot: ({
    runtimeReady,
    transcriptionModelReady,
    speechModelsReady,
    projectDir,
    initialProject,
    nativeMenuRequest,
    onOpenProjectHome,
    onOpenModelSettings,
    onOpenProjectSettings,
    configurationRefreshId,
    onNativeMenuStateChange,
  }: {
    runtimeReady: boolean;
    transcriptionModelReady: boolean;
    speechModelsReady: boolean;
    projectDir?: string;
    initialProject?: { id?: string };
    nativeMenuRequest?: { sequence: number; command: string } | null;
    onOpenProjectHome?: () => void;
    onOpenModelSettings?: (target?: AppSettingsTarget) => void;
    onOpenProjectSettings?: (originElement: HTMLElement) => void;
    configurationRefreshId?: number;
    onNativeMenuStateChange?: (state: Record<string, unknown>) => void;
  }) => {
    const [sessionCount, setSessionCount] = useState(0);
    useEffect(() => {
      editorMountMock();
      editorMenuStateHandler.current = onNativeMenuStateChange ?? null;
      onNativeMenuStateChange?.({
        view: "editor",
        canImport: true,
        canExport: false,
        canUndo: false,
        canRedo: false,
        canSplit: false,
        canTrimStart: false,
        canTrimEnd: false,
        canDelete: false,
        canRippleDelete: false,
        canSelectForward: false,
      });
      return () => {
        editorMenuStateHandler.current = null;
      };
    }, [onNativeMenuStateChange]);
    return <section aria-label="Mock editor workspace">
      <div>Runtime {runtimeReady ? "ready" : "unavailable"}</div>
      <div>Transcription model {transcriptionModelReady ? "ready" : "missing"}</div>
      <div>Speech models {speechModelsReady ? "ready" : "missing"}</div>
      <div>Project dir {projectDir ?? "none"}</div>
      <div>Project id {initialProject?.id ?? "sample"}</div>
      <button type="button" onClick={() => setSessionCount((count) => count + 1)}>
        Session count {sessionCount}
      </button>
      <div>Native command {nativeMenuRequest?.command ?? "none"}</div>
      <div>Configuration refresh {configurationRefreshId ?? 0}</div>
      <button type="button" onClick={onOpenProjectHome}>
        Home
      </button>
      <button type="button" onClick={() => onOpenModelSettings?.()}>
        Editor model settings
      </button>
      <button type="button" onClick={(event) => onOpenProjectSettings?.(event.currentTarget)}>
        Editor project settings
      </button>
      <button
        type="button"
        onClick={() =>
          onOpenModelSettings?.({
            category: "integrations",
            provider: "openai",
          })
        }
      >
        Configure provider
      </button>
    </section>;
  },
  };
});

vi.mock("@/components/settings/general-settings", () => ({
  GeneralSettings: ({
    preferences,
    onPreferencesChange,
  }: {
    preferences: ReturnType<typeof appPreferences>;
    onPreferencesChange: (patch: Record<string, unknown>) => void;
  }) => (
    <section aria-label="Mock general settings">
      <span data-testid="accepted-settings-project-location">
        {preferences.projectLocation.mode === "suggestedParent"
          ? preferences.projectLocation.parentPath
          : "ask"}
      </span>
      <span data-testid="accepted-settings-project-defaults">
        {preferences.newProjectDefaults.fps} {preferences.newProjectDefaults.captions}
      </span>
      <button
        type="button"
        onClick={() =>
          onPreferencesChange({
            projectLocation: {
              mode: "suggestedParent",
              parentPath: "/projects/accepted-parent",
            },
            newProjectDefaults: {
              width: 3840,
              height: 2160,
              fps: 24,
              loudnessLufs: -16,
              captions: "mux",
            },
          })
        }
      >
        Apply accepted project preferences
      </button>
      <button
        type="button"
        onClick={() =>
          onPreferencesChange({
            projectLocation: {
              mode: "suggestedParent",
              parentPath: "/projects/older-parent",
            },
          })
        }
      >
        Apply older project location
      </button>
      <button
        type="button"
        onClick={() =>
          onPreferencesChange({
            projectLocation: {
              mode: "suggestedParent",
              parentPath: "/projects/newer-parent",
            },
          })
        }
      >
        Apply newer project location
      </button>
      <button
        type="button"
        onClick={() => onPreferencesChange({ newProjectDefaults: { fps: 24 } })}
      >
        Apply project FPS
      </button>
      <button
        type="button"
        onClick={() => onPreferencesChange({ newProjectDefaults: { captions: "mux" } })}
      >
        Apply project captions
      </button>
    </section>
  ),
}));

function readySettingsHealthSnapshot() {
  const category = (id: string) => ({ id, state: "ready", items: [] });
  return {
    generatedAt: "2026-07-19T12:00:00Z",
    overall: "ready",
    categories: {
      general: category("general"),
      models: category("models"),
      agent: category("agent"),
      skills: category("skills"),
      storage: category("storage"),
      providers: category("providers"),
    },
  };
}

function readyTranscriptionModel(overrides: Record<string, unknown> = {}) {
  return {
    modelId: "nvidia/parakeet-tdt-0.6b-v3",
    displayName: "Parakeet TDT 0.6B v3",
    isActive: true,
    installStatus: "ready",
    localPath: "/models/parakeet",
    approximateSizeBytes: 485_000_000,
    downloadedFiles: 18,
    totalFiles: 18,
    verifiedAt: "2026-06-13T08:15:00Z",
    ...overrides,
  };
}

function readySpeechModels() {
  return {
    modelSetId: "production-speech-analysis-v1",
    runtimeId: "onnx",
    ready: true,
    installedFiles: 7,
    totalFiles: 7,
    installedBytes: 344_000_000,
    totalBytes: 344_000_000,
    rootPath: "/models/speech-analysis",
    vadRepo: "silero-vad",
    vadRevision: "main",
    diarizationRepo: "speaker-diarization",
    diarizationRevision: "main",
    artifactFormat: "ONNX",
    licenses: ["MIT"],
  };
}

function settingsCommandResult(command: string) {
  switch (command) {
    case "get_settings_health_snapshot":
      return readySettingsHealthSnapshot();
    case "get_agent_settings_health":
      return { id: "agent", state: "ready", items: [] };
    case "get_provider_health":
    case "list_provider_credential_statuses":
    case "list_settings_operations":
    case "list_transcription_models":
      return [];
    case "get_storage_health":
      return { id: "storage", state: "ready", items: [] };
    case "list_generation_model_catalog":
      return {
        loaded: false,
        generationModels: [],
        providerCredentialsExposed: false,
      };
    case "get_active_transcription_model":
    case "get_production_speech_model_status":
      return null;
    case "get_transcription_runtime_status":
      return "unavailable";
    default:
      return null;
  }
}


describe("App transcription model bridge handling", () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    appPreferencesMock.mockReset();
    appPreferencesMock.mockResolvedValue(appPreferences());
    invokeMock.mockReset();
    listenMock.mockReset();
    nativeMenuHandler.current = null;
    settingsOperationHandler.current = null;
    editorModuleLoadMock.mockClear();
    editorMountMock.mockClear();
    listenMock.mockImplementation(
      async (eventName: string, handler: (event: { payload: unknown }) => void) => {
        if (eventName === "settings-operation") {
          settingsOperationHandler.current = handler;
        } else {
          nativeMenuHandler.current = handler;
        }
        return vi.fn();
      },
    );
    window.localStorage.clear();
    window.sessionStorage.clear();
    setRemoteOutcomeReconciler(async () => null);
    consoleErrorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it("renders the disconnected browser shell without loading backend state", () => {
    render(
      <App
        runtime={createRuntimeDescriptor("browser", {
          status: "disconnected",
        })}
      />,
    );

    expect(
      screen.getByRole("main", { name: "Desktop host connection" }),
    ).toHaveTextContent("Pair this device");
    expect(appPreferencesMock).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(listenMock).not.toHaveBeenCalled();
  });

  it("does not register desktop menu bridges in a connected browser", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "remote_list_projects") return Promise.resolve([]);
      return Promise.resolve(settingsCommandResult(command));
    });
    render(
      <App
        runtime={createRuntimeDescriptor("browser", {
          status: "connected",
          transport: new FixtureTransport(new Map()),
        }, {
          kind: "connected",
          sessionId: "session-1",
          displayName: "Browser",
          hostLabel: "Studio host",
          csrfToken: "csrf-1",
        })}
      />,
    );

    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    expect(invokeMock).not.toHaveBeenCalledWith("sync_native_menu_state", expect.anything());
    expect(listenMock).not.toHaveBeenCalledWith(
      "video-creater://native-menu-command",
      expect.anything(),
    );
    expect(invokeMock).toHaveBeenCalledWith("remote_list_projects");
    expect(screen.queryByLabelText("Project folder path")).not.toBeInTheDocument();
    expect(screen.getByText("On Studio host")).toBeVisible();
  });

  it("does not expose persisted local recents while the remote catalog loads", async () => {
    window.localStorage.setItem("video-creater.recentProjects", JSON.stringify([{
      id: "recent:/projects/stale",
      name: "Stale local project",
      projectDir: "/projects/stale",
      updatedAtLabel: "Opened recently",
    }]));
    let resolveCatalog!: (projects: Array<{ projectId: string; name: string; updatedAtMs: number }>) => void;
    const catalog = new Promise<Array<{ projectId: string; name: string; updatedAtMs: number }>>(
      (resolve) => { resolveCatalog = resolve; },
    );
    invokeMock.mockImplementation((command: string) => {
      if (command === "remote_list_projects") return catalog;
      return Promise.resolve(settingsCommandResult(command));
    });

    render(
      <App
        runtime={createRuntimeDescriptor("browser", {
          status: "connected",
          transport: new FixtureTransport(new Map()),
        }, {
          kind: "connected",
          sessionId: "session-1",
          displayName: "Browser",
          hostLabel: "Studio host",
          csrfToken: "csrf-1",
        })}
      />,
    );

    expect(screen.queryByText("Stale local project")).not.toBeInTheDocument();
    resolveCatalog([{
        projectId: "remote-current",
        name: "Remote current project",
        updatedAtMs: 0,
      }]);

    expect(await screen.findByText("Remote current project")).toBeVisible();
    expect(screen.queryByText("Stale local project")).not.toBeInTheDocument();
  });

  it("loads the editor feature only when a project opens", async () => {
    invokeMock.mockRejectedValue(new BackendUnavailableError());
    renderConnectedApp();

    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    expect(editorModuleLoadMock).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));

    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeVisible();
    expect(editorModuleLoadMock).toHaveBeenCalledOnce();
  });

  it("keeps one remote catalog entry when initial listing races project creation", async () => {
    const catalog = deferred<Array<{ projectId: string; name: string; updatedAtMs: number }>>();
    const creation = deferred<{ catalogProjectId: string; project: VideoProject }>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "remote_list_projects") return catalog.promise;
      if (command === "remote_create_project") return creation.promise;
      return Promise.resolve(settingsCommandResult(command));
    });

    render(
      <App
        runtime={createRuntimeDescriptor("browser", {
          status: "connected",
          transport: new FixtureTransport(new Map()),
        }, {
          kind: "connected",
          sessionId: "session-1",
          displayName: "Browser",
          hostLabel: "Studio host",
          csrfToken: "csrf-1",
        })}
      />,
    );

    await screen.findByRole("main", { name: "Project home" });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByLabelText("Project name"), {
      target: { value: "Remote race project" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
      "remote_create_project",
      expect.anything(),
    ));

    const createdProject = createEmptySplitProject(
      "Remote race project",
      appPreferences().newProjectDefaults,
    );
    await act(async () => {
      catalog.resolve([{
        projectId: "remote-race-id",
        name: "Remote race project",
        updatedAtMs: Date.now(),
      }]);
      await catalog.promise;
      creation.resolve({
        catalogProjectId: "remote-race-id",
        project: createdProject,
      });
      await creation.promise;
    });

    fireEvent.click(await screen.findByRole("button", { name: "Home" }));
    expect(screen.getAllByRole("article", {
      name: "Recent project Remote race project",
    })).toHaveLength(1);
    expect(window.localStorage.getItem("video-creater.recentProjects")).toBeNull();
  });

  it("keeps unconfirmed remote creation disabled across reconnect and lets the user inspect the catalog", async () => {
    let listed = false;
    const createdProject = createEmptySplitProject("Accepted remote draft", appPreferences().newProjectDefaults);
    invokeMock.mockImplementation((command: string) => {
      if (command === "remote_list_projects") return Promise.resolve(listed ? [{ projectId: "accepted-id", name: createdProject.name, updatedAtMs: Date.now() }] : []);
      if (command === "remote_create_project") return Promise.reject(new RemoteOperationError(command, "outcome_unknown", "Project creation may have completed.", "lost-create", "rpc", "unknown"));
      if (command === "load_split_project_from_folder") return Promise.resolve(createdProject);
      return Promise.resolve(settingsCommandResult(command));
    });
    const runtime = (sessionId: string) => createRuntimeDescriptor("browser", { status: "connected", transport: new FixtureTransport(new Map()) }, {
      kind: "connected", sessionId, displayName: "Browser", hostLabel: "Studio host", csrfToken: "test-csrf",
    });
    const rendered = render(<App runtime={runtime("session-one")} />);
    await screen.findByRole("main", { name: "Project home" });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));
    fireEvent.change(screen.getByLabelText("Project name"), { target: { value: createdProject.name } });
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));
    expect(await screen.findByText("Project creation unconfirmed")).toBeVisible();
    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();
    rendered.rerender(<App runtime={createRuntimeDescriptor("browser", { status: "disconnected" })} />);
    rendered.rerender(<App runtime={runtime("session-two")} />);
    await screen.findByRole("main", { name: "Project home" });
    expect(screen.getByRole("button", { name: "New project" })).toBeDisabled();
    listed = true;
    fireEvent.click(screen.getByRole("button", { name: "Refresh host projects" }));
    const accepted = await screen.findByRole("article", { name: `Recent project ${createdProject.name}` });
    fireEvent.click(within(accepted).getByRole("button", { name: "Open project" }));
    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeVisible();
    expect(invokeMock.mock.calls.filter(([command]) => command === "remote_create_project")).toHaveLength(1);
  });

  it("disables project creation after reloading while a host creation response is still pending", async () => {
    let finish!: (response: Response) => void;
    let requestId = "";
    const first = new RemoteTransport({ csrfToken: "private-csrf", hostLabel: "Studio host", fetcher: async (_url, init) => {
      requestId = JSON.parse(String(init?.body)).requestId;
      return new Promise<Response>((resolve) => { finish = resolve; });
    } });
    const creation = first.request("remote_create_project", { project: { name: "Private draft" } });
    await vi.waitFor(() => expect(requestId).not.toBe(""));
    const restored = new RemoteTransport({ csrfToken: "new-csrf", hostLabel: "Studio host", fetcher: async (_url, init) => new Response(JSON.stringify({ requestId: JSON.parse(String(init?.body)).requestId, ok: true, result: [] })) });
    invokeMock.mockImplementation((command: string) => command === "remote_list_projects" ? Promise.resolve([]) : Promise.resolve(settingsCommandResult(command)));
    const runtime = createRuntimeDescriptor("browser", { status: "connected", transport: restored }, {
      kind: "connected", sessionId: "new-session", displayName: "Browser", hostLabel: "Studio host", csrfToken: "new-csrf",
    });
    render(<App runtime={runtime} />);
    await screen.findByRole("main", { name: "Project home" });
    expect(screen.getByRole("button", { name: "New project" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Refresh host projects" })).toBeEnabled();
    finish(new Response(JSON.stringify({ requestId, ok: true, result: { catalogProjectId: "accepted", project: {} } })));
    await creation;
    expect(screen.getByRole("button", { name: "New project" })).toBeDisabled();
  });

  it("keeps readiness at the defaults quietly when the backend is unavailable", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();

    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));

    expect(await screen.findByText("Runtime unavailable")).toBeInTheDocument();
    expect(screen.getByText("Transcription model missing")).toBeInTheDocument();
    expect(screen.getByText("Speech models missing")).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it("loads model readiness at startup without opening Settings", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "list_transcription_models") {
        return Promise.resolve([readyTranscriptionModel()]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(readyTranscriptionModel());
      }
      if (command === "get_transcription_runtime_status") {
        return Promise.resolve("native");
      }
      if (command === "get_production_speech_model_status") {
        return Promise.resolve(readySpeechModels());
      }
      return Promise.resolve(settingsOrSavedProjectResult(command, input));
    });

    renderConnectedApp();

    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_production_speech_model_status"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));

    expect(await screen.findByText("Transcription model ready")).toBeInTheDocument();
    expect(screen.getByText("Speech models ready")).toBeInTheDocument();
    expect(screen.getByText("Runtime ready")).toBeInTheDocument();
    // Settings never mounted: its operation feed and health probes were not requested.
    expect(invokeMock).not.toHaveBeenCalledWith("list_settings_operations");
    expect(invokeMock).not.toHaveBeenCalledWith("list_generation_model_catalog");
  });

  it("does not let an older startup readiness load overwrite the value Settings reported", async () => {
    const startupModel = deferred<unknown>();
    let activeModelRequests = 0;
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "get_active_transcription_model") {
        activeModelRequests += 1;
        return activeModelRequests === 1
          ? startupModel.promise
          : Promise.resolve(readyTranscriptionModel());
      }
      if (command === "get_transcription_runtime_status") {
        return Promise.resolve("native");
      }
      return Promise.resolve(settingsOrSavedProjectResult(command, input));
    });

    renderConnectedApp();

    await waitFor(() => expect(activeModelRequests).toBe(1));
    fireEvent.click(screen.getByRole("button", { name: "Model settings" }));
    await waitFor(() => expect(activeModelRequests).toBe(2));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("list_settings_operations"),
    );

    // The startup request resolves last with a stale "missing" model.
    await act(async () => {
      startupModel.resolve(readyTranscriptionModel({ installStatus: "missing" }));
      await startupModel.promise;
    });

    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "get_active_transcription_model") {
        return new Promise(() => {});
      }
      if (command === "get_transcription_runtime_status") {
        return Promise.resolve("native");
      }
      return Promise.resolve(settingsOrSavedProjectResult(command, input));
    });
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));

    expect(await screen.findByText("Transcription model ready")).toBeInTheDocument();
  });

  it("still logs non-bridge transcription command failures", async () => {
    const commandFailure = new Error("model store is unreadable");
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "materialize_sample_project_media") return Promise.resolve(null);
      if (command === "save_split_project_to_folder") {
        return Promise.resolve(savedProjectResult(input));
      }
      return Promise.reject(commandFailure);
    });

    renderConnectedApp();

    fireEvent.click(await screen.findByRole("button", { name: "Model settings" }));

    await waitFor(() => {
      expect(consoleErrorSpy).toHaveBeenCalledWith(
        "Failed to load transcription models",
        commandFailure,
      );
    });
  });

  it("treats unsupported platform runtime as unavailable for edit generation", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve({
          modelId: "nvidia/parakeet-tdt-0.6b-v3",
          displayName: "Parakeet TDT 0.6B v3",
          isActive: true,
          installStatus: "ready",
          localPath: "/models/parakeet",
          approximateSizeBytes: 485_000_000,
          downloadedFiles: 18,
          totalFiles: 18,
          verifiedAt: "2026-06-13T08:15:00Z",
        });
      }
      if (command === "get_transcription_runtime_status") {
        return Promise.resolve("unsupported_platform");
      }
      if (command === "get_production_speech_model_status") {
        return Promise.resolve({
          modelSetId: "production-speech-analysis-v1",
          runtimeId: "onnx",
          ready: true,
          installedFiles: 7,
          totalFiles: 7,
          installedBytes: 344_000_000,
          totalBytes: 344_000_000,
          rootPath: "/models/speech-analysis",
          vadRepo: "silero-vad",
          vadRevision: "main",
          diarizationRepo: "speaker-diarization",
          diarizationRevision: "main",
          artifactFormat: "ONNX",
          licenses: ["MIT"],
        });
      }
      if (command === "materialize_sample_project_media") return Promise.resolve(null);
      if (command === "save_split_project_to_folder") {
        return Promise.resolve(savedProjectResult(input));
      }
      if (command === "sync_native_menu_state") {
        return Promise.resolve(null);
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    renderConnectedApp();

    fireEvent.click(await screen.findByRole("button", { name: "Model settings" }));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("get_transcription_runtime_status", {
        modelId: null,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));

    expect(await screen.findByText("Transcription model ready")).toBeInTheDocument();
    expect(screen.getByText("Speech models ready")).toBeInTheDocument();
    expect(screen.getByText("Runtime unavailable")).toBeInTheDocument();
  });

  it("ignores the legacy update policy without checking a stub updater on mount", async () => {
    window.localStorage.setItem(
      appSettingsStorageKey,
      JSON.stringify({
        updatePolicy: "notify",
      }),
    );
    invokeMock.mockImplementation((command: string) => {
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      if (command === "get_transcription_runtime_status") {
        return Promise.resolve("unavailable");
      }
      return Promise.reject(new Error(`unexpected command ${command}`));
    });

    renderConnectedApp();

    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    expect(invokeMock).not.toHaveBeenCalledWith(
      "get_app_update_status",
      expect.anything(),
    );
  });

  it("opens to a project home with recent project actions", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();

    expect(await screen.findByRole("main", { name: "Project home" })).toBeInTheDocument();
    expect(screen.getByText("Recent projects")).toBeInTheDocument();
    expect(screen.getByText("Sample editor project")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Mock editor workspace" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));

    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeInTheDocument();
  });

  it("retries preferences bootstrap after a transient failure", async () => {
    invokeMock.mockResolvedValue(null);
    appPreferencesMock
      .mockRejectedValueOnce(new Error("settings service is starting"))
      .mockResolvedValueOnce(appPreferences());

    renderConnectedApp();

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "settings service is starting",
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry loading settings" }));

    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    expect(appPreferencesMock).toHaveBeenCalledTimes(2);
  });

  it("publishes native menu state from the active view owner", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "materialize_sample_project_media") return Promise.resolve(null);
      return Promise.resolve(settingsOrSavedProjectResult(command, input));
    });
    const latestMenuState = () => invokeMock.mock.calls
      .filter(([command]) => command === "sync_native_menu_state")
      .at(-1)?.[1];
    renderConnectedApp();

    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    await waitFor(() =>
      expect(latestMenuState()).toEqual({
        state: expect.objectContaining({ view: "editor", canExport: false }),
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Editor model settings" }));
    await waitFor(() =>
      expect(latestMenuState()).toEqual({
        state: expect.objectContaining({ view: "settings" }),
      }),
    );

    editorMenuStateHandler.current?.({
      view: "editor",
      canImport: true,
      canExport: true,
      mediaVisible: true,
      inspectorVisible: true,
      codexVisible: true,
      canMaximize: true,
      canSelectForward: false,
      canSplit: false,
      canTrimStart: false,
      canTrimEnd: false,
      canDelete: false,
      canRippleDelete: false,
      maximized: false,
      layoutPreset: "default",
    });
    await waitFor(() =>
      expect(latestMenuState()).toEqual({
        state: expect.objectContaining({ view: "settings" }),
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() =>
      expect(latestMenuState()).toEqual({
        state: expect.objectContaining({ view: "editor", canExport: true }),
      }),
    );
    expect(editorMountMock).toHaveBeenCalledOnce();
  });

  it("retains one project session through Settings and resets it for another project", async () => {
    invokeMock.mockImplementation((command: string, input?: { projectDir?: string }) => {
      if (command === "load_split_project_from_folder") {
        const projectDir = input?.projectDir ?? "";
        return Promise.resolve({
          id: projectDir.endsWith("project-b") ? "project-b" : "project-a",
          name: "Opened project",
        });
      }
      return Promise.resolve(settingsCommandResult(command));
    });
    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/project-a" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    fireEvent.click(await screen.findByRole("button", { name: "Session count 0" }));
    expect(screen.getByRole("button", { name: "Session count 1" })).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Editor model settings" }));
    fireEvent.click(await screen.findByRole("button", { name: "Back" }));
    expect(screen.getByRole("button", { name: "Session count 1" })).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/project-b" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));

    expect(await screen.findByRole("button", { name: "Session count 0" })).toBeVisible();
    expect(screen.getByText("Project id project-b")).toBeVisible();
  });

  it("routes typed native settings and editor help commands", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());

    nativeMenuHandler.current?.({
      payload: { sequence: 1, command: "openSettings" },
    });
    expect(await screen.findByTestId("settings-shell")).toBeVisible();
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );

    nativeMenuHandler.current?.({
      payload: { sequence: 2, command: "openProject" },
    });
    expect(await screen.findByRole("main", { name: "Project home" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));
    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeVisible();

    for (const [sequence, command] of [
      [3, "undo"],
      [4, "showTab:captions"],
      [5, "openConnectAgents"],
    ] as const) {
      nativeMenuHandler.current?.({ payload: { sequence, command } });
      expect(await screen.findByText(`Native command ${command}`)).toBeInTheDocument();
    }

    // Removed commands never reach the editor.
    nativeMenuHandler.current?.({
      payload: { sequence: 6, command: "openTour" },
    });
    expect(screen.queryByText("Native command openTour")).not.toBeInTheDocument();
  });

  it("sends feedback from any view without forwarding it to the editor", async () => {
    invokeMock.mockRejectedValue(new BackendUnavailableError());
    const openSpy = vi.spyOn(window, "open").mockImplementation(() => null);

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    nativeMenuHandler.current?.({ payload: { sequence: 1, command: "sendFeedback" } });
    expect(openSpy).toHaveBeenCalledWith("mailto:feedback@video-creater.local", "_self");

    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));
    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeVisible();
    nativeMenuHandler.current?.({ payload: { sequence: 2, command: "sendFeedback" } });
    expect(openSpy).toHaveBeenCalledTimes(2);
    expect(screen.getByText("Native command none")).toBeInTheDocument();
    openSpy.mockRestore();
  });

  it("routes native project settings and restores the editor invoking control", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    const origin = await screen.findByRole("button", {
      name: "Editor model settings",
    });
    origin.focus();

    nativeMenuHandler.current?.({
      payload: { sequence: 5, command: "openProjectSettings" },
    });

    expect(await screen.findByRole("main", { name: "Project Settings" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Back to editor" }));
    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeVisible();
    await waitFor(() => expect(origin).toHaveFocus());
  });

  it("opens project settings from the editor and restores the editor menu origin", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    const origin = await screen.findByRole("button", {
      name: "Editor project settings",
    });
    fireEvent.click(origin);

    expect(await screen.findByRole("main", { name: "Project Settings" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Back to editor" }));
    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeVisible();
    await waitFor(() => expect(origin).toHaveFocus());
  });

  it("routes native Advanced and System Health and restores the home invoking control", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) =>
      Promise.resolve(settingsOrSavedProjectResult(command, input)),
    );

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    const origin = await screen.findByRole("button", { name: "Model settings" });
    origin.focus();

    nativeMenuHandler.current?.({
      payload: { sequence: 6, command: "openAdvancedSettings" },
    });
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Advanced" })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() => expect(origin).toHaveFocus());

    nativeMenuHandler.current?.({
      payload: { sequence: 7, command: "openSystemHealth" },
    });
    expect(await screen.findByRole("main", { name: "System Health" })).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Back to Settings" }));
    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    await waitFor(() => expect(origin).toHaveFocus());

    nativeMenuHandler.current?.({
      payload: { sequence: 8, command: "openSystemHealth" },
    });
    expect(await screen.findByRole("main", { name: "System Health" })).toBeVisible();
    nativeMenuHandler.current?.({
      payload: { sequence: 9, command: "openSettings" },
    });
    expect(await screen.findByTestId("settings-shell")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("main", { name: "Project home" })).toBeVisible();
    await waitFor(() => expect(origin).toHaveFocus());
  });

  it("returns native Settings to the editor origin when opened from System Health", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) =>
      Promise.resolve(settingsOrSavedProjectResult(command, input)),
    );

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    const origin = await screen.findByRole("button", {
      name: "Editor model settings",
    });
    origin.focus();

    nativeMenuHandler.current?.({
      payload: { sequence: 10, command: "openSystemHealth" },
    });
    expect(await screen.findByRole("main", { name: "System Health" })).toBeVisible();

    nativeMenuHandler.current?.({
      payload: { sequence: 11, command: "openSettings" },
    });
    expect(await screen.findByTestId("settings-shell")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeVisible();
    await waitFor(() => expect(origin).toHaveFocus());
  });

  it("retargets open settings to Advanced without losing the editor return view", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "Editor model settings" }),
    );
    expect(await screen.findByTestId("settings-shell")).toBeVisible();
    expect(screen.getByRole("tab", { name: "AI & Models" })).toHaveAttribute(
      "aria-selected",
      "true",
    );

    nativeMenuHandler.current?.({
      payload: { sequence: 9, command: "openAdvancedSettings" },
    });
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "Advanced" })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeInTheDocument();
  });

  it("unmounts settings and restores the exact origin", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) =>
      Promise.resolve(settingsOrSavedProjectResult(command, input)),
    );

    renderConnectedApp();
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    const origin = await screen.findByRole("button", {
      name: "Editor model settings",
    });
    origin.focus();
    fireEvent.click(origin);

    const destination = await screen.findByRole("region", {
      name: "Transcription models",
    });
    await waitFor(() => expect(destination).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.queryByTestId("settings-shell")).not.toBeInTheDocument();
    await waitFor(() => expect(origin).toHaveFocus());
  });

  it("re-fetches configuration before returning focus from Settings", async () => {
    let resolvePreferencesRefresh!: (
      preferences: ReturnType<typeof appPreferences>,
    ) => void;
    const pendingPreferencesRefresh = new Promise<ReturnType<typeof appPreferences>>(
      (resolve) => {
        resolvePreferencesRefresh = resolve;
      },
    );
    appPreferencesMock
      .mockResolvedValueOnce(appPreferences())
      .mockReturnValueOnce(pendingPreferencesRefresh);
    invokeMock.mockImplementation((command: string, input?: unknown) =>
      Promise.resolve(settingsOrSavedProjectResult(command, input)),
    );

    renderConnectedApp();
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    const origin = await screen.findByRole("button", {
      name: "Editor model settings",
    });
    origin.focus();
    fireEvent.click(origin);
    await screen.findByTestId("settings-shell");
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    await waitFor(() => expect(appPreferencesMock).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("Configuration refresh 1")).toBeInTheDocument();
    await waitFor(() => expect(origin).toHaveFocus());
    await act(async () => {
      resolvePreferencesRefresh(
        appPreferences({ enabledGenerationModelIds: ["openai:gpt-image-2"] }),
      );
      await pendingPreferencesRefresh;
    });
  });

  it("cancels stale focus restoration when Settings is reopened immediately", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) =>
      Promise.resolve(settingsOrSavedProjectResult(command, input)),
    );
    let nextFrameId = 0;
    const frames = new Map<number, FrameRequestCallback>();
    const requestFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        nextFrameId += 1;
        frames.set(nextFrameId, callback);
        return nextFrameId;
      });
    const cancelFrame = vi
      .spyOn(window, "cancelAnimationFrame")
      .mockImplementation((frameId) => {
        frames.delete(frameId);
      });

    renderConnectedApp();
    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    const staleOrigin = await screen.findByRole("button", {
      name: "Configure provider",
    });
    fireEvent.click(staleOrigin);
    await screen.findByTestId("settings-shell");

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    const restorationFrame = nextFrameId;
    const replacementOrigin = screen.getByRole("button", {
      name: "Editor model settings",
    });
    replacementOrigin.focus();
    fireEvent.click(replacementOrigin);

    expect(cancelFrame).toHaveBeenCalledWith(restorationFrame);
    const transcriptionTarget = await screen.findByRole("region", {
      name: "Transcription models",
    });
    await waitFor(() => expect(frames.size).toBeGreaterThan(0));
    act(() => {
      for (const [frameId, callback] of [...frames]) {
        frames.delete(frameId);
        callback(0);
      }
    });
    await waitFor(() => expect(transcriptionTarget).toHaveFocus());
    expect(staleOrigin).not.toHaveFocus();

    requestFrame.mockRestore();
    cancelFrame.mockRestore();
  });

  it("opens AI & Models for every model-settings entry request", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();
    fireEvent.click(await screen.findByRole("button", { name: "Model settings" }));
    expect(await screen.findByRole("tab", { name: "AI & Models" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    fireEvent.click(screen.getByRole("tab", { name: "Integrations" }));
    expect(screen.getByRole("tab", { name: "Integrations" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    fireEvent.click(await screen.findByRole("button", { name: "Model settings" }));

    expect(await screen.findByRole("tab", { name: "AI & Models" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("resets the persistent Settings shell to General for repeated native open requests", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    nativeMenuHandler.current?.({
      payload: { sequence: 20, command: "openSettings" },
    });
    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
    fireEvent.click(screen.getByRole("tab", { name: "Integrations" }));

    nativeMenuHandler.current?.({
      payload: { sequence: 21, command: "openSettings" },
    });

    await waitFor(() =>
      expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute(
        "aria-selected",
        "true",
      ),
    );
  });

  it("returns from the editor to project home", async () => {
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();

    fireEvent.click(await screen.findByRole("button", { name: "Open sample" }));
    fireEvent.click(await screen.findByRole("button", { name: "Home" }));

    expect(await screen.findByRole("main", { name: "Project home" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Mock editor workspace" })).not.toBeInTheDocument();
  });

  it("opens a split project folder from the home surface", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "load_split_project_from_folder") {
        expect(input).toEqual({ projectDir: "/projects/launch-cut" });
        return Promise.resolve({ id: "launch-cut-project" });
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/launch-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));

    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeInTheDocument();
    expect(screen.getByText("Project dir /projects/launch-cut")).toBeInTheDocument();
    expect(screen.getByText("Project id launch-cut-project")).toBeInTheDocument();
  });

  it("keeps the newest project request active when an older open fails late", async () => {
    const olderOpen = deferred<{ id: string; name: string }>();
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "load_split_project_from_folder") {
        expect(input).toEqual({ projectDir: "/projects/slow-cut" });
        return olderOpen.promise;
      }
      if (command === "materialize_sample_project_media") return Promise.resolve(null);
      if (command === "save_split_project_to_folder") {
        return Promise.resolve(savedProjectResult(input));
      }
      return Promise.resolve(settingsCommandResult(command));
    });

    renderConnectedApp();
    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/slow-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));

    expect(await screen.findByText("Project id project-sample")).toBeVisible();

    await act(async () => {
      olderOpen.reject(new Error("slow project failed after sample opened"));
      await olderOpen.promise.catch(() => undefined);
    });

    expect(screen.getByText("Project id project-sample")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/next-cut" },
    });
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open project folder" })).toBeEnabled(),
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps the newest project request busy when an older open succeeds first", async () => {
    const olderOpen = deferred<{ id: string; name: string }>();
    const newerSample = deferred<null>();
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "load_split_project_from_folder") return olderOpen.promise;
      if (command === "materialize_sample_project_media") return newerSample.promise;
      if (command === "save_split_project_to_folder") {
        return Promise.resolve(savedProjectResult(input));
      }
      return Promise.resolve(settingsCommandResult(command));
    });

    renderConnectedApp();
    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/slow-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    fireEvent.click(screen.getByRole("button", { name: "Open sample" }));

    await act(async () => {
      olderOpen.resolve({ id: "obsolete-project", name: "Obsolete project" });
      await olderOpen.promise;
    });

    expect(screen.queryByRole("region", { name: "Mock editor workspace" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open project folder" })).toBeDisabled();

    await act(async () => {
      newerSample.resolve(null);
      await newerSample.promise;
    });
    expect(await screen.findByText("Project id project-sample")).toBeVisible();
  });

  it("keeps invalid project folders on home with recovery copy", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "load_split_project_from_folder") {
        return Promise.reject(new Error("Create video-creater.project.json before opening."));
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/missing-manifest" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Project folder is missing its manifest",
    );
    expect(screen.getByText(/Choose a split project folder/)).toBeInTheDocument();
    expect(screen.getByText(/Create video-creater.project.json/)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Mock editor workspace" })).not.toBeInTheDocument();
  });

  it("loads persisted recent split projects on the home surface", async () => {
    window.localStorage.setItem(
      "video-creater.recentProjects",
      JSON.stringify([
        {
          id: "recent:/projects/saved-cut",
          name: "saved-cut",
          projectDir: "/projects/saved-cut",
          updatedAtLabel: "Opened recently",
          statusLabel: "Local project",
        },
      ]),
    );
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();

    expect(await screen.findByText("saved-cut")).toBeInTheDocument();
    expect(screen.getByText("/projects/saved-cut")).toBeInTheDocument();
  });

  it("records an opened split project in the recent project list", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "load_split_project_from_folder") {
        return Promise.resolve({ id: "launch-cut-project", name: "Launch Cut" });
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/launch-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open project folder" }));
    fireEvent.click(await screen.findByRole("button", { name: "Home" }));

    expect(await screen.findByText("Launch Cut")).toBeInTheDocument();
    expect(screen.getByText("/projects/launch-cut")).toBeInTheDocument();
    expect(window.localStorage.getItem("video-creater.recentProjects")).toContain(
      "/projects/launch-cut",
    );
  });

  it("creates a new split project from the home surface", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "save_split_project_to_folder") {
        expect(input).toMatchObject({
          projectDir: "/projects/new-cut",
          project: {
            schemaVersion: 2,
            name: "new-cut",
            media: [],
            generatedAssets: [],
            renderReports: [],
            transcripts: [],
            jobs: [],
          },
        });
        return Promise.resolve(savedProjectResult(input));
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/new-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));

    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeInTheDocument();
    expect(screen.getByText("Project dir /projects/new-cut")).toBeInTheDocument();
    expect(screen.getByText(/Project id project-/)).toBeInTheDocument();
  });

  it("copies native project defaults into only the new project", () => {
    const defaults = {
      width: 3840,
      height: 2160,
      fps: 24,
      loudnessLufs: -16,
      captions: "mux" as const,
    };

    const project = createEmptySplitProject("/tmp/new", defaults);

    expect(project.renderSettings).toEqual(defaults);
    expect(project.renderSettings).not.toBe(defaults);
  });

  it("copies accepted native project defaults into only the new project", async () => {
    appPreferencesMock.mockResolvedValueOnce(
      appPreferences({
        newProjectDefaults: {
          width: 3840,
          height: 2160,
          fps: 24,
          loudnessLufs: -16,
          captions: "mux",
        },
      }),
    );
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "save_split_project_to_folder") {
        expect(input).toMatchObject({
          projectDir: "/projects/native-defaults",
          project: {
            renderSettings: {
              width: 3840,
              height: 2160,
              fps: 24,
              loudnessLufs: -16,
              captions: "mux",
            },
          },
        });
        return Promise.resolve(savedProjectResult(input));
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(
        command === "get_transcription_runtime_status" ? "unavailable" : null,
      );
    });

    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/native-defaults" },
    });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));

    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeInTheDocument();
  });

  it("uses the accepted native location only as a suggested project parent", async () => {
    appPreferencesMock.mockResolvedValueOnce(
      appPreferences({
        projectLocation: {
          mode: "suggestedParent",
          parentPath: "/projects/default-cuts",
        },
      }),
    );
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "save_split_project_to_folder") {
        expect(input).toMatchObject({
          projectDir: "/projects/default-cuts/new-cut",
          project: {
            schemaVersion: 2,
            name: "new-cut",
          },
        });
        return Promise.resolve(savedProjectResult(input));
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    expect(await screen.findByLabelText("Project folder path")).toHaveValue(
      "/projects/default-cuts",
    );
    fireEvent.click(screen.getByRole("button", { name: "New project" }));

    expect(invokeMock).not.toHaveBeenCalledWith(
      "save_split_project_to_folder",
      expect.anything(),
    );
    expect(screen.getByRole("button", { name: "Create project" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "/projects/default-cuts/new-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create project" }));

    expect(await screen.findByRole("region", { name: "Mock editor workspace" })).toBeInTheDocument();
    expect(screen.getByText("Project dir /projects/default-cuts/new-cut")).toBeInTheDocument();
  });

  it("uses Settings-accepted project preferences on Home without a restart", async () => {
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "update_app_preferences") {
        const patch = (input as { patch: Record<string, unknown> }).patch;
        return Promise.resolve(appPreferences(patch));
      }
      if (command === "save_split_project_to_folder") {
        expect(input).toMatchObject({
          projectDir: "/projects/accepted-parent/new-cut",
          project: {
            renderSettings: {
              width: 3840,
              height: 2160,
              fps: 24,
              loudnessLufs: -16,
              captions: "mux",
            },
          },
        });
        return Promise.resolve(savedProjectResult(input));
      }
      return Promise.resolve(settingsCommandResult(command));
    });

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    nativeMenuHandler.current?.({
      payload: { sequence: 40, command: "openSettings" },
    });

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Apply accepted project preferences",
      }),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith(
        "update_app_preferences",
        expect.anything(),
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    expect(await screen.findByLabelText("Project folder path")).toHaveValue(
      "/projects/accepted-parent",
    );
    fireEvent.change(screen.getByLabelText("Project folder path"), {
      target: { value: "/projects/accepted-parent/new-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));

    expect(
      await screen.findByRole("region", { name: "Mock editor workspace" }),
    ).toBeInTheDocument();
  });

  it("serializes overlapping Settings writes before adopting the newer App preference", async () => {
    const older = deferred<ReturnType<typeof appPreferences>>();
    const newer = deferred<ReturnType<typeof appPreferences>>();
    let updateCount = 0;
    invokeMock.mockImplementation((command: string) => {
      if (command === "update_app_preferences") {
        updateCount += 1;
        return updateCount === 1 ? older.promise : newer.promise;
      }
      return Promise.resolve(settingsCommandResult(command));
    });

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    nativeMenuHandler.current?.({
      payload: { sequence: 41, command: "openSettings" },
    });

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Apply older project location",
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Apply newer project location" }),
    );

    await waitFor(() => expect(updateCount).toBe(1));
    older.resolve(
      appPreferences({
        projectLocation: {
          mode: "suggestedParent",
          parentPath: "/projects/older-parent",
        },
      }),
    );
    await waitFor(() => expect(updateCount).toBe(2));
    await act(async () => {
      newer.resolve(
        appPreferences({
          projectLocation: {
            mode: "suggestedParent",
            parentPath: "/projects/newer-parent",
          },
        }),
      );
      await newer.promise;
    });
    await waitFor(() =>
      expect(screen.getByTestId("accepted-settings-project-location")).toHaveTextContent(
        "/projects/newer-parent",
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    expect(await screen.findByLabelText("Project folder path")).toHaveValue(
      "/projects/newer-parent",
    );
  });

  it("keeps preference serialization and nested rebase authority across Settings close and reopen", async () => {
    const first = deferred<ReturnType<typeof appPreferences>>();
    const second = deferred<ReturnType<typeof appPreferences>>();
    const updateInputs: unknown[] = [];
    invokeMock.mockImplementation((command: string, input?: unknown) => {
      if (command === "update_app_preferences") {
        updateInputs.push(input);
        return updateInputs.length === 1 ? first.promise : second.promise;
      }
      return Promise.resolve(settingsCommandResult(command));
    });

    renderConnectedApp();
    await waitFor(() => expect(nativeMenuHandler.current).not.toBeNull());
    nativeMenuHandler.current?.({ payload: { sequence: 42, command: "openSettings" } });
    fireEvent.click(await screen.findByRole("button", { name: "Apply project FPS" }));
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    nativeMenuHandler.current?.({ payload: { sequence: 43, command: "openSettings" } });
    fireEvent.click(await screen.findByRole("button", { name: "Apply project captions" }));

    await waitFor(() => expect(updateInputs).toHaveLength(1));
    first.resolve(
      appPreferences({
        newProjectDefaults: {
          ...appPreferences().newProjectDefaults,
          fps: 24,
        },
      }),
    );
    await waitFor(() => expect(updateInputs).toHaveLength(2));
    expect(updateInputs[1]).toEqual({
      patch: {
        newProjectDefaults: {
          ...appPreferences().newProjectDefaults,
          fps: 24,
          captions: "mux",
        },
      },
    });
    await act(async () => {
      second.resolve(
        appPreferences({
          newProjectDefaults: {
            ...appPreferences().newProjectDefaults,
            fps: 24,
            captions: "mux",
          },
        }),
      );
      await second.promise;
    });
    await waitFor(() =>
      expect(screen.getByTestId("accepted-settings-project-defaults")).toHaveTextContent(
        "24 mux",
      ),
    );
  });

  it("keeps failed new project creation on home with recovery copy", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "save_split_project_to_folder") {
        return Promise.reject(new Error("folder is not writable"));
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    fireEvent.change(await screen.findByLabelText("Project folder path"), {
      target: { value: "/projects/locked-cut" },
    });
    fireEvent.click(screen.getByRole("button", { name: "New project" }));

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(
      "Project folder is not writable",
    );
    expect(alert).toHaveTextContent("Check folder permissions");
    expect(alert).toHaveTextContent("folder is not writable");
    expect(screen.queryByRole("region", { name: "Mock editor workspace" })).not.toBeInTheDocument();
  });

  it("marks a failed recent project open with relink recovery", async () => {
    window.localStorage.setItem(
      "video-creater.recentProjects",
      JSON.stringify([
        {
          id: "recent:/projects/missing-cut",
          name: "missing-cut",
          projectDir: "/projects/missing-cut",
          updatedAtLabel: "Opened recently",
          statusLabel: "Local project",
        },
      ]),
    );
    invokeMock.mockImplementation((command: string) => {
      if (command === "load_split_project_from_folder") {
        return Promise.reject(new Error("project manifest is missing"));
      }
      if (command === "list_transcription_models") {
        return Promise.resolve([]);
      }
      if (command === "get_active_transcription_model") {
        return Promise.resolve(null);
      }
      return Promise.resolve(command === "get_transcription_runtime_status" ? "unavailable" : null);
    });

    renderConnectedApp();

    const card = await screen.findByLabelText("Recent project missing-cut");
    fireEvent.click(within(card).getByRole("button", { name: "Open project" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("project manifest is missing");
    expect(within(card).getByText("Needs relink")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Relink project" })).toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Relink project" }));
    expect(screen.getByLabelText("Project folder path")).toHaveValue("/projects/missing-cut");
  });

  it("removes a broken recent project from the home surface", async () => {
    window.localStorage.setItem(
      "video-creater.recentProjects",
      JSON.stringify([
        {
          id: "recent:/projects/missing-cut",
          name: "missing-cut",
          projectDir: "/projects/missing-cut",
          updatedAtLabel: "Opened recently",
          statusLabel: "Needs relink",
          warningLabel: "Project folder could not be opened.",
        },
      ]),
    );
    invokeMock.mockRejectedValue(
      new BackendUnavailableError(),
    );

    renderConnectedApp();

    const card = await screen.findByLabelText("Recent project missing-cut");
    fireEvent.click(within(card).getByRole("button", { name: "Remove from recents" }));

    expect(screen.queryByText("missing-cut")).not.toBeInTheDocument();
    expect(window.localStorage.getItem("video-creater.recentProjects")).not.toContain(
      "/projects/missing-cut",
    );
  });
});
