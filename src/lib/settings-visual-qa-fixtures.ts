/// <reference types="vite/client" />

import type { AppPreferences } from "./app-settings";
import type { ProviderCredentialStatus } from "./provider-credentials";
import type { VideoProject } from "./project";
import { sampleProjectDir } from "./sample-project";
import type { McpClientConfigurationState } from "./settings/agent";
import type {
  SettingsCategoryHealth,
  SettingsComponentHealth,
  SettingsHealthSnapshot,
  SystemHealthComponent,
  SystemHealthSection,
  SystemHealthSnapshot,
} from "./settings/health";
import type { SettingsOperation } from "./settings/operations";
import type { ProviderHealth } from "./settings/providers";
import type {
  NotificationCapability,
  RenderSystemHealth,
  UpdateHealth,
} from "./settings/render-system";
import type { StorageCleanupPreview } from "./settings/storage";
import type {
  ProductionSpeechModelStatus,
  RuntimeSelection,
  TranscriptionModelStatus,
} from "./transcription-models";
import type { FixtureOperationHandler } from "./runtime/adapters/fixture-transport";

export const settingsVisualQaFixtureIds = [
  "settings-general-no-project",
  "settings-projects-no-project",
  "settings-ai-models-no-project",
  "settings-integrations-no-project",
  "settings-storage-no-project",
  "settings-advanced-no-project",
  "project-settings-render-cleanup",
  "system-health-project-failed",
] as const;

export type SettingsVisualQaFixtureId =
  (typeof settingsVisualQaFixtureIds)[number];

export type SettingsVisualQaCategory =
  | "general"
  | "projects"
  | "aiModels"
  | "integrations"
  | "storage"
  | "advanced";

export type SettingsVisualQaSurface =
  | "appSettings"
  | "projectSettings"
  | "systemHealth";

export type SettingsVisualQaCaptureState =
  | "generationModelSelectorOpen"
  | "renderArtifactCleanupOpen"
  | null;

export interface SettingsVisualQaMarker {
  enabled: boolean;
  fixtureId: string;
}

export interface SettingsVisualQaCaptureContext {
  marker: string;
  actionName: string;
}

export interface SettingsVisualQaGenerationModelCatalog {
  loaded: boolean;
  generationModels: Array<{
    provider: string;
    id: string;
    kind: "image" | "video" | "audio" | "upscale";
    displayName: string;
  }>;
}

export interface SettingsVisualQaFixture {
  id: SettingsVisualQaFixtureId;
  surface: SettingsVisualQaSurface;
  category: SettingsVisualQaCategory | null;
  projectContext: "none" | "active";
  projectRoot: string | null;
  expectedMarker: string;
  captureState: SettingsVisualQaCaptureState;
  captureContext: SettingsVisualQaCaptureContext | null;
  appPreferences: AppPreferences;
  generationModelCatalog: SettingsVisualQaGenerationModelCatalog;
  snapshot: SettingsHealthSnapshot;
  systemHealth: SystemHealthSnapshot;
  transcriptionModels: TranscriptionModelStatus[];
  activeTranscriptionModel: TranscriptionModelStatus | null;
  runtimeSelection: RuntimeSelection;
  speechModels: ProductionSpeechModelStatus;
  operations: SettingsOperation[];
  renderSystem: RenderSystemHealth;
  updateHealth: UpdateHealth;
  notificationCapability: NotificationCapability;
  agentHealth: SettingsCategoryHealth;
  mcpConfiguration: McpClientConfigurationState;
  skillsHealth: SettingsCategoryHealth;
  storageHealth: SettingsCategoryHealth;
  providerHealth: ProviderHealth[];
  credentialStatuses: ProviderCredentialStatus[];
  projectRenderArtifactIds: string[];
  renderArtifactCleanupPreview: StorageCleanupPreview | null;
}

const timestamp = "2026-07-20T08:15:00.000Z";
const modelId = "nvidia/parakeet-tdt-0.6b-v3";
const projectRoot = sampleProjectDir;
const modelPath =
  "/Users/visual-qa/Library/Application Support/com.olhapi.video-creater/models/nvidia__parakeet-tdt-0.6b-v3/coreml/ParakeetEncoder.mlmodelc";

function component(
  id: string,
  label: string,
  state: SettingsComponentHealth["state"],
  summary: string,
  provenance: Record<string, string> = {},
): SettingsComponentHealth {
  return {
    id,
    label,
    state,
    summary,
    actionId: state === "ready" ? null : `${id}.retry`,
    actionLabel: state === "ready" ? null : "Retry",
    lastCheckedAt: timestamp,
    diagnosticCode: state === "failed" ? `${id}.visualQaFailure` : null,
    diagnosticDetail:
      state === "failed"
        ? "Deterministic failure detail retained for narrow-layout wrapping and recovery review."
        : null,
    provenance,
  };
}

