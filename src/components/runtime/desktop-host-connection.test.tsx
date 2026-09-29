import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { DesktopHostConnection } from "./desktop-host-connection";
import { pairRemoteDevice } from "@/lib/runtime/remote-session";

vi.mock("@/lib/runtime/remote-session", async (importOriginal) => ({
  ...await importOriginal<typeof import("@/lib/runtime/remote-session")>(),
  pairRemoteDevice: vi.fn(),
}));

describe("DesktopHostConnection", () => {
  beforeEach(() => vi.mocked(pairRemoteDevice).mockReset());

  it("offers same-origin device pairing without asking for a server address", () => {
    render(<DesktopHostConnection />);

    const main = screen.getByRole("main", {
      name: "Desktop host connection",
    });
    expect(main).toHaveTextContent("Pair this device");
    expect(screen.getByLabelText("Pairing code")).toHaveAttribute("inputmode", "numeric");
    expect(screen.getByLabelText("Device name")).toBeVisible();
    expect(screen.getByRole("button", { name: "Pair device" })).toBeDisabled();
    expect(screen.queryByLabelText(/server|host address|url/i)).not.toBeInTheDocument();
    expect(main).not.toHaveTextContent(/macOS|Windows|Linux/i);
  });

  it("pairs a named device and hands the authenticated session back to startup", async () => {
    const onPaired = vi.fn();
    vi.mocked(pairRemoteDevice).mockResolvedValue({
      kind: "connected",
      sessionId: "session-1",
      displayName: "Kitchen iPad",
      hostLabel: "Studio host",
      csrfToken: "csrf-1",
    });
    render(<DesktopHostConnection onPaired={onPaired} />);

    fireEvent.change(screen.getByLabelText("Pairing code"), { target: { value: "123456" } });
    fireEvent.change(screen.getByLabelText("Device name"), { target: { value: "Kitchen iPad" } });
    fireEvent.click(screen.getByRole("button", { name: "Pair device" }));

    await waitFor(() => expect(pairRemoteDevice).toHaveBeenCalledWith("123456", "Kitchen iPad"));
    expect(onPaired).toHaveBeenCalledOnce();
  });

  it("blocks an incompatible browser protocol with an actionable explanation", () => {
    render(<DesktopHostConnection initialState={{ kind: "incompatible", hostProtocolVersion: 2 }} />);

    expect(screen.getByRole("heading", { name: "Update required" })).toBeVisible();
    expect(screen.getByText(/host protocol 2/i)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Pair device" })).not.toBeInTheDocument();
  });
});
