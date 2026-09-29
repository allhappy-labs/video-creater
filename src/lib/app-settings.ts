import { backendRequest } from "@/lib/runtime/backend-client";

type GenerationExecutionBackend = "inProcess" | "temporal";

/** Which conversation-turn agent backend to use. `automatic` resolves at turn time
 * to whichever backend is ready. */
export type AgentBackend = "automatic" | "codex" | "claude";

/** The Claude model alias; aliases resolve to the current model generation. */
export type ClaudeModel = "sonnet" | "haiku" | "opus";

export type ProjectLocationPreference =
  | { mode: "ask" }
  | { mode: "suggestedParent"; parentPath: string };

export interface NewProjectDefaults {
  width: number;
  height: number;
  fps: number;
  loudnessLufs: number;
  captions: "burn_in" | "mux" | "off";
}

export interface AppPreferences {
  schemaVersion: 2;
  projectLocation: ProjectLocationPreference;
  requireProviderUploadConfirmation: boolean;
  renderCompletionNotifications: boolean;
  newProjectDefaults: NewProjectDefaults;
  enabledGenerationModelIds: string[];
  generationExecutionBackend: GenerationExecutionBackend;
  agentBackend: AgentBackend;
  claudeModel: ClaudeModel;
  claudeExecutablePath: string;
}

export type AppPreferencesPatch = Partial<
  Omit<AppPreferences, "schemaVersion">
>;

export type AppPreferenceIntent = Partial<
  Omit<AppPreferences, "schemaVersion" | "newProjectDefaults">
> & {
  newProjectDefaults?: Partial<NewProjectDefaults>;
};

export const appSettingsStorageKey = "video-creater.appSettings.v1";

export const defaultAppPreferences: AppPreferences = {
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
  enabledGenerationModelIds: [],
  generationExecutionBackend: "inProcess",
  agentBackend: "automatic",
  claudeModel: "sonnet",
  claudeExecutablePath: "",
};

interface LegacyAppPreferencesV1 {
  projectLocation?: ProjectLocationPreference;
  requireProviderUploadConfirmation?: boolean;
  renderCompletionNotifications?: boolean;
  generationExecutionBackend?: GenerationExecutionBackend;
}

interface StoredLegacyAppPreferencesV1 {
  defaultProjectStorageLocation?: unknown;
  projectLocation?: unknown;
  requireProviderUploadConfirmation?: unknown;
  renderCompletionNotifications?: unknown;
  generationExecutionBackend?: unknown;
}

let acceptedAppPreferences: AppPreferences = defaultAppPreferences;
let appPreferencesLoadInFlight: Promise<AppPreferences> | null = null;
let appPreferencesUpdateTail: Promise<void> = Promise.resolve();

function browserStorage() {
  if (typeof window === "undefined") {
    return null;
  }
  return window.localStorage;
}

function legacyProjectLocation(
  parsed: StoredLegacyAppPreferencesV1,
): ProjectLocationPreference | undefined {
  if (isRecord(parsed.projectLocation)) {
    if (parsed.projectLocation.mode === "ask") {
      return { mode: "ask" };
    }
    if (
      parsed.projectLocation.mode === "suggestedParent" &&
      typeof parsed.projectLocation.parentPath === "string"
    ) {
      const parentPath = parsed.projectLocation.parentPath.trim();
      if (normalizeAbsoluteProjectPathForComparison(parentPath)) {
        return { mode: "suggestedParent", parentPath };
      }
      return { mode: "ask" };
    }
  }

  if (typeof parsed.defaultProjectStorageLocation === "string") {
    const parentPath = parsed.defaultProjectStorageLocation.trim();
    if (
      parentPath !== "Split project folder" &&
      normalizeAbsoluteProjectPathForComparison(parentPath)
    ) {
      return { mode: "suggestedParent", parentPath };
    }
    return { mode: "ask" };
  }

  return undefined;
}

function readLegacyPreferencesWithoutSecrets(): LegacyAppPreferencesV1 | null {
  const raw = browserStorage()?.getItem(appSettingsStorageKey);
  if (!raw) {
    return null;
  }

  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isRecord(parsed)) {
      return null;
    }

    const legacy = parsed as StoredLegacyAppPreferencesV1;
    const preferences: LegacyAppPreferencesV1 = {};
    const projectLocation = legacyProjectLocation(legacy);
    if (projectLocation) {
      preferences.projectLocation = projectLocation;
    }
    if (typeof legacy.requireProviderUploadConfirmation === "boolean") {
      preferences.requireProviderUploadConfirmation =
        legacy.requireProviderUploadConfirmation;
    }
    if (typeof legacy.renderCompletionNotifications === "boolean") {
      preferences.renderCompletionNotifications =
        legacy.renderCompletionNotifications;
    }
    if (
      legacy.generationExecutionBackend === "inProcess" ||
      legacy.generationExecutionBackend === "temporal"
    ) {
      preferences.generationExecutionBackend =
        legacy.generationExecutionBackend;
    }
    return preferences;
  } catch {
    return null;
  }
}