function category(
  id: string,
  items: SettingsComponentHealth[],
): SettingsCategoryHealth {
  const state = items.some(({ state }) => state === "failed")
    ? "failed"
    : items.some(({ state }) => state === "actionRequired")
      ? "actionRequired"
      : items.some(({ state }) => state === "checking")
        ? "checking"
        : items.some(({ state }) => state === "unavailable")
          ? "unavailable"
          : "ready";
  return { id, state, items };
}

const renderItems = [
  component("render.gstreamerGes", "GStreamer / GES", "ready", "Composition runtime is initialized."),
  component("render.pluginPolicy", "GStreamer plugin policy", "ready", "Reviewed plugins passed policy."),
  component("render.avfoundation", "AVFoundation delivery", "ready", "Native delivery is available."),
  component("render.compatibilityDecoder", "Compatibility decoder", "ready", "Compatibility decoding is available."),
];

const agentItems = [
  component("agent.codex", "Codex app-server", "ready", "Initialize handshake succeeded."),
  component("agent.claude", "Claude CLI", "ready", "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed."),
  component("agent.mcpServer", "Video Creater MCP server", "ready", "MCP handshake succeeded."),
  component("agent.proposalValidator", "Project action validator", "ready", "Structured actions passed validation."),
];

const skillItems = [
  component("video-creater-video-pipeline", "Video pipeline guidance", "ready", "Bundled guidance is verified."),
  component("video-creater-graphics", "Graphics guidance", "ready", "Bundled guidance is verified."),
  component("video-creater-visuals", "Interface guidance", "ready", "Bundled guidance is verified."),
];

function storageItems(activeProject: boolean): SettingsComponentHealth[] {
  const items = [
    component("storage.models", "App-managed models", "ready", "Model storage is available.", {
      path: modelPath,
      bytes: "968000000",
      freeBytes: "128000000000",
      scope: "globalModels",
    }),
    component("storage.cache", "Application cache", "ready", "Disposable cache can be reviewed safely.", {
      path: "/Users/visual-qa/Library/Caches/com.olhapi.video-creater/render-proxies",
      bytes: "4380000000",
      freeBytes: "128000000000",
      scope: "disposableAppCache",
    }),
  ];
  if (activeProject) {
    items.push(
      component("storage.project", "Current project", "ready", "Canonical project data is protected.", {
        path: projectRoot,
        bytes: "27100000000",
        freeBytes: "128000000000",
        scope: "projectRoot",
      }),
      component("storage.projectRenders", "Render artifacts", "ready", "One reviewed render artifact is eligible for cleanup.", {
        path: `${projectRoot}/renders`,
        bytes: "73400320",
        freeBytes: "128000000000",
        scope: "projectRenderArtifacts",
      }),
    );
  }
  return items;
}

const providerHealth: ProviderHealth[] = [
  {
    provider: "openai",
    displayName: "OpenAI",
    credentialSource: "keychain",
    configured: true,
    validationState: "available",
    accountLabel: "Visual QA organization",
    balanceLabel: "Available",
    dependentModelIds: ["openai:gpt-image-1", "openai:sora-2"],
    lastCheckedAt: timestamp,
    diagnosticCode: null,
  },
  {
    provider: "google",
    displayName: "Google AI",
    credentialSource: "missing",
    configured: false,
    validationState: "missing",
    accountLabel: null,
    balanceLabel: null,
    dependentModelIds: ["google:veo-3"],
    lastCheckedAt: timestamp,
    diagnosticCode: null,
  },
];

const credentialStatuses: ProviderCredentialStatus[] = providerHealth.map((provider) => ({
  provider: provider.provider,
  displayName: provider.displayName,
  configured: provider.configured,
  source: provider.credentialSource,
}));

const transcriptionModel: TranscriptionModelStatus = {
  modelId,
  displayName: "Parakeet TDT 0.6B v3",
  isActive: true,
  installStatus: "ready",
  localPath: modelPath,
  approximateSizeBytes: 968_000_000,
  installedBytes: 968_000_000,
  downloadedFiles: 18,
  totalFiles: 18,
  sourceRepoId: modelId,
  sourceRevision: "4f3b37cf70a8271d8c519eb1761d3f7781f54327",
  sourceLicense: "CC-BY-4.0",
  artifactFormat: "compiled Core ML bundle",
  runtimeId: "coreml-native",
  lastErrorCode: null,
  lastErrorDetail: null,
  verifiedAt: timestamp,
};

