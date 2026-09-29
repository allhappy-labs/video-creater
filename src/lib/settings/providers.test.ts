import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getProviderHealth,
  providerIntegrationGroup,
  refreshProviderHealth,
} from "./providers";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: invokeMock }));

describe("Provider settings Tauri adapters", () => {
  beforeEach(() => invokeMock.mockReset());

  it("loads only the non-secret provider health projection", async () => {
    invokeMock.mockResolvedValue([]);

    await getProviderHealth();

    expect(invokeMock).toHaveBeenCalledWith("get_provider_health");
  });

  it("scopes provider refresh and forwards only dependency preferences", async () => {
    invokeMock.mockResolvedValue({ id: "refresh-openai" });

    await refreshProviderHealth("openai", ["google:veo-3"]);

    expect(invokeMock).toHaveBeenCalledWith("refresh_provider_health", {
      provider: "openai",
      disabledGenerationModelIds: ["google:veo-3"],
    });
    expect(JSON.stringify(invokeMock.mock.calls)).not.toContain("credential");
  });

  it("classifies untouched credentials as available integrations", () => {
    expect(providerIntegrationGroup({ configured: false })).toBe("available");
  });

  it("prefers live Keychain status over divergent cached health", () => {
    expect(
      providerIntegrationGroup({ configured: false }, { configured: true }),
    ).toBe("configured");
    expect(
      providerIntegrationGroup({ configured: true }, { configured: false }),
    ).toBe("available");
  });
});
