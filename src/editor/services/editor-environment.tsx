import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { defaultAppPreferences, type AppSettingsPreferences } from "@/lib/app-settings";
import type { GenerationModelCatalog } from "@/lib/generation/types";
import { isTemporalWorkerEnvironmentReport } from "@/lib/jobs/temporal-fallback";
import { getTemporalWorkerEnvironmentReport } from "@/lib/project";
import { listProviderCredentialStatuses, type ProviderCredentialStatus } from "@/lib/provider-credentials";
import type { AppSettingsTarget } from "@/lib/settings/target";
import { loadGenerationCatalog } from "./generation-service";

type ProbeStatus = "checking" | "ready" | "failed";

/** What the Generate view needs to know before it lets a generation start. */
export interface GenerationConfiguration {
  /** Enabled models by mode; null until (or unless) the backend catalog loads. */
  readonly catalog: GenerationModelCatalog | null;
  readonly providerStatuses: readonly ProviderCredentialStatus[] | undefined;
  /** Probe status per check; null outside `EditorRoot`, where nothing gates generation. */
  readonly readiness: { readonly models: ProbeStatus; readonly providers: ProbeStatus; readonly temporal: ProbeStatus } | null;
  readonly preferences: AppSettingsPreferences;
  /** Whether the Temporal worker reported ready; undefined when it never reported. */
  readonly temporalBackendReady: boolean | undefined;
}

/** App-level facts and callbacks the editor panels need from `EditorRoot` props. */
interface EditorEnvironment {
  /** The active transcription model is installed. */
  readonly transcriptionModelReady: boolean;
  /** The on-device speech analysis models (VAD and speaker diarization) are installed. */
  readonly speechModelsReady: boolean;
  /** Opens Settings on the model category. */
  openModelSettings(): void;
  readonly generation: GenerationConfiguration;
  /** Starts the generation configuration probes; the Generate view calls it when it opens. */
  probeGeneration(): void;
  /** Opens Settings at `target`, returning focus to `originElement`; null when there is no Settings host. */
  readonly openSettings: ((target: AppSettingsTarget, originElement: HTMLElement) => void) | null;
}

const isolatedGeneration: GenerationConfiguration = {
  catalog: null,
  providerStatuses: undefined,
  readiness: null,
  preferences: defaultAppPreferences,
  temporalBackendReady: undefined,
};

/** Outside `EditorRoot` (isolated panel tests) models count as ready and settings cannot open. */
const EditorEnvironmentContext = createContext<EditorEnvironment>({
  transcriptionModelReady: true,
  speechModelsReady: true,
  openModelSettings: () => undefined,
  generation: isolatedGeneration,
  probeGeneration: () => undefined,
  openSettings: null,
});

interface Probe<T> {
  readonly refreshId: number;
  readonly status: ProbeStatus;
  readonly value: T;
}

/**
 * Pre-cut configuration probes: generation catalog, provider credentials and the Temporal worker,
 * run once something asks for them (`enabled`) and re-run whenever Settings bumps
 * `configurationRefreshId`. A probe from an older refresh reads as "checking" so stale results never
 * unblock generation.
 */
function useGenerationConfiguration(appPreferences: AppSettingsPreferences | undefined, refreshId: number, enabled: boolean): GenerationConfiguration {
  const preferences = appPreferences ?? defaultAppPreferences;
  const [catalog, setCatalog] = useState<Probe<GenerationModelCatalog | null>>({ refreshId, status: "checking", value: null });
  const [providers, setProviders] = useState<Probe<readonly ProviderCredentialStatus[] | undefined>>({ refreshId, status: "checking", value: undefined });
  const [temporal, setTemporal] = useState<Probe<boolean | undefined>>({ refreshId, status: "checking", value: undefined });

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    loadGenerationCatalog(preferences)
      .then((value) => !cancelled && setCatalog((current) => (value ? { refreshId, status: "ready", value } : { refreshId, status: "failed", value: current.value })))
      .catch(() => !cancelled && setCatalog((current) => ({ refreshId, status: "failed", value: current.value })));
    return () => {
      cancelled = true;
    };
  }, [enabled, preferences, refreshId]);

  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    listProviderCredentialStatuses()
      .then((value) => !cancelled && setProviders(Array.isArray(value) ? { refreshId, status: "ready", value } : { refreshId, status: "failed", value: undefined }))
      .catch(() => !cancelled && setProviders({ refreshId, status: "failed", value: undefined }));
    getTemporalWorkerEnvironmentReport()
      .then((report) => !cancelled && setTemporal(isTemporalWorkerEnvironmentReport(report) ? { refreshId, status: "ready", value: report.ready } : { refreshId, status: "failed", value: undefined }))
      .catch(() => !cancelled && setTemporal({ refreshId, status: "failed", value: undefined }));
    return () => {
      cancelled = true;
    };
  }, [enabled, refreshId]);

  return useMemo(() => {
    const current = (probe: Probe<unknown>): ProbeStatus => (probe.refreshId === refreshId ? probe.status : "checking");
    return {
      catalog: catalog.value,
      providerStatuses: providers.value,
      readiness: appPreferences ? { models: current(catalog), providers: current(providers), temporal: current(temporal) } : null,
      preferences,
      temporalBackendReady: temporal.value,
    };
  }, [appPreferences, catalog, preferences, providers, refreshId, temporal]);
}

interface EditorEnvironmentProviderProps {
  readonly transcriptionModelReady: boolean;
  /** Defaults to ready, like isolated panels, so providers that never gate speech analysis stay usable. */
  readonly speechModelsReady?: boolean;
  readonly onOpenModelSettings: () => void;
  /** App preferences; when absent, generation readiness is not gated (as in the pre-cut editor). */
  readonly appPreferences?: AppSettingsPreferences;
  readonly configurationRefreshId?: number;
  readonly onOpenSettings?: (target: AppSettingsTarget, originElement: HTMLElement) => void;
  readonly children: ReactNode;
}

export function EditorEnvironmentProvider({
  transcriptionModelReady,
  speechModelsReady = true,
  onOpenModelSettings,
  appPreferences,
  configurationRefreshId = 0,
  onOpenSettings,
  children,
}: EditorEnvironmentProviderProps) {
  const [probing, setProbing] = useState(false);
  const generation = useGenerationConfiguration(appPreferences, configurationRefreshId, probing);
  const probeGeneration = useCallback(() => setProbing(true), []);
  const value = useMemo(
    () => ({
      transcriptionModelReady,
      speechModelsReady,
      openModelSettings: onOpenModelSettings,
      generation,
      probeGeneration,
      openSettings: onOpenSettings ?? null,
    }),
    [generation, onOpenModelSettings, onOpenSettings, probeGeneration, speechModelsReady, transcriptionModelReady],
  );
  return <EditorEnvironmentContext.Provider value={value}>{children}</EditorEnvironmentContext.Provider>;
}

export function useEditorEnvironment(): EditorEnvironment {
  return useContext(EditorEnvironmentContext);
}
