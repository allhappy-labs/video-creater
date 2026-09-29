import { nativeMenuEventName } from "./native-menu";
import type { ProjectRenderReport, VideoProject } from "./project";
import { createSampleProject } from "./sample-project";

const MODEL_ID = "nvidia/parakeet-tdt-0.6b-v3";
const TRANSCRIPTION_RUNTIME_ID = "fluid_audio_coreml";
const SETTINGS_ACCEPTANCE_UI_WAIT_ATTEMPTS = 600;
const SETTINGS_ACCEPTANCE_MENU_EMIT_ATTEMPTS = 60;
const SPEECH_MODEL_SET_ID = "silero-vad+wspk-vbx-v1";
const SPEECH_RUNTIME_ID = "fluid_audio_speech_analysis";
const ACCEPTANCE_RENDER_ARTIFACT_ID = "settings-acceptance-render-artifact";
const ACCEPTANCE_RENDER_REPORT_PATH =
  `renders/${ACCEPTANCE_RENDER_ARTIFACT_ID}/report.json`;
const SETTINGS_CATEGORIES = [
  { id: "general", label: "General" },
  { id: "projects", label: "Projects" },
  { id: "aiModels", label: "AI & Models" },
  { id: "integrations", label: "Integrations" },
  { id: "storage", label: "Storage" },
  { id: "advanced", label: "Advanced" },
] as const;

function createAcceptanceRenderReport(): ProjectRenderReport {
  return {
    schemaVersion: 1,
    id: ACCEPTANCE_RENDER_ARTIFACT_ID,
    status: "completed",
    outputPath: `renders/${ACCEPTANCE_RENDER_ARTIFACT_ID}/output.mp4`,
    durationSeconds: 8,
    streams: { video: true, audio: true },
    checks: { artifactPaths: "passed", duration: "passed", streams: "passed" },
    artifacts: [ACCEPTANCE_RENDER_REPORT_PATH],
    previewComparisonRequest: null,
    previewComparison: null,
    logPath: `renders/${ACCEPTANCE_RENDER_ARTIFACT_ID}/render.log`,
    createdAt: "2026-07-12T00:00:00Z",
  };
}

function createAcceptanceProject(): VideoProject {
  return {
    ...createSampleProject(),
    renderReports: [createAcceptanceRenderReport()],
  };
}

type AcceptanceStage = "pre_restart" | "post_restart";

type SettingsAcceptanceProgressStep =
  | "settingsDomSettled"
  | "interruptedRecoveryConfirmed"
  | "modelDownloadRequested"
  | "modelDownloadSettled"
  | "modelVerificationRequested"
  | "modelVerificationSettled"
  | "speechSeparationConfirmed"
  | "storageProjectLoadRequested"
  | "storageCleanupSettled"
  | "providerKeychainSettled"
  | "agentMcpSkillsSettled";

interface SettingsAcceptanceContext {
  stage: AcceptanceStage;
  projectRoot: string;
}

interface ModelStatus {
  modelId: string;
  installStatus: string;
  runtimeId: string;
}

interface SettingsAcceptanceProjectLoad {
  id: string;
  renderArtifactIds: string[];
}

interface SettingsOperation {
  id: string;
  kind: string;
  targetId: string;
  state: string;
  phase: string;
  completedUnits: number;
  error: { code: string } | null;
}

interface ProductionSpeechModelStatus {
  modelSetId: string;
  runtimeId: string;
  ready: boolean;
  installedFiles: number;
  totalFiles: number;
  installedBytes: number;
  totalBytes: number;
  rootPath: string;
  vadRepo: string;
  vadRevision: string;
  diarizationRepo: string;
  diarizationRevision: string;
  artifactFormat: string;
  licenses: string[];
  lastErrorCode: string | null;
}

interface AcceptanceCheck {
  id:
    | "settingsDom"
    | "modelCancel"
    | "interruptedRecovery"
    | "modelReady"
    | "speechSeparation"
    | "storageCleanup"
    | "providerKeychain"
    | "agentMcpSkills";
  status: "passed" | "failed" | "blocked";
  diagnosticCode: string;
}

interface FinalSettingsBlockedActionEvidence {
  id: string;
  configureTarget: string;
  observedTarget: string | null;
  configureTargetResolved: boolean;
}

interface FinalSettingsHealthSectionEvidence {
  id: string;
  required: boolean;
  items: Array<{ id: string }>;
}

export interface FinalSettingsAcceptanceEvidence {
  categories: readonly string[];
  sidebarHealthBadges: number;
  environmentCredentialControls: number;
  modelSelectionControl: "multiselect-combobox" | "missing";
  modelComboboxOpen: boolean;
  modelFilterQuery: string;
  filteredModelResults: number;
  blockedActions: FinalSettingsBlockedActionEvidence[];
  systemHealthSections: FinalSettingsHealthSectionEvidence[];
}

