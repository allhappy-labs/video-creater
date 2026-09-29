type ProviderId = string;

export type AppSettingsCategory =
  | "general"
  | "projects"
  | "aiModels"
  | "integrations"
  | "remoteAccess"
  | "storage"
  | "advanced";

export type AppSettingsTarget =
  | { category: "general"; item?: "privacy" | "notifications" }
  | { category: "projects"; item?: "newProjectDefaults" }
  | {
      category: "aiModels";
      item?: "transcription" | "generationModels";
    }
  | { category: "integrations"; provider?: ProviderId }
  | { category: "remoteAccess" }
  | { category: "storage"; item?: "projectLocation" | "cache" }
  | { category: "advanced"; item?: "execution" | "agent" | "mcp" | "recovery" };

export interface SettingsOrigin {
  element: HTMLElement | null;
  returnView: "home" | "editor";
}

export const appSettingsCategoryLabels: Record<AppSettingsCategory, string> = {
  general: "General",
  projects: "Projects",
  aiModels: "AI & Models",
  integrations: "Integrations",
  remoteAccess: "Remote access",
  storage: "Storage",
  advanced: "Advanced",
};

function sentenceCase(value: string) {
  return value.length === 0
    ? value
    : `${value.slice(0, 1).toUpperCase()}${value.slice(1)}`;
}

export function appSettingsTargetLabel(target: AppSettingsTarget): string {
  switch (target.category) {
    case "general":
      if (target.item === "privacy") return "Privacy settings";
      if (target.item === "notifications") return "Notification settings";
      break;
    case "projects":
      if (target.item === "newProjectDefaults") return "New project defaults";
      break;
    case "aiModels":
      if (target.item === "transcription") return "Transcription models";
      if (target.item === "generationModels") return "Generation models";
      break;
    case "integrations":
      if (target.provider) return `${sentenceCase(target.provider)} integration`;
      break;
    case "remoteAccess":
      break;
    case "storage":
      if (target.item === "projectLocation") return "Project location";
      if (target.item === "cache") return "Application cache";
      break;
    case "advanced":
      if (target.item === "execution") return "Execution settings";
      if (target.item === "agent") return "Agent settings";
      if (target.item === "mcp") return "MCP settings";
      if (target.item === "recovery") return "Recovery settings";
      break;
  }
  return appSettingsCategoryLabels[target.category];
}

export function appSettingsTargetKey(target: AppSettingsTarget): string {
  if (target.category === "integrations") {
    return target.provider
      ? `${target.category}:${target.provider}`
      : target.category;
  }
  if (target.category === "remoteAccess") {
    return target.category;
  }
  return target.item ? `${target.category}:${target.item}` : target.category;
}
