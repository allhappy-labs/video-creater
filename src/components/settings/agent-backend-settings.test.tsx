import "@testing-library/jest-dom/vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { defaultAppPreferences, type AppPreferences } from "@/lib/app-settings";
import { getAgentHealth, runAgentComponentSelfTest } from "@/lib/settings/agent";
import type {
  SettingsComponentHealth,
  SettingsHealthState,
} from "@/lib/settings/health";
import { AgentBackendSettings } from "./agent-backend-settings";

vi.mock("@/lib/settings/agent", () => ({
  getAgentHealth: vi.fn(),
  runAgentComponentSelfTest: vi.fn(),
}));

const mockGetAgentHealth = vi.mocked(getAgentHealth);
const mockRunAgentComponentSelfTest = vi.mocked(runAgentComponentSelfTest);

function component(
  id: string,
  label: string,
  state: SettingsHealthState,
  summary: string,
  provenance: Record<string, string> = {},
): SettingsComponentHealth {
  return {
    id,
    label,
    state,
    summary,
    actionId: `${id}.selfTest`,
    actionLabel: null,
    lastCheckedAt: "2026-09-18T12:00:00Z",
    diagnosticCode: `${id}.fixture`,
    diagnosticDetail: null,
    provenance,
  };
}

function agentHealth(codexReady: boolean, claudeReady: boolean) {
  return {
    id: "agent",
    state: "ready" as SettingsHealthState,
    items: [
      component(
        "agent.codex",
        "Codex app-server",
        codexReady ? "ready" : "notConfigured",
        codexReady
          ? "Codex app-server initialize handshake succeeded."
          : "The bundled Codex runtime is missing from this app installation. It is optional: turns can run on Claude instead.",
      ),
      component(
        "agent.claude",
        "Claude CLI",
        claudeReady ? "ready" : "notConfigured",
        claudeReady
          ? "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed."
          : "Claude isn't installed. Install it from claude.com, then sign in with your subscription.",
        claudeReady ? { executable: "/home/user/.local/bin/claude" } : {},
      ),
      component(
        "agent.mcpServer",
        "Video Creater MCP server",
        "ready",
        "MCP initialize handshake succeeded.",
      ),
    ],
  };
}

function renderSettings(
  preferences: Partial<AppPreferences> = {},
  onChange = vi.fn(),
) {
  render(
    <AgentBackendSettings
      preferences={{ ...defaultAppPreferences, ...preferences }}
      onChange={onChange}
    />,
  );
  return onChange;
}

/** Readiness is deliberately on request, so every readiness assertion asks first. */
async function checkAgents() {
  fireEvent.click(screen.getByRole("button", { name: "Check agents" }));
  await waitFor(() => expect(mockGetAgentHealth).toHaveBeenCalledWith(null));
}

