import { backendRequest } from "@/lib/runtime/backend-client";

import type { SettingsCategoryHealth } from "./health";
import type { SettingsOperation } from "./operations";

export interface SkillRepairPreview {
  skillIds: string[];
  affectedPaths: string[];
}

interface SkillRepairReport {
  repairedPaths: string[];
  backupPaths: string[];
  verification: SkillVerification[];
}

interface SkillVerification {
  id: string;
  label: string;
  path: string;
  checksum: string | null;
  bundledChecksum: string;
  loadState: "missing" | "matchesBundled" | "differs" | "unreadable";
  promptIncluded: boolean;
}

export interface SkillRepairCommandResponse {
  preview: SkillRepairPreview;
  operation: SettingsOperation | null;
  report: SkillRepairReport | null;
}

export async function getSkillsHealth(
  projectRoot: string | null,
): Promise<SettingsCategoryHealth> {
  return backendRequest<SettingsCategoryHealth>(
    "get_skills_settings_health",
    { projectRoot },
  );
}

export function repairBundledSkills(
  projectRoot: string,
  skillIds: string[],
  confirmedPaths: string[] | null,
): Promise<SkillRepairCommandResponse> {
  return backendRequest<SkillRepairCommandResponse>("repair_bundled_skills", {
    projectRoot,
    skillIds,
    confirmedPaths,
  });
}
