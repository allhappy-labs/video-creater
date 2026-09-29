import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  deleteProviderCredential,
  listProviderCredentialStatuses,
  setProviderCredential,
  type ProviderCredentialStatus,
} from "./provider-credentials";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: invokeMock,
}));

const storedStatus: ProviderCredentialStatus = {
  provider: "openai",
  displayName: "OpenAI",
  configured: true,
  source: "keychain",
};

describe("provider credential command adapters", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("lists typed provider credential statuses", async () => {
    invokeMock.mockResolvedValue([storedStatus]);

    const statuses = await listProviderCredentialStatuses();

    expect(statuses).toEqual([storedStatus]);
    expect(statuses[0]).not.toHaveProperty("envVar");
    expect(invokeMock).toHaveBeenCalledWith("list_provider_credential_statuses");
  });

  it("stores a provider credential without changing the command payload", async () => {
    invokeMock.mockResolvedValue(storedStatus);

    await expect(setProviderCredential("openai", "sk-local-secret")).resolves.toEqual(
      storedStatus,
    );
    expect(invokeMock).toHaveBeenCalledWith("set_provider_credential", {
      provider: "openai",
      credential: "sk-local-secret",
    });
  });

  it("deletes a provider credential by canonical provider id", async () => {
    const missingStatus: ProviderCredentialStatus = {
      ...storedStatus,
      configured: false,
      source: "missing",
    };
    invokeMock.mockResolvedValue(missingStatus);

    await expect(deleteProviderCredential("openai")).resolves.toEqual(missingStatus);
    expect(invokeMock).toHaveBeenCalledWith("delete_provider_credential", {
      provider: "openai",
    });
  });
});