const speechModels: ProductionSpeechModelStatus = {
  modelSetId: "production-speech-analysis-v1",
  runtimeId: "fluid-audio-native",
  ready: true,
  installedFiles: 7,
  totalFiles: 7,
  installedBytes: 344_000_000,
  totalBytes: 344_000_000,
  rootPath: "/Users/visual-qa/Library/Application Support/com.olhapi.video-creater/models/speech-analysis/production-v1",
  vadRepo: "FluidInference/silero-vad-coreml",
  vadRevision: "9eb9d7f1958ac45562d483f26c1d57e940195d63",
  diarizationRepo: "FluidInference/speaker-diarization-coreml",
  diarizationRevision: "27c6d2693f065b26efea062c193ac0673263d94a",
  artifactFormat: "compiled Core ML bundles",
  licenses: ["MIT", "Apache-2.0"],
  lastErrorCode: null,
  lastErrorDetail: null,
  lastErrorRecoveryAction: null,
};

const generationModelCatalog: SettingsVisualQaGenerationModelCatalog = {
  loaded: true,
  generationModels: [
    { provider: "openai", id: "gpt-image-1", kind: "image", displayName: "GPT Image 1" },
    { provider: "openai", id: "sora-2", kind: "video", displayName: "Sora 2" },
    { provider: "google", id: "veo-3", kind: "video", displayName: "Veo 3" },
  ],
};

const appPreferences: AppPreferences = {
  schemaVersion: 2,
  projectLocation: { mode: "ask" },
  requireProviderUploadConfirmation: true,
  renderCompletionNotifications: false,
  newProjectDefaults: {
    width: 1920,
    height: 1080,
    fps: 30,
    loudnessLufs: -14,
    captions: "burn_in",
  },
  enabledGenerationModelIds: ["openai:gpt-image-1", "openai:sora-2"],
  generationExecutionBackend: "inProcess",
  agentBackend: "automatic",
  claudeModel: "sonnet",
  claudeExecutablePath: "",
};

function systemComponent(
  id: string,
  label: string,
  state: SystemHealthComponent["state"],
  summary: string,
): SystemHealthComponent {
  return {
    id,
    label,
    state,
    summary,
    actionId: state === "ready" ? null : `${id}.retry`,
    actionLabel: state === "ready" ? null : "Retry",
    lastCheckedAt: timestamp,
    diagnosticCode: state === "failed" ? `${id}.visualQaFailure` : null,
    diagnosticDetail: state === "failed" ? "The deterministic renderer probe failed." : null,
    provenance: {},
  };
}

function systemSection(
  id: string,
  label: string,
  required: boolean,
  state: SystemHealthSection["state"],
  items: SystemHealthComponent[],
): SystemHealthSection {
  return { id, label, required, state, items };
}

function systemHealthSnapshot(): SystemHealthSnapshot {
  return {
    generatedAt: timestamp,
    overall: "failed",
    sections: {
      rendering: systemSection("rendering", "Rendering", true, "failed", [
        systemComponent("render.gstreamerGes", "GStreamer / GES", "failed", "GStreamer composition probe failed locally."),
        systemComponent("render.avfoundation", "AVFoundation delivery", "ready", "Native delivery remains available."),
      ]),
      localAi: systemSection("localAi", "Local AI", true, "ready", [
        systemComponent("localAi.transcription", "Core ML transcription", "ready", "Active model integrity is verified."),
      ]),
      // Not required, and neither backend is: the Rust section registers `required: false`
      // because one working agent is a working install (settings/health.rs). The fixture shows
      // the shape a Claude-only install has — Codex absent, the app usable.
      agent: systemSection("agent", "Agent Runtime", false, "ready", [
        systemComponent("agent.claude", "Claude CLI", "ready", "Signed in with a Claude subscription."),
        systemComponent("agent.codex", "Codex app-server", "notConfigured", "The bundled Codex runtime is missing from this app installation. It is optional: turns can run on Claude instead."),
      ]),
      project: systemSection("project", "Project Integrity", true, "ready", [
        systemComponent("project.schema", "Split project schema", "ready", "The active project manifest is valid."),
      ]),
      environment: systemSection("environment", "Environment", false, "unavailable", [
        systemComponent("environment.updater", "Application updater", "unavailable", "No signed updater is included in this build."),
      ]),
    },
  };
}

