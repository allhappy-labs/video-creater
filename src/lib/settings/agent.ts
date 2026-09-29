import { backendRequest } from "@/lib/runtime/backend-client";

import type {
  SettingsCategoryHealth,
} from "./health";
import type { SettingsOperation } from "./operations";

export interface McpClientConfigurationState {
  actionLabel: string;
  configuration: string | null;
  executable: string;
  projectDir: string | null;
}

export async function getAgentHealth(
  _projectRoot: string | null,
): Promise<SettingsCategoryHealth> {
  return backendRequest<SettingsCategoryHealth>("get_agent_settings_health");
}

export function runAgentComponentSelfTest(
  componentId: string,
): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("run_agent_component_self_test", {
    componentId,
  });
}

export function getMcpClientConfiguration(
  activeProjectDir: string | null,
): Promise<McpClientConfigurationState> {
  return backendRequest<McpClientConfigurationState>("mcp_client_configuration", {
    activeProjectDir,
  });
}
