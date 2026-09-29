import { backendRequest } from "@/lib/runtime/backend-client";

/** Mirrors `SettingsHealthState` in `src-tauri/src/settings/health.rs`. */
export type SettingsHealthState =
  | "ready"
  | "actionRequired"
  | "needsAction"
  | "checking"
  | "failed"
  | "notConfigured"
  | "notEnabled"
  | "notApplicable"
  | "unavailable";

export interface SettingsComponentHealth {
  id: string;
  label: string;
  state: SettingsHealthState;
  summary: string;
  actionId: string | null;
  actionLabel: string | null;
  lastCheckedAt: string;
  diagnosticCode: string | null;
  diagnosticDetail: string | null;
  provenance: Record<string, string>;
}

export interface SettingsCategoryHealth {
  id: string;
  state: SettingsHealthState;
  items: SettingsComponentHealth[];
}

export interface SettingsHealthSnapshot {
  generatedAt: string;
  overall: SettingsHealthState;
  categories: Record<string, SettingsCategoryHealth>;
}

interface SettingsHealthCommandError {
  code: string;
  message: string;
  detail: string;
}

export type SystemHealthState =
  | "ready"
  | "needsAction"
  | "failed"
  | "checking"
  | "notConfigured"
  | "notEnabled"
  | "notApplicable"
  | "unavailable";

export interface SystemHealthComponent {
  id: string;
  label: string;
  state: SystemHealthState;
  summary: string;
  actionId: string | null;
  actionLabel: string | null;
  lastCheckedAt: string;
  diagnosticCode: string | null;
  diagnosticDetail: string | null;
  provenance: Record<string, string>;
}

export interface SystemHealthSection {
  id: string;
  label: string;
  required: boolean;
  state: SystemHealthState;
  items: SystemHealthComponent[];
}

export interface SystemHealthSnapshot {
  generatedAt: string;
  overall: SystemHealthState;
  sections: Record<string, SystemHealthSection>;
}

const systemHealthSeverity: Record<SystemHealthState, number> = {
  ready: 0,
  notConfigured: 0,
  notEnabled: 0,
  notApplicable: 0,
  checking: 2,
  unavailable: 3,
  needsAction: 4,
  failed: 5,
};

function systemHealthOverall(
  sections: Iterable<SystemHealthSection>,
): SystemHealthState {
  let overall: SystemHealthState = "ready";
  for (const section of sections) {
    if (
      section.required &&
      systemHealthSeverity[section.state] > systemHealthSeverity[overall]
    ) {
      overall = section.state;
    }
  }
  return overall;
}

export function mergeSystemHealthSection(
  snapshot: SystemHealthSnapshot,
  section: SystemHealthSection,
): SystemHealthSnapshot {
  const sections = { ...snapshot.sections, [section.id]: section };
  return {
    ...snapshot,
    overall: systemHealthOverall(Object.values(sections)),
    sections,
  };
}

export function getSettingsHealthSnapshot(
  projectRoot: string | null = null,
): Promise<SettingsHealthSnapshot> {
  return backendRequest<SettingsHealthSnapshot>("get_settings_health_snapshot", {
    projectRoot,
  });
}

export function getSystemHealthSnapshot(
  projectRoot: string | null = null,
): Promise<SystemHealthSnapshot> {
  return backendRequest<SystemHealthSnapshot>("get_system_health_snapshot", {
    projectRoot,
  });
}

export function refreshSystemHealthSection(
  sectionId: string,
  projectRoot: string | null = null,
): Promise<SystemHealthSection> {
  return backendRequest<SystemHealthSection>("refresh_system_health_section", {
    sectionId,
    projectRoot,
  });
}