export function loadAppPreferences(): Promise<AppPreferences> {
  if (appPreferencesLoadInFlight) {
    return appPreferencesLoadInFlight;
  }
  const legacy = readLegacyPreferencesWithoutSecrets();
  const request = backendRequest<AppPreferences>("get_app_preferences", { legacy })
    .then((accepted) => {
      acceptedAppPreferences = accepted;
      browserStorage()?.removeItem(appSettingsStorageKey);
      return accepted;
    })
    .finally(() => {
      if (appPreferencesLoadInFlight === request) {
        appPreferencesLoadInFlight = null;
      }
    });
  appPreferencesLoadInFlight = request;
  return request;
}

export async function updateAppPreferences(
  patch: AppPreferencesPatch,
): Promise<AppPreferences> {
  const normalizedPatch =
    patch.enabledGenerationModelIds === undefined
      ? patch
      : {
          ...patch,
          enabledGenerationModelIds: normalizeGenerationModelPreferenceIds(
            patch.enabledGenerationModelIds,
          ),
        };
  const accepted = await backendRequest<AppPreferences>("update_app_preferences", {
    patch: normalizedPatch,
  });
  acceptedAppPreferences = accepted;
  return accepted;
}

function materializeAppPreferenceIntent(
  intent: AppPreferenceIntent,
): AppPreferencesPatch {
  const { newProjectDefaults, ...topLevel } = intent;
  return {
    ...topLevel,
    ...(newProjectDefaults
      ? {
          newProjectDefaults: {
            ...acceptedAppPreferences.newProjectDefaults,
            ...newProjectDefaults,
          },
        }
      : {}),
  };
}

export function updateAppPreferencesFromIntent(
  intent: AppPreferenceIntent,
): Promise<AppPreferences> {
  const request = appPreferencesUpdateTail.then(() =>
    updateAppPreferences(materializeAppPreferenceIntent(intent)),
  );
  appPreferencesUpdateTail = request.then(
    () => undefined,
    () => undefined,
  );
  return request;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function normalizeAbsoluteProjectPathForComparison(path: string) {
  const trimmedPath = path.trim();
  let kind: "posix" | "windowsDrive" | "unc";
  let root: string;
  let pathSegments: string[];
  let caseInsensitive = false;

  if (trimmedPath.startsWith("\\\\") || trimmedPath.startsWith("//")) {
    const normalizedSeparators = trimmedPath.replace(/\\/g, "/");
    const segments = normalizedSeparators.split("/").filter(Boolean);
    const [server, share, ...remainingSegments] = segments;
    if (
      !server ||
      !share ||
      server === "." ||
      server === ".." ||
      share === "." ||
      share === ".."
    ) {
      return null;
    }
    kind = "unc";
    root = `//${server}/${share}`;
    pathSegments = remainingSegments;
    caseInsensitive = true;
  } else {
    const driveMatch = trimmedPath.match(/^([A-Za-z]):[\\/]+/);
    if (driveMatch) {
      kind = "windowsDrive";
      root = `${driveMatch[1]}:/`;
      pathSegments = trimmedPath
        .slice(driveMatch[0].length)
        .replace(/\\/g, "/")
        .split("/");
      caseInsensitive = true;
    } else if (trimmedPath.startsWith("/")) {
      kind = "posix";
      root = "/";
      pathSegments = trimmedPath.slice(1).split("/");
    } else {
      return null;
    }
  }

  const resolvedSegments: string[] = [];
  for (const segment of pathSegments) {
    if (!segment || segment === ".") {
      continue;
    }
    if (segment === "..") {
      resolvedSegments.pop();
      continue;
    }
    resolvedSegments.push(segment);
  }

  const suffix = resolvedSegments.join("/");
  const normalizedPath = suffix
    ? `${root}${root.endsWith("/") ? "" : "/"}${suffix}`
    : root;
  const comparisonPath = caseInsensitive
    ? normalizedPath.toLowerCase()
    : normalizedPath;
  return `${kind}:${comparisonPath}`;
}

export type AppSettingsPreferences = AppPreferences;

export function loadAppSettingsPreferences(): AppSettingsPreferences {
  return acceptedAppPreferences;
}

export interface GenerationProviderModel {
  provider: string;
  id?: string | null;
}

export type GenerationExecutionMode = "mock" | "live";

export function generationExecutionModeForModel(
  model: GenerationProviderModel | null | undefined,
): GenerationExecutionMode {
  return model?.provider.trim().toLowerCase() === "mock" ? "mock" : "live";
}

export function generationModelPreferenceId(
  model: GenerationProviderModel | null | undefined,
) {
  const provider = model?.provider.trim();
  const id = model?.id?.trim();
  if (!provider || !id) {
    return null;
  }
  return `${provider}:${id}`;
}

export function normalizeGenerationModelPreferenceIds(
  modelIds: readonly string[],
) {
  const normalized = modelIds.flatMap((modelId) => {
    const separator = modelId.indexOf(":");
    if (separator <= 0) {
      return [];
    }
    const provider = modelId.slice(0, separator).trim();
    const id = modelId.slice(separator + 1).trim();
    return provider && id ? [`${provider}:${id}`] : [];
  });
  return [...new Set(normalized)].sort();
}

export function isGenerationModelDisabled(
  model: GenerationProviderModel | null | undefined,
  preferences: AppSettingsPreferences = loadAppSettingsPreferences(),
) {
  const key = generationModelPreferenceId(model);
  if (!key) {
    return false;
  }
  const provider = model?.provider.trim().toLowerCase();
  if (provider === "mock") {
    return false;
  }
  return !preferences.enabledGenerationModelIds.includes(key);
}