export interface FinalSettingsAcceptanceReport {
  appSettings: {
    categories: string[];
    sidebarHealthBadges: number;
  };
  providers: {
    environmentCredentialControls: number;
  };
  models: {
    selectionControl: "multiselect-combobox" | "missing";
    comboboxOpen: boolean;
    filterQuery: string;
    filteredResultCount: number;
  };
  blockedActions: FinalSettingsBlockedActionEvidence[];
  systemHealth: {
    optionalProviderCountedAsRequired: boolean;
  };
}

export function createFinalSettingsAcceptanceReport(
  evidence: FinalSettingsAcceptanceEvidence,
): FinalSettingsAcceptanceReport {
  const providerSections = evidence.systemHealthSections.filter((section) =>
    section.items.some(({ id }) => /provider|openai|fal|replicate|google/i.test(id)),
  );
  return {
    appSettings: {
      categories: [...evidence.categories],
      sidebarHealthBadges: evidence.sidebarHealthBadges,
    },
    providers: {
      environmentCredentialControls: evidence.environmentCredentialControls,
    },
    models: {
      selectionControl: evidence.modelSelectionControl,
      comboboxOpen: evidence.modelComboboxOpen,
      filterQuery: evidence.modelFilterQuery,
      filteredResultCount: evidence.filteredModelResults,
    },
    blockedActions: evidence.blockedActions.map((action) => ({ ...action })),
    systemHealth: {
      optionalProviderCountedAsRequired: providerSections.some(
        (section) => section.required,
      ),
    },
  };
}

export interface SettingsAcceptanceBridge {
  invoke(command: string, args?: Record<string, unknown>): Promise<unknown>;
  emit(event: string, payload: unknown): Promise<void>;
  document: Document;
  sleep(milliseconds: number): Promise<void>;
}

class SettingsAcceptancePhaseError extends Error {
  readonly phase: AcceptanceCheck["id"];
  readonly cause: unknown;

  constructor(phase: AcceptanceCheck["id"], cause: unknown) {
    super(cause instanceof Error ? cause.message : "Settings acceptance phase failed");
    this.name = "SettingsAcceptancePhaseError";
    this.phase = phase;
    this.cause = cause;
  }
}

export async function runSettingsAcceptancePhase<T>(
  phase: AcceptanceCheck["id"],
  operation: () => Promise<T>,
): Promise<T> {
  try {
    return await operation();
  } catch (error) {
    throw new SettingsAcceptancePhaseError(phase, error);
  }
}

export function settingsAcceptanceFailurePhase(error: unknown): AcceptanceCheck["id"] {
  return error instanceof SettingsAcceptancePhaseError ? error.phase : "settingsDom";
}

export function settingsAcceptanceFailureDiagnosticCode(error: unknown): string {
  const phase = settingsAcceptanceFailurePhase(error);
  if (!(error instanceof SettingsAcceptancePhaseError)) {
    return `settings.acceptance.${phase}.failed`;
  }
  const message = error.message;
  if (phase === "storageCleanup") {
    if (/project render artifact fixture was not isolated/i.test(message)) {
      return "settings.acceptance.storageCleanup.renderArtifacts";
    }
    if (/disposable app cache fixture was not isolated/i.test(message)) {
      return "settings.acceptance.storageCleanup.disposableCache";
    }
    if (/settings\.storage\.cleanupConfirmationMismatch/i.test(message)) {
      return "settings.acceptance.storageCleanup.confirmationMismatch";
    }
    if (/settings\.storage\.cleanupProgressFailed/i.test(message)) {
      return "settings.acceptance.storageCleanup.progressPersistence";
    }
    if (/settings\.storage\.cleanupProjectSessionMismatch/i.test(message)) {
      return "settings.acceptance.storageCleanup.projectSession";
    }
    if (/storage cleanup health command was rejected/i.test(message)) {
      return "settings.acceptance.storageCleanup.healthCommand";
    }
    if (/storage cleanup preview command was rejected/i.test(message)) {
      return "settings.acceptance.storageCleanup.previewCommand";
    }
    if (/storage cleanup start command was rejected/i.test(message)) {
      return "settings.acceptance.storageCleanup.startCommand";
    }
    return "settings.acceptance.storageCleanup.failed";
  }
  if (phase !== "settingsDom") return `settings.acceptance.${phase}.failed`;
  if (
    /category rail|Settings category tab|health badge/i.test(message)
  ) {
    return "settings.acceptance.settingsDom.categoryTabs";
  }
  if (/expected GPT Image result/i.test(message)) {
    return "settings.acceptance.settingsDom.modelResults";
  }
  if (
    /generation model|AI & Models category tab/i.test(message)
  ) {
    return "settings.acceptance.settingsDom.models";
  }
  if (/blocked Settings action/i.test(message)) {
    return "settings.acceptance.settingsDom.blockedActions";
  }
  if (/Integrations|environment credential/i.test(message)) {
    return "settings.acceptance.settingsDom.integrations";
  }
  return "settings.acceptance.settingsDom.failed";
}

