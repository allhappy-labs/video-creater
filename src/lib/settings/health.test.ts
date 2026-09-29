import { backendRequest as invoke } from "@/lib/runtime/backend-client";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getSystemHealthSnapshot,
  mergeSystemHealthSection,
  refreshSystemHealthSection,
  type SystemHealthSnapshot,
} from "./health";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn() }));

const invokeMock = vi.mocked(invoke);

const snapshot: SystemHealthSnapshot = {
  generatedAt: "2026-07-19T12:00:00Z",
  overall: "ready",
  sections: {
    rendering: {
      id: "rendering",
      label: "Rendering",
      required: true,
      state: "ready",
      items: [],
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

describe("system health bridge", () => {
  beforeEach(() => invokeMock.mockReset());

  it("uses typed snapshot and section commands", async () => {
    invokeMock.mockResolvedValueOnce(snapshot).mockResolvedValueOnce({
      ...snapshot.sections.agent,
      state: "failed",
    });

    await getSystemHealthSnapshot("/projects/current");
    await refreshSystemHealthSection("agent", "/projects/current");

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_system_health_snapshot", {
      projectRoot: "/projects/current",
    });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "refresh_system_health_section", {
      sectionId: "agent",
      projectRoot: "/projects/current",
    });
  });

  it("keeps overall ready when an optional refreshed section fails", () => {
    const agent = snapshot.sections.agent;
    if (!agent) throw new Error("agent fixture section missing");
    const merged = mergeSystemHealthSection(snapshot, {
      ...agent,
      state: "failed",
    });

    expect(merged.overall).toBe("ready");
    expect(merged.sections.rendering?.state).toBe("ready");
    expect(merged.sections.agent?.state).toBe("failed");
  });

  it("treats an unconfigured required section as readiness-neutral", () => {
    const rendering = snapshot.sections.rendering;
    if (!rendering) throw new Error("rendering fixture section missing");

    const merged = mergeSystemHealthSection(snapshot, {
      ...rendering,
      state: "notConfigured",
    });

    expect(merged.overall).toBe("ready");
  });
});
