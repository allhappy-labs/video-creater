import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getRemoteAccessStatus,
  setRemoteAccessRunning,
  type RemoteAccessStatus,
} from "@/lib/remote-access";
import {
  discoverRemoteSession,
  listRemoteDeviceSessions,
  revokeRemoteDeviceSession,
} from "@/lib/runtime/remote-session";
import { RemoteAccessSettings } from "./remote-access-settings";

vi.mock("@/lib/remote-access", () => ({
  getRemoteAccessStatus: vi.fn(),
  setRemoteAccessRunning: vi.fn(),
}));
let runtimeMode: "desktop" | "browser" = "desktop";
vi.mock("@/editor/services/use-runtime-mode", () => ({
  useRuntimeMode: () => runtimeMode,
}));
vi.mock("@/lib/runtime/remote-session", () => ({
  discoverRemoteSession: vi.fn(),
  listRemoteDeviceSessions: vi.fn(),
  revokeRemoteDeviceSession: vi.fn(),
}));

const mockGetRemoteAccessStatus = vi.mocked(getRemoteAccessStatus);
const mockSetRemoteAccessRunning = vi.mocked(setRemoteAccessRunning);
const mockDiscoverRemoteSession = vi.mocked(discoverRemoteSession);
const mockListRemoteDeviceSessions = vi.mocked(listRemoteDeviceSessions);
const mockRevokeRemoteDeviceSession = vi.mocked(revokeRemoteDeviceSession);

function status(overrides: Partial<RemoteAccessStatus> = {}): RemoteAccessStatus {
  return {
    serviceState: "running",
    managementAvailable: true,
    detail: "The user service is running.",
    tailscale: {
      state: "ready",
      dnsName: "studio.tailnet.example",
      tailnetIp: "100.64.0.1",
      url: "https://studio.tailnet.example",
      healthVerified: true,
      detail: "Tailscale Serve reaches this host with the expected protocol.",
    },
    ...overrides,
  };
}

describe("RemoteAccessSettings", () => {
  beforeEach(() => {
    mockGetRemoteAccessStatus.mockReset();
    mockSetRemoteAccessRunning.mockReset();
    mockGetRemoteAccessStatus.mockResolvedValue(status());
    runtimeMode = "desktop";
    mockDiscoverRemoteSession.mockReset();
    mockListRemoteDeviceSessions.mockReset();
    mockRevokeRemoteDeviceSession.mockReset();
  });

  it("shows only a health-verified tailnet URL", async () => {
    render(<RemoteAccessSettings />);

    expect(await screen.findByRole("heading", { name: "Verified tailnet URL" })).toBeVisible();
    expect(screen.getByRole("link", { name: "https://studio.tailnet.example" })).toHaveAttribute(
      "href",
      "https://studio.tailnet.example",
    );
    expect(screen.queryByText("100.64.0.1")).not.toBeInTheDocument();
    expect(document.body).not.toHaveTextContent(/secret-cookie-value|x-webauth-user|\/home\/operator/i);
  });

  it("withholds an unverified URL and disables controls when host management is unavailable", async () => {
    mockGetRemoteAccessStatus.mockResolvedValue(
      status({
        serviceState: "unavailable",
        managementAvailable: false,
        tailscale: {
          ...status().tailscale,
          state: "health_unreachable",
          healthVerified: false,
          url: "https://unverified.tailnet.example",
        },
      }),
    );

    render(<RemoteAccessSettings />);

    expect(await screen.findByRole("heading", { name: "Tailnet route not ready" })).toBeVisible();
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  });

  it("starts a stopped installed service and renders the returned state", async () => {
    mockGetRemoteAccessStatus.mockResolvedValue(
      status({ serviceState: "stopped", detail: "The user service is installed but is not running." }),
    );
    mockSetRemoteAccessRunning.mockResolvedValue(status());

    render(<RemoteAccessSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Start" }));

    await waitFor(() => expect(mockSetRemoteAccessRunning).toHaveBeenCalledWith(true));
    expect(await screen.findByText("The user service is running.")).toBeVisible();
  });

  it("stops a running service", async () => {
    mockSetRemoteAccessRunning.mockResolvedValue(
      status({ serviceState: "stopped", detail: "The user service is installed but is not running." }),
    );

    render(<RemoteAccessSettings />);
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));

    await waitFor(() => expect(mockSetRemoteAccessRunning).toHaveBeenCalledWith(false));
    expect(await screen.findByText("The user service is installed but is not running.")).toBeVisible();
  });

  it("lists and revokes paired browsers from the remote settings surface", async () => {
    runtimeMode = "browser";
    mockDiscoverRemoteSession.mockResolvedValue({
      kind: "connected",
      sessionId: "session-1",
      displayName: "Laptop",
      hostLabel: "Studio host",
      csrfToken: "csrf-1",
    });
    mockListRemoteDeviceSessions
      .mockResolvedValueOnce([{
        sessionId: "session-2",
        displayName: "Phone",
        identity: "person@example.test",
        createdAt: 100,
        expiresAt: 200,
        revoked: false,
        current: false,
      }])
      .mockResolvedValueOnce([]);
    mockRevokeRemoteDeviceSession.mockResolvedValue();

    render(<RemoteAccessSettings />);
    expect(await screen.findByText("Phone")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Revoke" }));

    await waitFor(() => expect(mockRevokeRemoteDeviceSession)
      .toHaveBeenCalledWith("session-2", "csrf-1"));
  });
});