function matchesSearchTerms(value: string, query: string): boolean {
  const words = value.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean);
  return query
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
    .every((term) => words.some((word) => word.includes(term)));
}

export async function runSettingsAcceptanceIfEnabled(
  bridge: SettingsAcceptanceBridge,
): Promise<void> {
  const context = await bridge.invoke("get_settings_acceptance_context");
  if (!isAcceptanceContext(context)) {
    return;
  }
  try {
    const uiEvidence = await runSettingsAcceptancePhase(
      "settingsDom",
      () => openAndTraverseSettings(bridge),
    );
    if (context.stage === "pre_restart") {
      await runSettingsAcceptancePhase(
        "modelCancel",
        () => runPreRestart(bridge, context),
      );
    } else {
      await runPostRestart(bridge, context, uiEvidence);
    }
  } catch (error) {
    const phase = settingsAcceptanceFailurePhase(error);
    const failure = {
      stage: context.stage,
      phase,
      diagnosticCode: settingsAcceptanceFailureDiagnosticCode(error),
    };
    try {
      await bridge.invoke("write_settings_acceptance_failure", {
        failure,
      });
    } catch {
      try {
        await bridge.invoke("abort_settings_acceptance_run", { failure });
      } catch {
        // Preserve the original typed phase error if the emergency exit command is unavailable.
      }
    }
    throw error;
  }
}

interface SettingsUiEvidence {
  categories: string[];
  sidebarHealthBadges: number;
  environmentCredentialControls: number;
  modelSelectionControl: "multiselect-combobox" | "missing";
  modelComboboxOpen: boolean;
  modelFilterQuery: string;
  filteredModelResults: number;
  blockedActions: FinalSettingsBlockedActionEvidence[];
}

