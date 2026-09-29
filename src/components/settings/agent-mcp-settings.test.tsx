import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getAgentHealth,
  getMcpClientConfiguration,
  runAgentComponentSelfTest,
} from "@/lib/settings/agent";
import { AgentMcpSettings } from "./agent-mcp-settings";

vi.mock("@/lib/settings/agent", () => ({
  getAgentHealth: vi.fn(),
  getMcpClientConfiguration: vi.fn(),
  runAgentComponentSelfTest: vi.fn(),
}));

const mockGetAgentHealth = vi.mocked(getAgentHealth);
const mockGetMcpClientConfiguration = vi.mocked(getMcpClientConfiguration);
const mockRunAgentComponentSelfTest = vi.mocked(runAgentComponentSelfTest);

describe("AgentMcpSettings", () => {
  beforeEach(() => {
    mockGetAgentHealth.mockReset();
    mockRunAgentComponentSelfTest.mockReset();
    mockGetMcpClientConfiguration.mockReset().mockResolvedValue({
      actionLabel: "Copy client configuration",
      configuration:
        '{"mcpServers":{"video-creater":{"command":"/Applications/Video Creater.app/video-creater-mcp-server","args":["--project-dir","/projects/current"]}}}',
      executable: "/Applications/Video Creater.app/video-creater-mcp-server",
      projectDir: "/projects/current",
    });
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockResolvedValue(undefined) },
    });
  });

  it("shows a neutral no-project state without probing project MCP", () => {
    render(<AgentMcpSettings projectRoot={null} />);

    expect(
      screen.getByText("Open a project to copy MCP configuration."),
    ).toBeVisible();
    expect(mockGetMcpClientConfiguration).not.toHaveBeenCalled();
    expect(screen.queryByText("Action required")).not.toBeInTheDocument();
  });

  it("loads and copies configuration pinned to the active project", async () => {
    render(<AgentMcpSettings projectRoot="/projects/current" />);

    expect(mockGetMcpClientConfiguration).toHaveBeenCalledWith(
      "/projects/current",
    );
    fireEvent.click(
      await screen.findByRole("button", {
        name: "Copy client configuration",
      }),
    );
    await waitFor(() =>
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith(
        expect.stringContaining("/projects/current"),
      ),
    );
    expect(screen.getByText("Copied")).toBeVisible();
  });

  it("keeps self-tests and raw protocol diagnostics out of App Settings", async () => {
    render(<AgentMcpSettings projectRoot="/projects/current" />);

    await screen.findByRole("button", { name: "Copy client configuration" });
    expect(mockGetAgentHealth).not.toHaveBeenCalled();
    expect(mockRunAgentComponentSelfTest).not.toHaveBeenCalled();
    expect(screen.queryByText(/self-test/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/protocol version/i)).not.toBeInTheDocument();
  });

  it("ignores stale clipboard completion after the project changes", async () => {
    let resolveClipboard!: () => void;
    const writeText = vi.fn().mockReturnValue(
      new Promise<void>((resolve) => {
        resolveClipboard = resolve;
      }),
    );
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const view = render(<AgentMcpSettings projectRoot="/projects/current" />);
    fireEvent.click(
      await screen.findByRole("button", {
        name: "Copy client configuration",
      }),
    );
    view.rerender(<AgentMcpSettings projectRoot="/projects/next" />);

    await act(async () => {
      resolveClipboard();
      await Promise.resolve();
    });

    expect(screen.queryByText("Copied")).not.toBeInTheDocument();
  });
});
