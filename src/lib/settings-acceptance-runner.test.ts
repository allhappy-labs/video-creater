import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ProjectRenderReport } from "./project";
import { createSampleProject } from "./sample-project";
import {
  createFinalSettingsAcceptanceReport,
  runSettingsAcceptancePhase,
  runSettingsAcceptanceIfEnabled,
  settingsAcceptanceFailureDiagnosticCode,
  settingsAcceptanceFailurePhase,
  type SettingsAcceptanceBridge,
} from "./settings-acceptance-runner";

const categories = [
  { id: "general", label: "General" },
  { id: "projects", label: "Projects" },
  { id: "aiModels", label: "AI & Models" },
  { id: "integrations", label: "Integrations" },
  { id: "storage", label: "Storage" },
  { id: "advanced", label: "Advanced" },
] as const;

function settingsDocument(): Document {
  document.body.innerHTML = `<main><nav role="tablist" aria-label="Settings categories" data-settings-acceptance-category-rail>${categories
    .map(
      ({ id, label }) =>
        `<button id="settings-tab-${id}" type="button" role="tab" data-settings-category-id="${id}" aria-selected="${id === "general"}"><span data-category-label>${label}</span></button>`,
    )
    .join("")}</nav><button type="button" role="combobox" aria-label="Enabled generation models" aria-expanded="false"></button><div role="menu" aria-label="Generation model choices"><input type="search" aria-label="Search generation models"><button type="button" role="menuitemcheckbox" aria-label="GPT Image 1, OpenAI">GPT Image 1</button><button type="button" aria-label="Configure OpenAI" data-settings-blocked-action="missing-openai-provider" data-settings-configure-target="integrations:openai">Configure</button></div><div data-testid="providers-settings-page" data-settings-credential-boundary="keychain-only"><form data-settings-target="integrations:openai" data-settings-credential-control data-settings-credential-storage="keychain" tabindex="-1"></form></div></main>`;
  const tabs = [...document.querySelectorAll<HTMLButtonElement>('[id^="settings-tab-"]')];
  for (const tab of tabs) {
    tab.addEventListener("click", () => {
      for (const candidate of tabs) {
        candidate.setAttribute("aria-selected", String(candidate === tab));
      }
    });
  }
  document.querySelector<HTMLButtonElement>('[aria-label="Configure OpenAI"]')
    ?.addEventListener("click", () => {
      const integrations = document.getElementById("settings-tab-integrations");
      for (const candidate of tabs) {
        candidate.setAttribute("aria-selected", String(candidate === integrations));
      }
      document.querySelector<HTMLElement>('[data-settings-target="integrations:openai"]')
        ?.focus();
    });
  const modelCombobox = document.querySelector<HTMLButtonElement>(
    '[aria-label="Enabled generation models"]',
  );
  modelCombobox?.addEventListener("click", () => {
    modelCombobox.setAttribute("aria-expanded", "true");
  });
  return document;
}

function acceptanceRenderReport(
  id = "settings-acceptance-render-artifact",
): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id,
    status: "completed",
    outputPath: `renders/${id}/output.mp4`,
    durationSeconds: 8,
    streams: { video: true, audio: true },
    checks: { artifactPaths: "passed", duration: "passed", streams: "passed" },
    artifacts: [`renders/${id}/report.json`],
    previewComparisonRequest: null,
    previewComparison: null,
    logPath: `renders/${id}/render.log`,
    createdAt: "2026-07-12T00:00:00Z",
  };
}

function sampleProjectWithAcceptanceReport() {
  return {
    ...createSampleProject(),
    renderReports: [acceptanceRenderReport()],
  };
}