async function openAndTraverseSettings(
  bridge: SettingsAcceptanceBridge,
): Promise<SettingsUiEvidence> {
  await bridge.sleep(250);
  let categoryRail: Element | null = null;
  for (let attempt = 1; attempt <= SETTINGS_ACCEPTANCE_MENU_EMIT_ATTEMPTS; attempt += 1) {
    await bridge.emit(nativeMenuEventName, { sequence: attempt, command: "openSettings" });
    categoryRail = bridge.document.querySelector(
      '[data-settings-acceptance-category-rail][role="tablist"]',
    );
    if (categoryRail) break;
    await bridge.sleep(1_000);
  }
  categoryRail ??= await waitForElement(
    bridge,
    '[data-settings-acceptance-category-rail][role="tablist"]',
  );
  const allCategoryTabs = [
    ...categoryRail.querySelectorAll<HTMLElement>('[role="tab"]'),
  ];
  const categoryTabs = [
    ...categoryRail.querySelectorAll<HTMLElement>(
      '[role="tab"][data-settings-category-id]',
    ),
  ];
  const categories = categoryTabs.map((tab) => tab.dataset.settingsCategoryId ?? "");
  const expectedCategories = SETTINGS_CATEGORIES.map(({ id }) => id);
  if (
    allCategoryTabs.length !== categoryTabs.length ||
    JSON.stringify(categories) !== JSON.stringify(expectedCategories)
  ) {
    throw new Error("Settings category rail does not contain the exact marked categories");
  }
  const sidebarHealthBadges = categoryRail.querySelectorAll(
    "[data-settings-health-badge]",
  ).length;
  if (sidebarHealthBadges !== 0) {
    throw new Error("Settings category rail contains a health badge");
  }
  let environmentCredentialControls = 0;
  let modelSelectionControl: SettingsUiEvidence["modelSelectionControl"] = "missing";
  let modelComboboxOpen = false;
  let modelFilterQuery = "";
  let filteredModelResults = 0;
  const blockedActions: FinalSettingsBlockedActionEvidence[] = [];
  for (const [index, category] of SETTINGS_CATEGORIES.entries()) {
    const button = categoryTabs[index];
    if (!(button instanceof HTMLButtonElement) || button.type !== "button") {
      throw new Error(
        `Settings category tab is invalid: ${category.label} (settings-tab-${category.id})`,
      );
    }
    button.click();
    await bridge.sleep(25);
    if (category.id === "aiModels") {
      const selector = await waitForElement(
        bridge,
        '[role="combobox"][aria-label="Enabled generation models"]',
      );
      modelSelectionControl =
        selector instanceof HTMLButtonElement ? "multiselect-combobox" : "missing";
      if (!(selector instanceof HTMLButtonElement)) {
        throw new Error("generation model selection control is not a button combobox");
      }
      selector.click();
      let modelMenu = await waitForElement(
        bridge,
        '[role="menu"][aria-label="Generation model choices"]',
      );
      modelComboboxOpen = selector.getAttribute("aria-expanded") !== "false";
      const search = await waitForElement(
        bridge,
        'input[type="search"][aria-label="Search generation models"]',
      );
      if (!(search instanceof HTMLInputElement)) {
        throw new Error("generation model filter is not a search input");
      }
      setInputValue(search, "gpt image");
      await waitForCondition(
        bridge,
        () => search.value === "gpt image",
        "generation model filter did not accept the acceptance query",
      );
      const filteredResults = () => [
        ...modelMenu.querySelectorAll<HTMLElement>('[role="menuitemcheckbox"]'),
      ].filter((item) => matchesSearchTerms(
        item.getAttribute("aria-label") ?? "",
        "gpt image",
      ));
      await waitForCondition(
        bridge,
        () => filteredResults().length > 0,
        "generation model filter did not retain the expected GPT Image result",
      );
      modelFilterQuery = search.value;
      filteredModelResults = filteredResults().length;

      const configureActionDescriptors = [
        ...modelMenu.querySelectorAll<HTMLElement>("[data-settings-blocked-action]"),
      ].map((action) => ({
        id: action.dataset.settingsBlockedAction ?? "",
        configureTarget: action.dataset.settingsConfigureTarget ?? "",
      }));
      if (configureActionDescriptors.length === 0) {
        throw new Error("generation model menu did not expose marked blocked Settings actions");
      }
      for (const [actionIndex, descriptor] of configureActionDescriptors.entries()) {
        if (actionIndex > 0) {
          const aiModelsTab = bridge.document.getElementById("settings-tab-aiModels");
          if (!(aiModelsTab instanceof HTMLButtonElement)) {
            throw new Error("AI & Models category tab disappeared during blocked-action traversal");
          }
          aiModelsTab.click();
          const reopenedSelector = await waitForElement(
            bridge,
            '[role="combobox"][aria-label="Enabled generation models"]',
          );
          if (!(reopenedSelector instanceof HTMLButtonElement)) {
            throw new Error("generation model selection control did not reopen");
          }
          reopenedSelector.click();
          modelMenu = await waitForElement(
            bridge,
            '[role="menu"][aria-label="Generation model choices"]',
          );
        }
        const configure = [
          ...modelMenu.querySelectorAll<HTMLElement>("[data-settings-blocked-action]"),
        ].find(
          (action) =>
            action.dataset.settingsBlockedAction === descriptor.id &&
            action.dataset.settingsConfigureTarget === descriptor.configureTarget,
        );
        const { id, configureTarget } = descriptor;
        if (!(configure instanceof HTMLButtonElement) || !id || !configureTarget) {
          throw new Error("marked blocked Settings action is invalid");
        }
        configure.click();
        let destination: Element;
        try {
          destination = await waitForElement(
            bridge,
            `[data-settings-target="${configureTarget}"]`,
          );
          await waitForCondition(
            bridge,
            () =>
              bridge.document.activeElement === destination &&
              bridge.document
                .getElementById("settings-tab-integrations")
                ?.getAttribute("aria-selected") === "true",
            "blocked Settings action did not focus its exact target",
          );
        } catch {
          throw new Error(`blocked Settings action did not resolve: ${id}`);
        }
        const observedTarget = (destination as HTMLElement).dataset.settingsTarget ?? null;
        if (observedTarget !== configureTarget) {
          throw new Error(`blocked Settings action did not resolve: ${id}`);
        }
        blockedActions.push({
          id,
          configureTarget,
          observedTarget,
          configureTargetResolved: true,
        });
      }
    }
    if (category.id === "integrations") {
      const integrations = await waitForElement(
        bridge,
        '[data-testid="providers-settings-page"][data-settings-credential-boundary="keychain-only"]',
      );
      environmentCredentialControls = integrations.querySelectorAll(
        '[data-settings-credential-control][data-settings-credential-storage="environment"]',
      ).length;
      if (environmentCredentialControls !== 0) {
        throw new Error("Integrations contains an environment credential control");
      }
    }
  }
  return {
    categories,
    sidebarHealthBadges,
    environmentCredentialControls,
    modelSelectionControl,
    modelComboboxOpen,
    modelFilterQuery,
    filteredModelResults,
    blockedActions,
  };
}

async function runPreRestart(
  bridge: SettingsAcceptanceBridge,
  context: SettingsAcceptanceContext,
): Promise<void> {
  const models = requireModels(await bridge.invoke("list_transcription_models"));
  if (!models.some((model) => model.modelId === MODEL_ID)) {
    throw new Error("pinned Parakeet catalog entry is unavailable");
  }
  const operation = requireOperation(
    await bridge.invoke("download_transcription_model", { modelId: MODEL_ID }),
  );
  await waitForOperation(bridge, operation.id, (candidate) => candidate.completedUnits > 0);
  await bridge.invoke("cancel_model_download", { modelId: MODEL_ID });
  await waitForOperation(bridge, operation.id, (candidate) => candidate.state === "cancelled");
  const interruptedCandidate = requireOperation(
    await bridge.invoke("download_transcription_model", { modelId: MODEL_ID }),
  );
  await waitForOperation(
    bridge,
    interruptedCandidate.id,
    (candidate) =>
      candidate.kind === "modelDownload" &&
      candidate.targetId === MODEL_ID &&
      candidate.state === "running" &&
      candidate.completedUnits > 0,
  );
  const acceptanceProject = createAcceptanceProject();
  await bridge.invoke("save_split_project_to_folder", {
    projectDir: context.projectRoot,
    project: acceptanceProject,
    expectedRevision: acceptanceProject.contentRevision ?? 0,
  });
  await writeCheckpoint(bridge, "pre_restart", [
    passed("settingsDom"),
    passed("modelCancel"),
  ]);
}

