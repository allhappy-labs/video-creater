import { backendRequest } from "@/lib/runtime/backend-client";

import type { ProviderCredentialSource } from "@/lib/provider-credentials";
import type { SettingsOperation } from "./operations";

type ProviderValidationState =
  | "notChecked"
  | "available"
  | "balanceUnavailable"
  | "missing"
  | "rejected"
  | "unavailable";

export interface ProviderHealth {
  provider: string;
  displayName: string;
  credentialSource: ProviderCredentialSource;
  configured: boolean;
  validationState: ProviderValidationState;
  accountLabel: string | null;
  balanceLabel: string | null;
  dependentModelIds: string[];
  lastCheckedAt: string | null;
  diagnosticCode: string | null;
}

export type ProviderIntegrationGroup = "configured" | "available";

export function providerIntegrationGroup(
  provider: Pick<ProviderHealth, "configured">,
  credentialStatus?: { configured: boolean } | null,
): ProviderIntegrationGroup {
  return (credentialStatus?.configured ?? provider.configured)
    ? "configured"
    : "available";
}

export function getProviderHealth(): Promise<ProviderHealth[]> {
  return backendRequest<ProviderHealth[]>("get_provider_health");
}

export function refreshProviderHealth(
  provider: string | null,
  disabledGenerationModelIds: string[],
): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("refresh_provider_health", {
    provider,
    disabledGenerationModelIds,
  });
}