function buildFixture(
  id: SettingsVisualQaFixtureId,
  surface: SettingsVisualQaSurface,
  categoryId: SettingsVisualQaCategory | null,
  expectedMarker: string,
  captureState: SettingsVisualQaCaptureState = null,
): SettingsVisualQaFixture {
  const activeProject = surface !== "appSettings";
  const general = category("general", renderItems);
  const models = category("models", [
    component("models.transcription", transcriptionModel.displayName, "ready", "Installed and verified."),
  ]);
  const agent = category("agent", agentItems);
  const skills = category("skills", skillItems);
  const storage = category("storage", storageItems(activeProject));
  const providers = category("providers", providerHealth.map((provider) =>
    component(
      `provider.${provider.provider}`,
      provider.displayName,
      provider.configured ? "ready" : "unavailable",
      provider.configured ? "Credential validated in Keychain." : "Optional provider is not configured.",
    ),
  ));
  const snapshot: SettingsHealthSnapshot = {
    generatedAt: timestamp,
    overall: "ready",
    categories: { general, models, agent, skills, storage, providers },
  };
  const projectRenderArtifactIds = activeProject ? ["visual-qa-render-1"] : [];
  const cleanupPreview: StorageCleanupPreview | null = activeProject
    ? {
        target: { kind: "projectRenderArtifacts", artifactIds: projectRenderArtifactIds },
        items: [
          { path: "renders/visual-qa-render-1/output.webm", bytes: 62_914_560 },
          { path: "renders/visual-qa-render-1/pipeline-report.json", bytes: 12_288 },
        ],
        totalBytes: 62_926_848,
        previewNonce: "settings-visual-qa-render-cleanup",
        confirmationToken: "settings-visual-qa-render-cleanup-confirmation",
        projectGeneration: 1,
      }
    : null;

  return {
    id,
    surface,
    category: categoryId,
    projectContext: activeProject ? "active" : "none",
    projectRoot: activeProject ? projectRoot : null,
    expectedMarker,
    captureState,
    captureContext:
      captureState === "renderArtifactCleanupOpen"
        ? { marker: "Confirm render artifact cleanup", actionName: "Delete reviewed render files" }
        : captureState === "generationModelSelectorOpen"
          ? { marker: "GPT Image 1", actionName: "GPT Image 1, OpenAI" }
          : null,
    appPreferences: { ...appPreferences },
    generationModelCatalog: {
      loaded: generationModelCatalog.loaded,
      generationModels: generationModelCatalog.generationModels.map((model) => ({ ...model })),
    },
    snapshot,
    systemHealth: systemHealthSnapshot(),
    transcriptionModels: [{ ...transcriptionModel }],
    activeTranscriptionModel: { ...transcriptionModel },
    runtimeSelection: "native",
    speechModels: { ...speechModels },
    operations: [],
    renderSystem: {
      checkedAt: timestamp,
      state: general.state,
      compositionReady: true,
      nativeDeliveryDegraded: false,
      compatibilityDegraded: false,
      items: general.items,
    },
    updateHealth: {
      state: "unavailable",
      installedVersion: "0.1.0-visual-qa",
      summary: "No signed updater is included in this build.",
    },
    notificationCapability: {
      state: "ready",
      deliveryAvailable: true,
      permissionStatus: "authorized",
      canRequest: false,
      summary: "Native render notifications are available but not enabled.",
      diagnosticCode: null,
      diagnosticDetail: null,
    },
    agentHealth: agent,
    mcpConfiguration: {
      actionLabel: activeProject ? "Copy client configuration" : "Open a project",
      configuration: activeProject
        ? JSON.stringify({ mcpServers: { "video-creater": { command: "video-creater", args: ["mcp", "--project", projectRoot] } } }, null, 2)
        : null,
      executable: "/Applications/Video Creater.app/Contents/MacOS/video-creater",
      projectDir: activeProject ? projectRoot : null,
    },
    skillsHealth: skills,
    storageHealth: storage,
    providerHealth: providerHealth.map((provider) => ({ ...provider })),
    credentialStatuses: credentialStatuses.map((status) => ({ ...status })),
    projectRenderArtifactIds,
    renderArtifactCleanupPreview: cleanupPreview,
  };
}