async function runPostRestart(
  bridge: SettingsAcceptanceBridge,
  context: SettingsAcceptanceContext,
  uiEvidence: SettingsUiEvidence,
): Promise<void> {
  await writeProgress(bridge, context.stage, "settingsDomSettled");
  await runSettingsAcceptancePhase("interruptedRecovery", async () => {
    const operations = requireOperations(await bridge.invoke("list_settings_operations"));
    const interruptedRecovered = operations.some(
      (operation) =>
        operation.kind === "modelDownload" &&
        operation.targetId === MODEL_ID &&
        operation.state === "failed" &&
        operation.phase === "interrupted" &&
        operation.error?.code === "settings.operation.interrupted",
    );
    if (!interruptedRecovered) {
      throw new Error("identified Parakeet download interruption was not recovered");
    }
  });
  await writeProgress(bridge, context.stage, "interruptedRecoveryConfirmed");
  const verified = await runSettingsAcceptancePhase("modelReady", async () => {
    await writeProgress(bridge, context.stage, "modelDownloadRequested");
    const operation = requireOperation(
      await bridge.invoke("download_transcription_model", { modelId: MODEL_ID }),
    );
    await waitForOperation(bridge, operation.id, (candidate) => candidate.state === "succeeded");
    await writeProgress(bridge, context.stage, "modelDownloadSettled");
    await writeProgress(bridge, context.stage, "modelVerificationRequested");
    const model = requireModel(
      await bridge.invoke("verify_transcription_model", { modelId: MODEL_ID }),
    );
    if (model.installStatus !== "ready") {
      throw new Error("Parakeet model did not verify ready");
    }
    if (model.runtimeId !== TRANSCRIPTION_RUNTIME_ID) {
      throw new Error("Parakeet model did not verify with the packaged transcription runtime");
    }
    await writeProgress(bridge, context.stage, "modelVerificationSettled");
    return model;
  });
  await runSettingsAcceptancePhase("speechSeparation", async () => {
    const speechStatus = requireProductionSpeechModelStatus(
      await bridge.invoke("get_production_speech_model_status"),
    );
    if (speechStatus.runtimeId === verified.runtimeId) {
      throw new Error("production speech analysis was conflated with transcription readiness");
    }
  });
  await writeProgress(bridge, context.stage, "speechSeparationConfirmed");
  await runSettingsAcceptancePhase("storageCleanup", async () => {
    await writeProgress(bridge, context.stage, "storageProjectLoadRequested");
    const loadedProject = requireSettingsAcceptanceProjectLoad(
      await bridge.invoke("load_settings_acceptance_project", {
        projectDir: context.projectRoot,
      }),
    );
    if (loadedProject.id !== createAcceptanceProject().id) {
      throw new Error("reloaded acceptance project identity did not match the saved project");
    }
    await runStorageCleanup(
      bridge,
      context.projectRoot,
      loadedProject.renderArtifactIds,
    );
  });
  await writeProgress(bridge, context.stage, "storageCleanupSettled");
  await runSettingsAcceptancePhase(
    "providerKeychain",
    () => runProviderKeychainRoundTrip(bridge, context.projectRoot),
  );
  await writeProgress(bridge, context.stage, "providerKeychainSettled");
  const systemHealthSections = await runSettingsAcceptancePhase("agentMcpSkills", async () => {
    await runAgentMcpSkills(bridge, context.projectRoot);
    return requireSystemHealthSections(
      await bridge.invoke("get_system_health_snapshot", {
        projectRoot: context.projectRoot,
      }),
    );
  });
  await writeProgress(bridge, context.stage, "agentMcpSkillsSettled");
  const productContract = createFinalSettingsAcceptanceReport({
    ...uiEvidence,
    systemHealthSections,
  });
  await writeCheckpoint(bridge, "post_restart", [
    passed("settingsDom"),
    passed("interruptedRecovery"),
    passed("modelReady"),
    passed("speechSeparation"),
    passed("storageCleanup"),
    passed("providerKeychain"),
    passed("agentMcpSkills"),
  ], productContract);
}

async function writeProgress(
  bridge: SettingsAcceptanceBridge,
  stage: AcceptanceStage,
  step: SettingsAcceptanceProgressStep,
): Promise<void> {
  await bridge.invoke("write_settings_acceptance_progress", {
    progress: { stage, step },
  });
}

