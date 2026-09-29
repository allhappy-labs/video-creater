import { backendRequest } from "@/lib/runtime/backend-client";

export type ProviderCredentialSource =
  | "keychain"
  | "missing"
  | "unavailable";

export interface ProviderCredentialStatus {
  provider: string;
  displayName: string;
  configured: boolean;
  source: ProviderCredentialSource;
}

export function listProviderCredentialStatuses(): Promise<ProviderCredentialStatus[]> {
  return backendRequest("list_provider_credential_statuses");
}

export function setProviderCredential(
  provider: string,
  credential: string,
): Promise<ProviderCredentialStatus> {
  return backendRequest("set_provider_credential", { provider, credential });
}

export function deleteProviderCredential(
  provider: string,
): Promise<ProviderCredentialStatus> {
  return backendRequest("delete_provider_credential", { provider });
}
