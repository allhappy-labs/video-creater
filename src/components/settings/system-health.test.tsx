import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getSystemHealthSnapshot,
  refreshSystemHealthSection,
  type SystemHealthSnapshot,
} from "@/lib/settings/health";
import { SystemHealth } from "./system-health";

vi.mock("@/lib/settings/health", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/settings/health")>();
  return {
    ...actual,
    getSystemHealthSnapshot: vi.fn(),
    refreshSystemHealthSection: vi.fn(),
  };
});

vi.mock("@/lib/settings/use-settings-operations", () => ({
  useSettingsOperations: () => ({
    operations: [],
    error: null,
    hydrated: true,
    baselineOperationIds: new Set(),
    baselineTerminalOperationIds: new Set(),
  }),
}));

const mockSnapshot = vi.mocked(getSystemHealthSnapshot);
const mockRefresh = vi.mocked(refreshSystemHealthSection);

const healthySnapshot: SystemHealthSnapshot = {
  generatedAt: "2026-07-19T12:00:00Z",
  overall: "ready",
  sections: {
    rendering: {
      id: "rendering",
      label: "Rendering",
      required: true,
      state: "ready",
      items: [
        {
          id: "render.gstreamer",
          label: "GStreamer / GES",
          state: "ready",
          summary: "Rendering dependencies are available.",
          actionId: null,
          actionLabel: null,
          lastCheckedAt: "2026-07-19T12:00:00Z",
          diagnosticCode: null,
          diagnosticDetail: null,
          provenance: {},
        },
      ],
    },
    agent: {
      id: "agent",
      label: "Agent Runtime",
      required: false,
      state: "ready",
      items: [],
    },
  },
};

function renderSystemHealth(snapshot = healthySnapshot) {
  mockSnapshot.mockResolvedValueOnce(snapshot);
  return render(
    <SystemHealth projectRoot="/projects/current" onBack={vi.fn()} />,
  );
}

describe("SystemHealth", () => {
  beforeEach(() => {
    mockSnapshot.mockReset();
    mockRefresh.mockReset();
  });

  it("keeps healthy sections visible when one refresh fails", async () => {
    renderSystemHealth();
    expect(await screen.findByText("GStreamer / GES")).toBeInTheDocument();
    mockRefresh.mockRejectedValueOnce(new Error("agent probe failed"));

    fireEvent.click(
      screen.getByRole("button", { name: "Retry Agent Runtime" }),
    );

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "agent probe failed",
    );
    expect(screen.getByText("GStreamer / GES")).toBeInTheDocument();
    expect(screen.getAllByText("Ready").length).toBeGreaterThan(0);
  });
});