async function runStorageCleanup(
  bridge: SettingsAcceptanceBridge,
  projectRoot: string,
  renderArtifactIds: string[],
): Promise<void> {
  const renderTarget = {
    kind: "projectRenderArtifacts",
    artifactIds: renderArtifactIds,
  };
  await runStorageCleanupTarget(
    bridge,
    renderTarget,
    projectRoot,
    "project render artifact fixture was not isolated for cleanup",
  );

  const target = { kind: "disposableAppCache" };
  await runStorageCleanupTarget(
    bridge,
    target,
    null,
    "disposable app cache fixture was not isolated for cleanup",
  );
}

function requireSettingsAcceptanceProjectLoad(value: unknown): SettingsAcceptanceProjectLoad {
  const project = requireRecord(value, "reloaded acceptance project");
  if (typeof project.id !== "string" || project.id.length === 0) {
    throw new Error("reloaded acceptance project identity was invalid");
  }
  if (!Array.isArray(project.renderArtifactIds)) {
    throw new Error("reloaded acceptance project did not contain render artifact IDs");
  }
  if (
    !project.renderArtifactIds.every(
      (id): id is string => typeof id === "string" && id.length > 0,
    ) || !project.renderArtifactIds.includes(ACCEPTANCE_RENDER_ARTIFACT_ID)
  ) {
    throw new Error("acceptance render report did not survive native project reload");
  }
  return { id: project.id, renderArtifactIds: project.renderArtifactIds };
}

async function runStorageCleanupTarget(
  bridge: SettingsAcceptanceBridge,
  target: { kind: string; artifactIds?: string[] },
  activeProjectDir: string | null,
  emptyPreviewMessage: string,
): Promise<void> {
  try {
    await bridge.invoke("get_storage_health", { activeProjectDir });
  } catch {
    throw new Error("Settings storage cleanup health command was rejected");
  }
  for (let attempt = 0; attempt < 2; attempt += 1) {
    let previewResult: unknown;
    try {
      previewResult = await bridge.invoke("preview_storage_cleanup", {
        target,
        activeProjectDir,
      });
    } catch {
      throw new Error("Settings storage cleanup preview command was rejected");
    }
    const preview = requireRecord(previewResult, "storage cleanup preview");
    if (
      !Array.isArray(preview.items) ||
      preview.items.length === 0 ||
      typeof preview.confirmationToken !== "string" ||
      preview.confirmationToken.length === 0
    ) {
      throw new Error(emptyPreviewMessage);
    }
    let operationResult: unknown;
    try {
      operationResult = await bridge.invoke("run_storage_cleanup", {
        target,
        confirmationToken: preview.confirmationToken,
        activeProjectDir,
      });
    } catch {
      throw new Error("Settings storage cleanup start command was rejected");
    }
    const operation = requireOperation(operationResult);
    try {
      await waitForOperation(bridge, operation.id, (candidate) => candidate.state === "succeeded");
      return;
    } catch (error) {
      if (attempt === 1) throw error;
    }
  }
}

async function runProviderKeychainRoundTrip(
  bridge: SettingsAcceptanceBridge,
  projectRoot: string,
): Promise<void> {
  const runName = acceptanceRunName(projectRoot);
  const canaries = [
    `video-creater-acceptance-canary-${runName}-first`,
    `video-creater-acceptance-canary-${runName}-replacement`,
  ] as const;
  try {
    for (const credential of canaries) {
      const status = await bridge.invoke("set_provider_credential", {
        provider: "openai",
        credential,
      });
      assertProviderStatus(status, true, "keychain");
      assertSecretFree(status, canaries);
      const statuses = await bridge.invoke("list_provider_credential_statuses");
      assertSecretFree(statuses, canaries);
      const openAi = requireArray(statuses, "provider credential statuses").find(
        (candidate) =>
          typeof candidate === "object" &&
          candidate !== null &&
          !Array.isArray(candidate) &&
          (candidate as { provider?: unknown }).provider === "openai",
      );
      assertProviderStatus(openAi, true, "keychain");
    }
  } finally {
    const deleted = await bridge.invoke("delete_provider_credential", {
      provider: "openai",
    });
    assertProviderStatus(deleted, false, "missing");
    assertSecretFree(deleted, canaries);
  }
}