describe("packaged settings acceptance runner", () => {
  it("preserves the exact typed phase for every post-restart failure path", async () => {
    for (const phase of [
      "interruptedRecovery",
      "modelReady",
      "speechSeparation",
      "storageCleanup",
      "providerKeychain",
      "agentMcpSkills",
    ] as const) {
      const error = await runSettingsAcceptancePhase(phase, async () => {
        throw new Error(`Bearer acceptance-canary-${phase}`);
      }).catch((caught: unknown) => caught);
      expect(settingsAcceptanceFailurePhase(error)).toBe(phase);
    }
  });

  it("records a bounded diagnostic for an empty disposable app cache cleanup preview", async () => {
    const error = await runSettingsAcceptancePhase("storageCleanup", async () => {
      throw new Error("disposable app cache fixture was not isolated for cleanup");
    }).catch((caught: unknown) => caught);

    expect(settingsAcceptanceFailureDiagnosticCode(error)).toBe(
      "settings.acceptance.storageCleanup.disposableCache",
    );
  });

  it("records the allowlisted native storage-cleanup error code", async () => {
    const error = await runSettingsAcceptancePhase("storageCleanup", async () => {
      throw new Error(
        "Settings operation failed during acceptance (settings.storage.cleanupConfirmationMismatch)",
      );
    }).catch((caught: unknown) => caught);

    expect(settingsAcceptanceFailureDiagnosticCode(error)).toBe(
      "settings.acceptance.storageCleanup.confirmationMismatch",
    );
  });

  it("records a bounded storage-cleanup command boundary", async () => {
    const error = await runSettingsAcceptancePhase("storageCleanup", async () => {
      throw new Error("Settings storage cleanup start command was rejected");
    }).catch((caught: unknown) => caught);

    expect(settingsAcceptanceFailureDiagnosticCode(error)).toBe(
      "settings.acceptance.storageCleanup.startCommand",
    );
  });

  it("identifies a rejected storage-cleanup health command", async () => {
    const error = await runSettingsAcceptancePhase("storageCleanup", async () => {
      throw new Error("Settings storage cleanup health command was rejected");
    }).catch((caught: unknown) => caught);

    expect(settingsAcceptanceFailureDiagnosticCode(error)).toBe(
      "settings.acceptance.storageCleanup.healthCommand",
    );
  });

  beforeEach(() => {
    document.body.innerHTML = "";
  });

  it("is inert on a normal production launch", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const emit = vi.fn();

    await runSettingsAcceptanceIfEnabled({
      invoke,
      emit,
      document,
      sleep: async () => undefined,
    });

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("get_settings_acceptance_context");
    expect(emit).not.toHaveBeenCalled();
  });

  it("waits for the real Settings shell when native startup is slower than ten seconds", async () => {
    let sleeps = 0;
    const invoke = vi.fn(async (command: string) => {
      if (command === "get_settings_acceptance_context") {
        return {
          stage: "pre_restart",
          projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
        };
      }
      if (command === "write_settings_acceptance_failure") return undefined;
      throw new Error(`advanced beyond delayed Settings shell: ${command}`);
    });

    const emit = vi.fn(async () => undefined);
    await expect(runSettingsAcceptanceIfEnabled({
      invoke,
      emit,
      document,
      sleep: async () => {
        sleeps += 1;
        if (sleeps === 101) settingsDocument();
      },
    })).rejects.toThrow("advanced beyond delayed Settings shell: list_transcription_models");
    expect(invoke).toHaveBeenCalledWith("write_settings_acceptance_failure", {
      failure: {
        stage: "pre_restart",
        phase: "modelCancel",
        diagnosticCode: "settings.acceptance.modelCancel.failed",
      },
    });
    expect(emit).toHaveBeenCalledTimes(60);
  });

  it("rejects a stable Settings tab id attached to a non-button element", async () => {
    document.body.innerHTML = `<main><nav role="tablist" data-settings-acceptance-category-rail>${categories
      .map(({ id, label }) =>
        id === "general"
          ? `<div role="tab" data-settings-category-id="${id}" id="settings-tab-${id}">${label}</div>`
          : `<button role="tab" type="button" data-settings-category-id="${id}" id="settings-tab-${id}">${label}</button>`,
      )
      .join("")}</nav></main>`;
    const invoke = vi.fn().mockResolvedValue({
      stage: "pre_restart",
      projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
    });

    await expect(
      runSettingsAcceptanceIfEnabled({
        invoke,
        emit: vi.fn(async () => undefined),
        document,
        sleep: async () => undefined,
      }),
    ).rejects.toThrow(
      "Settings category tab is invalid: General (settings-tab-general)",
    );
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke).toHaveBeenLastCalledWith("write_settings_acceptance_failure", {
      failure: {
        stage: "pre_restart",
        phase: "settingsDom",
        diagnosticCode: "settings.acceptance.settingsDom.categoryTabs",
      },
    });
  });

  it("records bounded diagnostic codes for Settings DOM contract failures", async () => {
    const cases: Array<[
      string,
      (root: Document) => void,
      RegExp,
      string,
    ]> = [
      ["extra category", (root) => {
        root.querySelector("[data-settings-acceptance-category-rail]")?.insertAdjacentHTML(
          "beforeend",
          '<button role="tab" type="button" data-settings-category-id="experimental" id="settings-tab-experimental">Experimental</button>',
        );
      }, /category rail.*exact/i, "settings.acceptance.settingsDom.categoryTabs"],
      ["health badge", (root) => {
        root.querySelector("[data-settings-acceptance-category-rail]")?.insertAdjacentHTML(
          "beforeend",
          '<span data-settings-health-badge>1</span>',
        );
      }, /health badge/i, "settings.acceptance.settingsDom.categoryTabs"],
      ["environment credential", (root) => {
        root.querySelector('[data-testid="providers-settings-page"]')?.insertAdjacentHTML(
          "beforeend",
          '<input data-settings-credential-control data-settings-credential-storage="environment">',
        );
      }, /environment credential/i, "settings.acceptance.settingsDom.integrations"],
      ["unresolved blocked action", (root) => {
        root.querySelector('[role="menu"]')?.insertAdjacentHTML(
          "beforeend",
          '<button type="button" data-settings-blocked-action="missing-ghost-provider" data-settings-configure-target="integrations:ghost">Configure Ghost</button>',
        );
      }, /blocked Settings action.*resolve/i, "settings.acceptance.settingsDom.blockedActions"],
    ];

    for (const [, mutate, expected, diagnosticCode] of cases) {
      const fixture = settingsDocument();
      mutate(fixture);
      const invoke = vi.fn(async (command: string) => {
        if (command === "get_settings_acceptance_context") {
          return {
            stage: "pre_restart",
            projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
          };
        }
        if (command === "write_settings_acceptance_failure") return undefined;
        throw new Error(`advanced beyond Settings DOM: ${command}`);
      });
      await expect(runSettingsAcceptanceIfEnabled({
        invoke,
        emit: vi.fn(async () => undefined),
        document: fixture,
        sleep: async () => undefined,
      })).rejects.toThrow(expected);
      expect(invoke).toHaveBeenCalledWith("write_settings_acceptance_failure", {
        failure: {
          stage: "pre_restart",
          phase: "settingsDom",
          diagnosticCode,
        },
      });
    }
  });

  it("records a bounded code when the model search has no GPT Image result", async () => {
    const fixture = settingsDocument();
    fixture.querySelector('[role="menuitemcheckbox"]')?.remove();
    const invoke = vi.fn(async (command: string) => {
      if (command === "get_settings_acceptance_context") {
        return {
          stage: "pre_restart",
          projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
        };
      }
      if (command === "write_settings_acceptance_failure") return undefined;
      throw new Error(`advanced beyond Settings DOM: ${command}`);
    });

    await expect(runSettingsAcceptanceIfEnabled({
      invoke,
      emit: vi.fn(async () => undefined),
      document: fixture,
      sleep: async () => undefined,
    })).rejects.toThrow("generation model filter did not retain the expected GPT Image result");
    expect(invoke).toHaveBeenCalledWith("write_settings_acceptance_failure", {
      failure: {
        stage: "pre_restart",
        phase: "settingsDom",
        diagnosticCode: "settings.acceptance.settingsDom.modelResults",
      },
    });
  });

  it("recognizes a hyphenated GPT Image label in the live model menu", async () => {
    const fixture = settingsDocument();
    fixture.querySelector('[role="menuitemcheckbox"]')?.setAttribute(
      "aria-label",
      "OpenAI GPT-image-2",
    );
    const invoke = vi.fn(async (command: string) => {
      if (command === "get_settings_acceptance_context") {
        return {
          stage: "pre_restart",
          projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
        };
      }
      if (command === "write_settings_acceptance_failure") return undefined;
      throw new Error(`advanced beyond Settings DOM: ${command}`);
    });

    await expect(runSettingsAcceptanceIfEnabled({
      invoke,
      emit: vi.fn(async () => undefined),
      document: fixture,
      sleep: async () => undefined,
    })).rejects.toThrow("advanced beyond Settings DOM: list_transcription_models");
  });

  it("reopens the live model menu before resolving a second blocked provider action", async () => {
    const fixture = settingsDocument();
    const initialMenu = fixture.querySelector('[role="menu"]');
    initialMenu?.insertAdjacentHTML(
      "beforeend",
      '<button type="button" data-settings-blocked-action="missing-google-provider" data-settings-configure-target="integrations:google">Configure Google</button>',
    );
    fixture.querySelector('[data-testid="providers-settings-page"]')?.insertAdjacentHTML(
      "beforeend",
      '<form data-settings-target="integrations:google" tabindex="-1"></form>',
    );
    let openAiClicked = false;
    let googleClicked = false;
    fixture.querySelector('[data-settings-blocked-action="missing-openai-provider"]')
      ?.addEventListener("click", () => {
        openAiClicked = true;
        initialMenu?.remove();
      });
    fixture.getElementById("settings-tab-aiModels")?.addEventListener("click", () => {
      if (fixture.querySelector('[role="menu"]')) return;
      fixture.body.insertAdjacentHTML(
        "beforeend",
        '<div role="menu" aria-label="Generation model choices"><button type="button" data-settings-blocked-action="missing-google-provider" data-settings-configure-target="integrations:google">Configure Google</button></div>',
      );
      fixture.querySelector('[data-settings-blocked-action="missing-google-provider"]')
        ?.addEventListener("click", () => {
          googleClicked = true;
          const integrations = fixture.getElementById("settings-tab-integrations");
          for (const tab of fixture.querySelectorAll('[role="tab"]')) {
            tab.setAttribute("aria-selected", String(tab === integrations));
          }
          fixture.querySelector<HTMLElement>('[data-settings-target="integrations:google"]')
            ?.focus();
        });
    });
    const invoke = vi.fn(async (command: string) => {
      if (command === "get_settings_acceptance_context") {
        return {
          stage: "pre_restart",
          projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
        };
      }
      if (command === "write_settings_acceptance_failure") return undefined;
      throw new Error(`advanced beyond Settings DOM: ${command}`);
    });

    await expect(runSettingsAcceptanceIfEnabled({
      invoke,
      emit: vi.fn(async () => undefined),
      document: fixture,
      sleep: async () => undefined,
    })).rejects.toThrow("advanced beyond Settings DOM");
    expect({ openAiClicked, googleClicked }).toEqual({
      openAiClicked: true,
      googleClicked: true,
    });
  });

  it("uses a typed abort fallback when failure persistence rejects echoed secrets", async () => {
    document.body.innerHTML = `<main><nav role="tablist" data-settings-acceptance-category-rail>${categories
      .map(({ id, label }) =>
        id === "general"
          ? `<div role="tab" data-settings-category-id="${id}" id="settings-tab-${id}">${label}</div>`
          : `<button role="tab" type="button" data-settings-category-id="${id}" id="settings-tab-${id}">${label}</button>`,
      )
      .join("")}</nav></main>`;
    const invoke = vi.fn(async (command: string) => {
      if (command === "get_settings_acceptance_context") {
        return {
          stage: "pre_restart",
          projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
        };
      }
      if (command === "write_settings_acceptance_failure") {
        throw new Error("Bearer sk-echoed-canary: acceptance-token-value");
      }
      if (command === "abort_settings_acceptance_run") return undefined;
      throw new Error(`unexpected command ${command}`);
    });

    await expect(runSettingsAcceptanceIfEnabled({
      invoke,
      emit: vi.fn(async () => undefined),
      document,
      sleep: async () => undefined,
    })).rejects.toThrow("Settings category tab is invalid");
    expect(invoke).toHaveBeenLastCalledWith("abort_settings_acceptance_run", {
      failure: {
        stage: "pre_restart",
        phase: "settingsDom",
        diagnosticCode: "settings.acceptance.settingsDom.categoryTabs",
      },
    });
    expect(JSON.stringify(invoke.mock.calls)).not.toContain("sk-echoed-canary");
  });

  it("opens and traverses all six real categories before observing and cancelling a model download", async () => {
    const operations = [
      [modelOperation("download-1", "running", "downloading", 1)],
      [modelOperation("download-1", "cancelled", "cancelled", 1)],
      [modelOperation("download-2", "running", "downloading", 2)],
    ];
    let downloadCount = 0;
    const invoke = vi.fn(async (command: string, _args?: unknown) => {
      switch (command) {
        case "get_settings_acceptance_context":
          return {
            stage: "pre_restart",
            projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
          };
        case "list_transcription_models":
          return [{
            modelId: "nvidia/parakeet-tdt-0.6b-v3",
            installStatus: "missing",
            runtimeId: "fluid_audio_coreml",
          }];
        case "download_transcription_model":
          downloadCount += 1;
          return modelOperation(`download-${downloadCount}`, "queued", "queued", 0);
        case "list_settings_operations":
          return operations.shift() ?? [
            modelOperation("download-2", "running", "downloading", 2),
          ];
        case "cancel_model_download":
          return { modelId: "nvidia/parakeet-tdt-0.6b-v3", installStatus: "missing" };
        case "save_split_project_to_folder":
          return { manifestPath: "/private/tmp/acceptance-project/project.json" };
        case "write_settings_acceptance_checkpoint":
          return undefined;
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });
    const emit = vi.fn(async () => undefined);
    const clicked: string[] = [];
    for (const button of settingsDocument().querySelectorAll('[id^="settings-tab-"]')) {
      button.addEventListener("click", () => {
        clicked.push(button.querySelector("[data-category-label]")?.textContent ?? "");
      });
    }
    const bridge: SettingsAcceptanceBridge = {
      invoke,
      emit,
      document,
      sleep: async () => undefined,
    };

    await runSettingsAcceptanceIfEnabled(bridge);

    expect(emit).toHaveBeenCalledWith("video-creater://native-menu-command", {
      sequence: 1,
      command: "openSettings",
    });
    expect(clicked).toEqual(categories.map(({ label }) => label));
    expect(invoke).toHaveBeenCalledWith("download_transcription_model", {
      modelId: "nvidia/parakeet-tdt-0.6b-v3",
    });
    expect(invoke).toHaveBeenCalledWith("cancel_model_download", {
      modelId: "nvidia/parakeet-tdt-0.6b-v3",
    });
    expect(invoke).toHaveBeenCalledWith("save_split_project_to_folder", {
      projectDir: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
      project: sampleProjectWithAcceptanceReport(),
      expectedRevision: 0,
    });
    expect(invoke).toHaveBeenCalledTimes(10);
    expect(
      invoke.mock.calls.filter(([command]) => command === "download_transcription_model"),
    ).toHaveLength(2);
    expect(invoke).toHaveBeenLastCalledWith("write_settings_acceptance_checkpoint", {
      checkpoint: {
        stage: "pre_restart",
        checks: [
          {
            id: "settingsDom",
            status: "passed",
            diagnosticCode: "settings.acceptance.settingsDom.passed",
          },
          {
            id: "modelCancel",
            status: "passed",
            diagnosticCode: "settings.acceptance.modelCancel.passed",
          },
        ],
      },
    });
    expect(new Set(invoke.mock.calls.map(([command]) => command))).toEqual(
      new Set([
        "get_settings_acceptance_context",
        "list_transcription_models",
        "download_transcription_model",
        "list_settings_operations",
        "cancel_model_download",
        "save_split_project_to_folder",
        "write_settings_acceptance_checkpoint",
      ]),
    );
  });

  it("proves interrupted recovery then retries the pinned model to verified ready after restart", async () => {
    const operationResponses = [
      [interruptedModelOperation("download-2")],
      [modelOperation("download-3", "succeeded", "ready", 18)],
      [storageOperation("storage-1", "succeeded", "completed", 1)],
      [storageOperation("storage-2", "succeeded", "completed", 1)],
    ];
    let skillPreviewed = false;
    let storageCleanupCount = 0;
    let activeProjectSessionRegistered = false;
    const progressSteps: string[] = [];
    const reloadedProject = {
      ...sampleProjectWithAcceptanceReport(),
      renderReports: [
        acceptanceRenderReport(),
        acceptanceRenderReport("preexisting-render-artifact"),
      ],
    };
    const expectedReloadedArtifactIds = reloadedProject.renderReports.map(({ id }) => id);
    const invoke = vi.fn(async (command: string, args?: unknown) => {
      switch (command) {
        case "get_settings_acceptance_context":
          return {
            stage: "post_restart",
            projectRoot: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
          };
        case "list_settings_operations":
          return operationResponses.shift() ?? [];
        case "download_transcription_model":
          return modelOperation("download-3", "queued", "queued", 0);
        case "verify_transcription_model":
          return {
            modelId: "nvidia/parakeet-tdt-0.6b-v3",
            installStatus: "ready",
            runtimeId: "fluid_audio_coreml",
          };
        case "get_production_speech_model_status":
          return speechModelStatus();
        case "load_settings_acceptance_project":
          activeProjectSessionRegistered = true;
          return {
            id: reloadedProject.id,
            renderArtifactIds: reloadedProject.renderReports.map((report) => report.id),
          };
        case "get_storage_health":
          return { state: "ready", components: [] };
        case "preview_storage_cleanup":
          if (!activeProjectSessionRegistered) {
            throw new Error("project render cleanup preview requires an active native project session");
          }
          if (
            (args as { target?: { kind?: string } } | undefined)?.target?.kind ===
              "projectRenderArtifacts" &&
            JSON.stringify(
              (args as { target?: { artifactIds?: string[] } }).target?.artifactIds,
            ) !== JSON.stringify(expectedReloadedArtifactIds)
          ) {
            throw new Error("cleanup preview artifact IDs must come from the reloaded project");
          }
          return {
            items: [{ path: "disposable-cache/fixture", bytes: 64 }],
            totalBytes: 64,
            confirmationToken: "cleanup-confirmation",
          };
        case "run_storage_cleanup":
          storageCleanupCount += 1;
          return storageOperation(`storage-${storageCleanupCount}`, "queued", "queued", 0);
        case "set_provider_credential":
          return {
            provider: "openai",
            configured: true,
            source: "keychain",
          };
        case "list_provider_credential_statuses":
          return [{ provider: "openai", configured: true, source: "keychain" }];
        case "delete_provider_credential":
          return { provider: "openai", configured: false, source: "missing" };
        case "get_agent_settings_health":
          return { state: "ready", components: [] };
        case "mcp_client_configuration":
          return { executable: "video-creater-mcp-server", configuration: "configured" };
        case "get_skills_settings_health":
          return { state: "ready", components: [] };
        case "get_system_health_snapshot":
          return systemHealthSnapshot();
        case "repair_bundled_skills":
          if (!skillPreviewed) {
            skillPreviewed = true;
            return {
              preview: {
                affectedPaths: [".agents/skills/video-creater-visuals/SKILL.md"],
              },
              operation: null,
              report: null,
            };
          }
          return {
            preview: { affectedPaths: [] },
            operation: { id: "skill-1", state: "succeeded", completedUnits: 1 },
            report: { repairedPaths: [".agents/skills/video-creater-visuals/SKILL.md"] },
          };
        case "write_settings_acceptance_checkpoint":
          return undefined;
        case "write_settings_acceptance_progress":
          progressSteps.push(
            (args as { progress?: { step?: string } } | undefined)?.progress?.step ?? "",
          );
          return undefined;
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });
    const bridge: SettingsAcceptanceBridge = {
      invoke,
      emit: vi.fn(async () => undefined),
      document: settingsDocument(),
      sleep: async () => undefined,
    };

    await runSettingsAcceptanceIfEnabled(bridge);

    expect(invoke).toHaveBeenCalledWith("verify_transcription_model", {
      modelId: "nvidia/parakeet-tdt-0.6b-v3",
    });
    expect(invoke).toHaveBeenCalledWith("load_settings_acceptance_project", {
      projectDir: "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
    });
    const loadIndex = invoke.mock.calls.findIndex(
      ([command]) => command === "load_settings_acceptance_project",
    );
    const projectPreviewIndex = invoke.mock.calls.findIndex(
      ([command, args]) =>
        command === "preview_storage_cleanup" &&
        (args as { target?: { kind?: string } } | undefined)?.target?.kind ===
          "projectRenderArtifacts",
    );
    expect(loadIndex).toBeGreaterThanOrEqual(0);
    expect(projectPreviewIndex).toBeGreaterThan(loadIndex);
    expect(invoke).toHaveBeenLastCalledWith("write_settings_acceptance_checkpoint", {
      checkpoint: {
        stage: "post_restart",
        checks: [
          {
            id: "settingsDom",
            status: "passed",
            diagnosticCode: "settings.acceptance.settingsDom.passed",
          },
          {
            id: "interruptedRecovery",
            status: "passed",
            diagnosticCode: "settings.acceptance.interruptedRecovery.passed",
          },
          {
            id: "modelReady",
            status: "passed",
            diagnosticCode: "settings.acceptance.modelReady.passed",
          },
          {
            id: "speechSeparation",
            status: "passed",
            diagnosticCode: "settings.acceptance.speechSeparation.passed",
          },
          {
            id: "storageCleanup",
            status: "passed",
            diagnosticCode: "settings.acceptance.storageCleanup.passed",
          },
          {
            id: "providerKeychain",
            status: "passed",
            diagnosticCode: "settings.acceptance.providerKeychain.passed",
          },
          {
            id: "agentMcpSkills",
            status: "passed",
            diagnosticCode: "settings.acceptance.agentMcpSkills.passed",
          },
        ],
        productContract: {
          appSettings: {
            categories: [
              "general",
              "projects",
              "aiModels",
              "integrations",
              "storage",
              "advanced",
            ],
            sidebarHealthBadges: 0,
          },
          providers: { environmentCredentialControls: 0 },
          models: {
            selectionControl: "multiselect-combobox",
            comboboxOpen: true,
            filterQuery: "gpt image",
            filteredResultCount: 1,
          },
          blockedActions: [
            {
              id: "missing-openai-provider",
              configureTarget: "integrations:openai",
              observedTarget: "integrations:openai",
              configureTargetResolved: true,
            },
          ],
          systemHealth: { optionalProviderCountedAsRequired: false },
        },
      },
    });
    expect(invoke).toHaveBeenCalledWith("run_storage_cleanup", {
      target: { kind: "disposableAppCache" },
      confirmationToken: "cleanup-confirmation",
      activeProjectDir: null,
    });
    expect(invoke).toHaveBeenCalledWith("preview_storage_cleanup", {
      target: {
        kind: "projectRenderArtifacts",
        artifactIds: expectedReloadedArtifactIds,
      },
      activeProjectDir:
        "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
    });
    expect(invoke).toHaveBeenCalledWith("run_storage_cleanup", {
      target: {
        kind: "projectRenderArtifacts",
        artifactIds: expectedReloadedArtifactIds,
      },
      confirmationToken: "cleanup-confirmation",
      activeProjectDir:
        "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
    });
    expect(invoke).toHaveBeenCalledWith("set_provider_credential", {
      provider: "openai",
      credential:
        "video-creater-acceptance-canary-video-creater-settings-acceptance-test-first",
    });
    expect(invoke).toHaveBeenCalledWith("set_provider_credential", {
      provider: "openai",
      credential:
        "video-creater-acceptance-canary-video-creater-settings-acceptance-test-replacement",
    });
    expect(invoke).toHaveBeenCalledWith("delete_provider_credential", {
      provider: "openai",
    });
    expect(invoke).toHaveBeenCalledWith("repair_bundled_skills", {
      projectRoot:
        "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
      skillIds: [
        "video-creater-video-pipeline",
        "video-creater-graphics",
        "video-creater-visuals",
      ],
      confirmedPaths: [".agents/skills/video-creater-visuals/SKILL.md"],
    });
    expect(progressSteps).toEqual([
      "settingsDomSettled",
      "interruptedRecoveryConfirmed",
      "modelDownloadRequested",
      "modelDownloadSettled",
      "modelVerificationRequested",
      "modelVerificationSettled",
      "speechSeparationConfirmed",
      "storageProjectLoadRequested",
      "storageCleanupSettled",
      "providerKeychainSettled",
      "agentMcpSkillsSettled",
    ]);
  });

  it("always deletes the isolated provider canary when a provider response fails validation", async () => {
    const operationResponses = [
      [interruptedModelOperation("download-2")],
      [modelOperation("download-3", "succeeded", "ready", 18)],
      [storageOperation("storage-1", "succeeded", "completed", 1)],
      [storageOperation("storage-1", "succeeded", "completed", 1)],
    ];
    const invoke = vi.fn(async (command: string) => {
      switch (command) {
        case "get_settings_acceptance_context":
          return {
            stage: "post_restart",
            projectRoot:
              "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
          };
        case "list_settings_operations":
          return operationResponses.shift() ?? [];
        case "download_transcription_model":
          return modelOperation("download-3", "queued", "queued", 0);
        case "verify_transcription_model":
          return {
            modelId: "nvidia/parakeet-tdt-0.6b-v3",
            installStatus: "ready",
            runtimeId: "fluid_audio_coreml",
          };
        case "get_production_speech_model_status":
          return speechModelStatus();
        case "load_settings_acceptance_project": {
          const project = sampleProjectWithAcceptanceReport();
          return {
            id: project.id,
            renderArtifactIds: project.renderReports.map((report) => report.id),
          };
        }
        case "get_storage_health":
          return { state: "ready", components: [] };
        case "preview_storage_cleanup":
          return {
            items: [{ path: "disposable-cache/fixture", bytes: 64 }],
            confirmationToken: "cleanup-confirmation",
          };
        case "run_storage_cleanup":
          return storageOperation("storage-1", "succeeded", "completed", 1);
        case "set_provider_credential":
          return {
            provider: "openai",
            configured: true,
            source: "keychain",
            leaked:
              "video-creater-acceptance-canary-video-creater-settings-acceptance-test-first",
          };
        case "delete_provider_credential":
          return { provider: "openai", configured: false, source: "missing" };
        case "write_settings_acceptance_progress":
          return undefined;
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });

    await expect(
      runSettingsAcceptanceIfEnabled({
        invoke,
        emit: vi.fn(async () => undefined),
        document: settingsDocument(),
        sleep: async () => undefined,
      }),
    ).rejects.toThrow("provider credential secret appeared in an IPC response");

    expect(invoke).toHaveBeenCalledWith("delete_provider_credential", {
      provider: "openai",
    });
    expect(invoke).toHaveBeenCalledWith("write_settings_acceptance_failure", {
      failure: {
        stage: "post_restart",
        phase: "providerKeychain",
        diagnosticCode: "settings.acceptance.providerKeychain.failed",
      },
    });
  });

  it("rejects wrong and incomplete production speech status instead of conflating it with transcription readiness", async () => {
    for (const speechStatus of [
      { ...speechModelStatus(), modelSetId: "wrong-model-set" },
      { ...speechModelStatus(), runtimeId: undefined },
    ]) {
      const operationResponses = [
        [interruptedModelOperation("download-2")],
        [modelOperation("download-3", "succeeded", "ready", 18)],
      ];
      const invoke = vi.fn(async (command: string) => {
        switch (command) {
          case "get_settings_acceptance_context":
            return {
              stage: "post_restart",
              projectRoot:
                "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
            };
          case "list_settings_operations":
            return operationResponses.shift() ?? [];
          case "download_transcription_model":
            return modelOperation("download-3", "queued", "queued", 0);
          case "verify_transcription_model":
            return {
              modelId: "nvidia/parakeet-tdt-0.6b-v3",
              installStatus: "ready",
              runtimeId: "fluid_audio_coreml",
            };
          case "get_production_speech_model_status":
            return speechStatus;
          case "write_settings_acceptance_progress":
            return undefined;
          default:
            throw new Error(`unexpected command ${command}`);
        }
      });

      await expect(
        runSettingsAcceptanceIfEnabled({
          invoke,
          emit: vi.fn(async () => undefined),
          document: settingsDocument(),
          sleep: async () => undefined,
        }),
      ).rejects.toThrow("production speech model status is invalid");
      expect(invoke).toHaveBeenCalledWith("write_settings_acceptance_failure", {
        failure: {
          stage: "post_restart",
          phase: "speechSeparation",
          diagnosticCode: "settings.acceptance.speechSeparation.failed",
        },
      });
    }
  });

  it("rejects empty and unrelated operation journals as interruption evidence", async () => {
    for (const operations of [
      [],
      [{
        ...interruptedModelOperation("unrelated-download"),
        targetId: "other/model",
      }],
    ]) {
      const invoke = vi.fn(async (command: string) => {
        if (command === "get_settings_acceptance_context") {
          return {
            stage: "post_restart",
            projectRoot:
              "/private/tmp/video-creater-settings-acceptance-test/projects/acceptance-project",
          };
        }
        if (command === "list_settings_operations") {
          return operations;
        }
        if (command === "write_settings_acceptance_progress") {
          return undefined;
        }
        throw new Error(`unexpected command ${command}`);
      });

      await expect(
        runSettingsAcceptanceIfEnabled({
          invoke,
          emit: vi.fn(async () => undefined),
          document: settingsDocument(),
          sleep: async () => undefined,
        }),
      ).rejects.toThrow("identified Parakeet download interruption was not recovered");
      expect(invoke).toHaveBeenCalledWith("write_settings_acceptance_failure", {
        failure: {
          stage: "post_restart",
          phase: "interruptedRecovery",
          diagnosticCode: "settings.acceptance.interruptedRecovery.failed",
        },
      });
    }
  });

  it("reports the final Settings product contract from observed acceptance evidence", () => {
    const report = createFinalSettingsAcceptanceReport({
      categories: categories.map(({ id }) => id),
      sidebarHealthBadges: 0,
      environmentCredentialControls: 0,
      modelSelectionControl: "multiselect-combobox",
      modelComboboxOpen: true,
      modelFilterQuery: "gpt image",
      filteredModelResults: 1,
      blockedActions: [
        {
          id: "missing-openai-provider",
          configureTarget: "integrations:openai",
          observedTarget: "integrations:openai",
          configureTargetResolved: true,
        },
      ],
      systemHealthSections: [
        {
          id: "rendering",
          required: true,
          items: [{ id: "gstreamer-composition" }],
        },
        {
          id: "environment",
          required: false,
          items: [{ id: "provider-openai" }],
        },
      ],
    });

    expect(report.appSettings.categories).toEqual([
      "general",
      "projects",
      "aiModels",
      "integrations",
      "storage",
      "advanced",
    ]);
    expect(report.appSettings.sidebarHealthBadges).toBe(0);
    expect(report.providers.environmentCredentialControls).toBe(0);
    expect(report.models.selectionControl).toBe("multiselect-combobox");
    expect(report.models).toMatchObject({
      comboboxOpen: true,
      filterQuery: "gpt image",
      filteredResultCount: 1,
    });
    expect(
      report.blockedActions.every((action) => action.configureTargetResolved),
    ).toBe(true);
    expect(report.blockedActions).toContainEqual({
      id: "missing-openai-provider",
      configureTarget: "integrations:openai",
      observedTarget: "integrations:openai",
      configureTargetResolved: true,
    });
    expect(report.systemHealth.optionalProviderCountedAsRequired).toBe(false);
  });
});

function modelOperation(
  id: string,
  state: string,
  phase: string,
  completedUnits: number,
) {
  return {
    id,
    kind: "modelDownload",
    targetId: "nvidia/parakeet-tdt-0.6b-v3",
    state,
    phase,
    completedUnits,
    error: null,
  };
}

function interruptedModelOperation(id: string) {
  return {
    ...modelOperation(id, "failed", "interrupted", 2),
    error: { code: "settings.operation.interrupted" },
  };
}

function storageOperation(
  id: string,
  state: string,
  phase: string,
  completedUnits: number,
) {
  return {
    id,
    kind: "storageCleanup",
    targetId: "disposable-app-cache:fixture",
    state,
    phase,
    completedUnits,
    error: null,
  };
}

function speechModelStatus() {
  return {
    modelSetId: "silero-vad+wspk-vbx-v1",
    runtimeId: "fluid_audio_speech_analysis",
    ready: false,
    installedFiles: 0,
    totalFiles: 26,
    installedBytes: 0,
    totalBytes: 22_662_842,
    rootPath: "/models/speech-analysis/production-v1",
    vadRepo: "FluidInference/silero-vad-coreml",
    vadRevision: "b419383c55c110e2c9271fa6ee0ea83d03c70d96",
    diarizationRepo: "FluidInference/speaker-diarization-coreml",
    diarizationRevision: "1ed7a662fdc7109e36d822db793ee6eebdaf8594",
    artifactFormat: "compiled_core_ml_bundles",
    licenses: ["MIT", "CC-BY-4.0"],
    lastErrorCode: null,
  };
}

function systemHealthSnapshot() {
  return {
    generatedAt: "2026-07-20T00:00:00Z",
    overall: "ready",
    sections: {
      rendering: {
        id: "rendering",
        required: true,
        items: [{ id: "gstreamer-composition" }],
      },
      environment: {
        id: "environment",
        required: false,
        items: [{ id: "provider-openai" }],
      },
    },
  };
}