describe("AgentBackendSettings", () => {
  beforeEach(() => {
    mockRunAgentComponentSelfTest.mockReset();
    mockGetAgentHealth.mockReset().mockResolvedValue(agentHealth(true, true));
  });

  it("is reachable as a settings deep-link target without starting an agent", () => {
    renderSettings();

    expect(
      document.querySelector('[data-settings-target="advanced:agent"]'),
    ).not.toBeNull();
    expect(mockGetAgentHealth).not.toHaveBeenCalled();
  });

  it("offers exactly the three backends and reports the chosen one", () => {
    const onChange = renderSettings({ agentBackend: "automatic" });
    const select = screen.getByLabelText("Agent backend");

    expect(
      [...(select as HTMLSelectElement).options].map((option) => option.text),
    ).toEqual(["Automatic", "Codex", "Claude"]);
    expect(select).toHaveValue("automatic");

    fireEvent.change(select, { target: { value: "claude" } });
    expect(onChange).toHaveBeenCalledWith({ agentBackend: "claude" });
  });

  it("names the agent an automatic choice resolves to", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(false, true));
    renderSettings({ agentBackend: "automatic" });
    await checkAgents();

    expect(await screen.findByText("Automatic is using Claude.")).toBeVisible();
  });

  it("prefers Claude when both agents are ready", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(true, true));
    renderSettings({ agentBackend: "automatic" });
    await checkAgents();

    // Mirrors resolve_agent_backend: Claude runs on the user's own subscription,
    // so Automatic sends the turn there rather than to the bundled sidecar.
    expect(await screen.findByText("Automatic is using Claude.")).toBeVisible();
  });

  it("says the Claude row is ready on a subscription without naming the account", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(false, true));
    renderSettings({ agentBackend: "automatic" });
    await checkAgents();

    expect(
      await screen.findByText(
        "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed.",
      ),
    ).toBeVisible();
    expect(document.body.textContent).not.toContain("@");
  });

  it("names Codex when it is the only agent an automatic choice can use", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(true, false));
    renderSettings({ agentBackend: "automatic" });
    await checkAgents();

    expect(await screen.findByText("Automatic is using Codex.")).toBeVisible();
  });

  it("warns when the chosen agent is not ready and another one takes the turn", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(true, false));
    renderSettings({ agentBackend: "claude" });
    await checkAgents();

    expect(
      await screen.findByText("Claude isn't ready, so turns will use Codex."),
    ).toBeVisible();
  });

  it("warns in the mirror case too", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(false, true));
    renderSettings({ agentBackend: "codex" });
    await checkAgents();

    expect(
      await screen.findByText("Codex isn't ready, so turns will use Claude."),
    ).toBeVisible();
  });

  it("says no agent is available when neither is ready", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(false, false));
    renderSettings({ agentBackend: "automatic" });
    await checkAgents();

    expect(
      await screen.findByText("No agent is available. Install Claude or Codex."),
    ).toBeVisible();
  });

  it("offers the Claude model aliases and defaults to Sonnet", () => {
    const onChange = renderSettings({ agentBackend: "claude" });
    const select = screen.getByLabelText("Claude model");

    expect(
      [...(select as HTMLSelectElement).options].map((option) => option.text),
    ).toEqual(["Sonnet", "Haiku", "Opus"]);
    expect(select).toHaveValue("sonnet");
    expect(select).toBeEnabled();

    fireEvent.change(select, { target: { value: "haiku" } });
    expect(onChange).toHaveBeenCalledWith({ claudeModel: "haiku" });
  });

  it("disables the Claude model when turns will not reach Claude", async () => {
    mockGetAgentHealth.mockResolvedValue(agentHealth(true, false));
    renderSettings({ agentBackend: "claude" });
    expect(screen.getByLabelText("Claude model")).toBeEnabled();

    await checkAgents();

    await waitFor(() =>
      expect(screen.getByLabelText("Claude model")).toBeDisabled(),
    );
  });

  it("keeps the executable path optional and trims what the user types", async () => {
    const onChange = renderSettings({ agentBackend: "claude" });
    await checkAgents();
    const field = screen.getByLabelText("Claude executable path");

    expect(field).not.toBeRequired();
    expect(field).toHaveAttribute("placeholder", "/home/user/.local/bin/claude");

    fireEvent.change(field, { target: { value: "  /opt/claude  " } });
    fireEvent.blur(field);
    expect(onChange).toHaveBeenCalledWith({
      claudeExecutablePath: "/opt/claude",
    });
  });

  it("shows both readiness rows and re-checks them on demand", async () => {
    mockRunAgentComponentSelfTest.mockResolvedValue({
      id: "operation-1",
      kind: "healthCheck",
      targetId: "agent.claude",
      phase: "ready",
      state: "succeeded",
      completedUnits: 1,
      totalUnits: 1,
      unit: "components",
      cancellable: false,
      message: "Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed.",
      error: null,
      startedAt: "2026-09-18T12:00:00Z",
      updatedAt: "2026-09-18T12:00:01Z",
    });
    renderSettings();
    await checkAgents();

    expect(
      await screen.findByText("Claude 2.1.270 is signed in with your Claude subscription (max plan). No API key is needed."),
    ).toBeVisible();
    expect(
      screen.getByText("Codex app-server initialize handshake succeeded."),
    ).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "Re-check Claude CLI" }));
    await waitFor(() =>
      expect(mockRunAgentComponentSelfTest).toHaveBeenCalledWith("agent.claude"),
    );
  });

  it("uses theme tokens rather than raw colors", () => {
    const source = readFileSync(
      resolve(process.cwd(), "src/components/settings/agent-backend-settings.tsx"),
      "utf8",
    );

    expect(source).not.toMatch(/\[#[0-9a-fA-F]{3,8}\]/);
    expect(source).not.toMatch(/\b(?:white|black)\/[\w.[\]]+/);
  });
});