async function runAgentMcpSkills(
  bridge: SettingsAcceptanceBridge,
  projectRoot: string,
): Promise<void> {
  await bridge.invoke("get_agent_settings_health");
  const mcp = requireRecord(
    await bridge.invoke("mcp_client_configuration", {
      activeProjectDir: projectRoot,
    }),
    "MCP configuration",
  );
  if (typeof mcp.executable !== "string" || mcp.executable.length === 0) {
    throw new Error("packaged MCP executable was not configured");
  }
  await bridge.invoke("get_skills_settings_health", { projectRoot });
  const skillIds = [
    "video-creater-video-pipeline",
    "video-creater-graphics",
    "video-creater-visuals",
  ];
  const preview = requireRecord(
    await bridge.invoke("repair_bundled_skills", {
      projectRoot,
      skillIds,
      confirmedPaths: null,
    }),
    "skill repair preview",
  );
  const previewPayload = requireRecord(preview.preview, "skill repair preview payload");
  if (
    !Array.isArray(previewPayload.affectedPaths) ||
    !previewPayload.affectedPaths.every((path) => typeof path === "string")
  ) {
    throw new Error("skill repair preview paths are invalid");
  }
  const confirmedPaths = previewPayload.affectedPaths as string[];
  if (confirmedPaths.length > 0) {
    const repaired = requireRecord(
      await bridge.invoke("repair_bundled_skills", {
        projectRoot,
        skillIds,
        confirmedPaths,
      }),
      "skill repair result",
    );
    if (!repaired.report) {
      throw new Error("scoped skill repair did not return a report");
    }
  }
}

async function waitForCategoryButton(
  bridge: SettingsAcceptanceBridge,
  category: (typeof SETTINGS_CATEGORIES)[number],
): Promise<HTMLButtonElement> {
  const tabId = `settings-tab-${category.id}`;
  for (let attempt = 0; attempt < SETTINGS_ACCEPTANCE_UI_WAIT_ATTEMPTS; attempt += 1) {
    const candidate = bridge.document.getElementById(tabId);
    if (candidate) {
      if (!(candidate instanceof HTMLButtonElement) || candidate.type !== "button") {
        throw new Error(
          `Settings category tab is invalid: ${category.label} (${tabId})`,
        );
      }
      return candidate;
    }
    await bridge.sleep(100);
  }
  throw new Error(`Settings category did not render: ${category.label} (${tabId})`);
}

async function waitForElement(
  bridge: SettingsAcceptanceBridge,
  selector: string,
): Promise<Element> {
  for (let attempt = 0; attempt < SETTINGS_ACCEPTANCE_UI_WAIT_ATTEMPTS; attempt += 1) {
    const candidate = bridge.document.querySelector(selector);
    if (candidate) return candidate;
    await bridge.sleep(100);
  }
  throw new Error(`Settings acceptance element did not render: ${selector}`);
}

async function waitForCondition(
  bridge: SettingsAcceptanceBridge,
  predicate: () => boolean,
  failure: string,
): Promise<void> {
  for (let attempt = 0; attempt < SETTINGS_ACCEPTANCE_UI_WAIT_ATTEMPTS; attempt += 1) {
    if (predicate()) return;
    await bridge.sleep(100);
  }
  throw new Error(failure);
}

function setInputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set;
  if (!setter) throw new Error("browser input value setter is unavailable");
  setter.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

async function waitForOperation(
  bridge: SettingsAcceptanceBridge,
  operationId: string,
  predicate: (operation: SettingsOperation) => boolean,
): Promise<SettingsOperation> {
  for (let attempt = 0; attempt < 1_800; attempt += 1) {
    const operation = requireOperations(
      await bridge.invoke("list_settings_operations"),
    ).find((candidate) => candidate.id === operationId);
    if (operation && predicate(operation)) {
      return operation;
    }
    if (operation?.state === "failed") {
      const code = operation.error?.code ?? "unknown";
      throw new Error(`Settings operation failed during acceptance (${code})`);
    }
    await bridge.sleep(250);
  }
  throw new Error("Settings operation timed out during acceptance");
}

function passed(id: AcceptanceCheck["id"]): AcceptanceCheck {
  return {
    id,
    status: "passed",
    diagnosticCode: `settings.acceptance.${id}.passed`,
  };
}

async function writeCheckpoint(
  bridge: SettingsAcceptanceBridge,
  stage: AcceptanceStage,
  checks: AcceptanceCheck[],
  productContract?: FinalSettingsAcceptanceReport,
): Promise<void> {
  await bridge.invoke("write_settings_acceptance_checkpoint", {
    checkpoint: { stage, checks, ...(productContract ? { productContract } : {}) },
  });
}

function isAcceptanceContext(value: unknown): value is SettingsAcceptanceContext {
  return (
    typeof value === "object" &&
    value !== null &&
    !Array.isArray(value) &&
    ((value as { stage?: unknown }).stage === "pre_restart" ||
      (value as { stage?: unknown }).stage === "post_restart") &&
    typeof (value as { projectRoot?: unknown }).projectRoot === "string" &&
    acceptanceRunNameOrNull((value as { projectRoot: string }).projectRoot) !== null
  );
}

function acceptanceRunName(projectRoot: string): string {
  const runName = acceptanceRunNameOrNull(projectRoot);
  if (!runName) {
    throw new Error("settings acceptance project root is outside the reviewed boundary");
  }
  return runName;
}

