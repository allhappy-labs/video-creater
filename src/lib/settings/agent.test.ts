import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getAgentHealth,
  getMcpClientConfiguration,
  runAgentComponentSelfTest,
} from "./agent";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: invokeMock }));

describe("Agent settings Tauri adapters", () => {
  beforeEach(() => invokeMock.mockReset());

  it("uses the Agent-only health command without a project or model snapshot", async () => {
    invokeMock.mockResolvedValue({ id: "agent", state: "ready", items: [] });
    await getAgentHealth("/projects/current");
    expect(invokeMock).toHaveBeenCalledWith("get_agent_settings_health");
  });

  it("invokes the independent self-test with a camel-case component ID", async () => {
    invokeMock.mockResolvedValue({ targetId: "agent.mcpServer" });
    await runAgentComponentSelfTest("agent.mcpServer");
    expect(invokeMock).toHaveBeenCalledWith("run_agent_component_self_test", {
      componentId: "agent.mcpServer",
    });
  });

  it("requests MCP configuration for the active project only", async () => {
    invokeMock.mockResolvedValue({ actionLabel: "Copy client configuration" });
    await getMcpClientConfiguration("/projects/current");
    expect(invokeMock).toHaveBeenCalledWith("mcp_client_configuration", {
      activeProjectDir: "/projects/current",
    });
  });
});
