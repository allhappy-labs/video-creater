import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { defaultAppPreferences } from "@/lib/app-settings";
import {
  getNotificationCapability,
  requestNotificationPermission,
} from "@/lib/settings/render-system";
import { GeneralSettings } from "./general-settings";

vi.mock("@/lib/settings/render-system", () => ({
  getNotificationCapability: vi.fn(),
  requestNotificationPermission: vi.fn(),
}));

const mockGetNotificationCapability = vi.mocked(getNotificationCapability);
const mockRequestNotificationPermission = vi.mocked(
  requestNotificationPermission,
);

describe("GeneralSettings", () => {
  beforeEach(() => {
    mockGetNotificationCapability.mockReset().mockResolvedValue({
      state: "actionRequired",
      deliveryAvailable: true,
      permissionStatus: "notDetermined",
      canRequest: true,
      summary: "Notification permission has not been requested.",
      diagnosticCode: null,
      diagnosticDetail: null,
    });
    mockRequestNotificationPermission.mockReset().mockResolvedValue({
      state: "ready",
      deliveryAvailable: true,
      permissionStatus: "authorized",
      canRequest: false,
      summary: "Native notifications are authorized.",
      diagnosticCode: null,
      diagnosticDetail: null,
    });
  });

  it("treats unrequested notifications as neutral", async () => {
    render(
      <GeneralSettings
        preferences={defaultAppPreferences}
        onPreferencesChange={vi.fn()}
      />,
    );

    expect(await screen.findByText("Not enabled")).toBeInTheDocument();
    expect(screen.queryByText("Action required")).not.toBeInTheDocument();
  });

  it("contains preferences only", async () => {
    render(
      <GeneralSettings
        preferences={defaultAppPreferences}
        onPreferencesChange={vi.fn()}
      />,
    );

    await screen.findByText("Not enabled");
    expect(screen.getByText("Provider upload confirmation")).toBeVisible();
    expect(screen.getByText("Render completion notifications")).toBeVisible();
    expect(screen.queryByText(/Updates/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Render System/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/GStreamer/i)).not.toBeInTheDocument();
  });

  it("submits privacy changes without optimistically changing accepted state", async () => {
    const onPreferencesChange = vi.fn();
    render(
      <GeneralSettings
        preferences={defaultAppPreferences}
        onPreferencesChange={onPreferencesChange}
      />,
    );

    await screen.findByText("Not enabled");
    const control = screen.getByLabelText("Confirm provider uploads");
    expect(control).toBeChecked();
    fireEvent.click(control);
    expect(onPreferencesChange).toHaveBeenCalledWith({
      requireProviderUploadConfirmation: false,
    });
    expect(control).toBeChecked();
  });

  it("requests native permission before enabling notification delivery", async () => {
    const onPreferencesChange = vi.fn();
    render(
      <GeneralSettings
        preferences={defaultAppPreferences}
        onPreferencesChange={onPreferencesChange}
      />,
    );

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable notifications" }),
    );

    await waitFor(() =>
      expect(mockRequestNotificationPermission).toHaveBeenCalledTimes(1),
    );
    expect(onPreferencesChange).toHaveBeenCalledWith({
      renderCompletionNotifications: true,
    });
  });

  it("does not persist notification enablement when delivery is unavailable", async () => {
    const onPreferencesChange = vi.fn();
    mockRequestNotificationPermission.mockResolvedValueOnce({
      state: "failed",
      deliveryAvailable: false,
      permissionStatus: "authorized",
      canRequest: false,
      summary: "Notification delivery could not be verified.",
      diagnosticCode: "notifications.settingsQueryFailed",
      diagnosticDetail: null,
    });
    render(
      <GeneralSettings
        preferences={defaultAppPreferences}
        onPreferencesChange={onPreferencesChange}
      />,
    );

    fireEvent.click(
      await screen.findByRole("button", { name: "Enable notifications" }),
    );

    expect(
      await screen.findByText("Notification delivery could not be verified."),
    ).toBeVisible();
    expect(onPreferencesChange).not.toHaveBeenCalled();
  });

  it.each(["provisional", "ephemeral"] as const)(
    "treats %s notification permission as delivery-ready",
    async (permissionStatus) => {
      mockGetNotificationCapability.mockResolvedValueOnce({
        state: "ready",
        deliveryAvailable: true,
        permissionStatus,
        canRequest: false,
        summary: "Native notifications can be delivered.",
        diagnosticCode: null,
        diagnosticDetail: null,
      });
      render(
        <GeneralSettings
          preferences={{
            ...defaultAppPreferences,
            renderCompletionNotifications: true,
          }}
          onPreferencesChange={vi.fn()}
        />,
      );

      expect(await screen.findByText("Enabled")).toBeVisible();
      expect(
        screen.getByLabelText("Render completion notifications"),
      ).toBeChecked();
    },
  );
});