function acceptanceRunNameOrNull(projectRoot: string): string | null {
  return (
    /^\/private\/tmp\/(video-creater-settings-acceptance-[a-z0-9-]+)\/projects\/acceptance-project$/.exec(
      projectRoot,
    )?.[1] ?? null
  );
}

function requireModels(value: unknown): ModelStatus[] {
  if (!Array.isArray(value)) {
    throw new Error("transcription model response is invalid");
  }
  return value.map(requireModel);
}

function requireModel(value: unknown): ModelStatus {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof (value as { modelId?: unknown }).modelId !== "string" ||
    typeof (value as { installStatus?: unknown }).installStatus !== "string" ||
    typeof (value as { runtimeId?: unknown }).runtimeId !== "string"
  ) {
    throw new Error("transcription model response is invalid");
  }
  return value as ModelStatus;
}

function requireOperation(value: unknown): SettingsOperation {
  if (
    typeof value !== "object" ||
    value === null ||
    Array.isArray(value) ||
    typeof (value as { id?: unknown }).id !== "string" ||
    typeof (value as { kind?: unknown }).kind !== "string" ||
    typeof (value as { targetId?: unknown }).targetId !== "string" ||
    typeof (value as { state?: unknown }).state !== "string" ||
    typeof (value as { phase?: unknown }).phase !== "string" ||
    typeof (value as { completedUnits?: unknown }).completedUnits !== "number" ||
    !isOperationError((value as { error?: unknown }).error)
  ) {
    throw new Error("Settings operation response is invalid");
  }
  return value as SettingsOperation;
}

function isOperationError(value: unknown): value is SettingsOperation["error"] {
  return (
    value === null ||
    (typeof value === "object" &&
      !Array.isArray(value) &&
      typeof (value as { code?: unknown }).code === "string")
  );
}

function requireProductionSpeechModelStatus(value: unknown): ProductionSpeechModelStatus {
  const status = requireRecord(value, "production speech model status");
  const validNumbers = [
    status.installedFiles,
    status.totalFiles,
    status.installedBytes,
    status.totalBytes,
  ].every((candidate) =>
    typeof candidate === "number" && Number.isFinite(candidate) && candidate >= 0
  );
  const validStrings = [
    status.rootPath,
    status.vadRepo,
    status.vadRevision,
    status.diarizationRepo,
    status.diarizationRevision,
    status.artifactFormat,
  ].every((candidate) => typeof candidate === "string" && candidate.length > 0);
  if (
    status.modelSetId !== SPEECH_MODEL_SET_ID ||
    status.runtimeId !== SPEECH_RUNTIME_ID ||
    typeof status.ready !== "boolean" ||
    !validNumbers ||
    !validStrings ||
    !Array.isArray(status.licenses) ||
    status.licenses.length === 0 ||
    !status.licenses.every((license) => typeof license === "string" && license.length > 0) ||
    !(status.lastErrorCode === null || typeof status.lastErrorCode === "string") ||
    (status.ready &&
      (status.installedFiles !== status.totalFiles ||
        status.installedBytes !== status.totalBytes ||
        status.totalFiles === 0 ||
        status.lastErrorCode !== null))
  ) {
    throw new Error("production speech model status is invalid");
  }
  return status as unknown as ProductionSpeechModelStatus;
}

function requireOperations(value: unknown): SettingsOperation[] {
  if (!Array.isArray(value)) {
    throw new Error("Settings operations response is invalid");
  }
  return value.map(requireOperation);
}

function requireSystemHealthSections(value: unknown): FinalSettingsHealthSectionEvidence[] {
  const snapshot = requireRecord(value, "system health snapshot");
  const sections = requireRecord(snapshot.sections, "system health sections");
  return Object.values(sections).map((value) => {
    const section = requireRecord(value, "system health section");
    if (
      typeof section.id !== "string" ||
      typeof section.required !== "boolean" ||
      !Array.isArray(section.items)
    ) {
      throw new Error("system health section response is invalid");
    }
    const items = section.items.map((value) => {
      const item = requireRecord(value, "system health item");
      if (typeof item.id !== "string") {
        throw new Error("system health item response is invalid");
      }
      return { id: item.id };
    });
    return { id: section.id, required: section.required, items };
  });
}

function requireRecord(value: unknown, label: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label} response is invalid`);
  }
  return value as Record<string, unknown>;
}

function requireArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new Error(`${label} response is invalid`);
  }
  return value;
}

function assertProviderStatus(
  value: unknown,
  configured: boolean,
  source: string,
): void {
  const status = requireRecord(value, "provider credential status");
  if (
    status.provider !== "openai" ||
    status.configured !== configured ||
    status.source !== source
  ) {
    throw new Error("provider credential status did not match the isolated Keychain action");
  }
}

function assertSecretFree(value: unknown, canaries: readonly string[]): void {
  const serialized = JSON.stringify(value);
  if (canaries.some((canary) => serialized.includes(canary))) {
    throw new Error("provider credential secret appeared in an IPC response");
  }
}