const fixtureCatalog: Record<SettingsVisualQaFixtureId, SettingsVisualQaFixture> = {
  "settings-general-no-project": buildFixture("settings-general-no-project", "appSettings", "general", "Provider upload confirmation"),
  "settings-projects-no-project": buildFixture("settings-projects-no-project", "appSettings", "projects", "New project defaults"),
  "settings-ai-models-no-project": buildFixture("settings-ai-models-no-project", "appSettings", "aiModels", "Generation models", "generationModelSelectorOpen"),
  "settings-integrations-no-project": buildFixture("settings-integrations-no-project", "appSettings", "integrations", "Integration status"),
  "settings-storage-no-project": buildFixture("settings-storage-no-project", "appSettings", "storage", "Storage inventory"),
  "settings-advanced-no-project": buildFixture("settings-advanced-no-project", "appSettings", "advanced", "Open a project to copy MCP configuration"),
  "project-settings-render-cleanup": buildFixture("project-settings-render-cleanup", "projectSettings", null, "Confirm render artifact cleanup", "renderArtifactCleanupOpen"),
  "system-health-project-failed": buildFixture("system-health-project-failed", "systemHealth", null, "GStreamer composition probe failed locally"),
};

function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const nested of Object.values(value as Record<string, unknown>)) {
      deepFreeze(nested);
    }
  }
  return value;
}

export const settingsVisualQaFixtures = deepFreeze(fixtureCatalog);

export function resolveSettingsVisualQaFixture(input: {
  development: boolean;
  marker: SettingsVisualQaMarker | null | undefined;
}): SettingsVisualQaFixture | null {
  if (!input.development || input.marker?.enabled !== true) return null;
  if (!settingsVisualQaFixtureIds.includes(input.marker.fixtureId as SettingsVisualQaFixtureId)) {
    return null;
  }
  return settingsVisualQaFixtures[input.marker.fixtureId as SettingsVisualQaFixtureId];
}

export function readSettingsVisualQaFixture(): SettingsVisualQaFixture | null {
  if (!import.meta.env.DEV || typeof window === "undefined") return null;
  const fixtureId = (
    window as Window & {
      __EDITOR_FIXTURE_RUNTIME__?: { enabled?: boolean; settingsFixtureId?: string };
    }
  ).__EDITOR_FIXTURE_RUNTIME__?.settingsFixtureId;
  const marker = fixtureId ? { enabled: true, fixtureId } : null;
  return resolveSettingsVisualQaFixture({ development: true, marker });
}

export function applySettingsVisualQaProjectFixture(
  project: VideoProject,
  fixture: SettingsVisualQaFixture | null,
): VideoProject {
  if (!fixture || fixture.surface !== "projectSettings") return project;
  return {
    ...project,
    renderReports: [{
      schemaVersion: 1,
      id: "visual-qa-render-1",
      status: "completed",
      outputPath: "renders/visual-qa-render-1/output.webm",
      durationSeconds: 5,
      streams: { video: true, audio: true },
      checks: { duration: "passed", streams: "passed", captions: "skipped" },
      artifacts: ["renders/visual-qa-render-1/pipeline-report.json"],
      previewComparisonRequest: null,
      previewComparison: null,
      logPath: "renders/visual-qa-render-1/render.log",
      createdAt: timestamp,
    }],
  };
}

export function settingsVisualQaOperations(
  fixture: SettingsVisualQaFixture,
): ReadonlyMap<string, FixtureOperationHandler> {
  const responses: Record<string, unknown> = {
    get_app_preferences: fixture.appPreferences,
    get_settings_acceptance_context: null,
    get_settings_health_snapshot: fixture.snapshot,
    get_system_health_snapshot: fixture.systemHealth,
    get_storage_health: fixture.storageHealth,
    list_transcription_models: fixture.transcriptionModels,
    get_active_transcription_model: fixture.activeTranscriptionModel,
    get_transcription_runtime_status: fixture.runtimeSelection,
    get_production_speech_model_status: fixture.speechModels,
    list_settings_operations: fixture.operations,
    list_generation_model_catalog: fixture.generationModelCatalog,
    get_render_system_health: fixture.renderSystem,
    get_update_health: fixture.updateHealth,
    get_notification_capability: fixture.notificationCapability,
    get_agent_settings_health: fixture.agentHealth,
    mcp_client_configuration: fixture.mcpConfiguration,
    get_skills_settings_health: fixture.skillsHealth,
    get_provider_health: fixture.providerHealth,
    list_provider_credential_statuses: fixture.credentialStatuses,
    preview_storage_cleanup: fixture.renderArtifactCleanupPreview,
    materialize_sample_project_media: undefined,
    sync_native_menu_state: undefined,
  };
  return new Map<string, FixtureOperationHandler>([
    ...Object.entries(responses).map(
      ([operation, response]): [string, FixtureOperationHandler] => [operation, () => response],
    ),
    // Saves return the committed project like the native command, so callers read result.project.
    [
      "save_split_project_to_folder",
      (input) => ({
        project: (input as { project?: VideoProject } | undefined)?.project,
        report: { manifestPath: "video-creater.project.json", writtenFiles: [], removedFiles: [] },
      }),
    ],
  ]);
}
