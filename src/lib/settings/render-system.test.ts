import { backendRequest as invoke } from "@/lib/runtime/backend-client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { installRuntimeMode } from "@/lib/runtime/runtime-mode";

import {
  checkRenderSystem,
  getNotificationCapability,
  getRenderSystemHealth,
  getUpdateHealth,
  requestNotificationPermission,
  showBrowserCompletionNotification,
} from "./render-system";

vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe("render-system settings adapter", () => {
  beforeEach(() => {
    installRuntimeMode("desktop");
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
  });

  it("uses the browser permission API only after an explicit request", async () => {
    const delivered: Array<{ title: string; body: string | undefined }> = [];
    class BrowserNotification {
      static permission: NotificationPermission = "default";
      static requestPermission = vi.fn(async () => {
        BrowserNotification.permission = "granted";
        return "granted" as NotificationPermission;
      });
      constructor(title: string, options?: NotificationOptions) {
        delivered.push({ title, body: options?.body });
      }
    }
    vi.stubGlobal("Notification", BrowserNotification);
    installRuntimeMode("browser");

    await expect(getNotificationCapability()).resolves.toMatchObject({ permissionStatus: "notDetermined", canRequest: true });
    expect(BrowserNotification.requestPermission).not.toHaveBeenCalled();
    await expect(requestNotificationPermission()).resolves.toMatchObject({ permissionStatus: "authorized", deliveryAvailable: true });
    expect(BrowserNotification.requestPermission).toHaveBeenCalledTimes(1);
    expect(showBrowserCompletionNotification("Exported demo.mp4")).toBe(true);
    expect(delivered).toEqual([{ title: "Exported demo.mp4", body: "The export is ready to download." }]);
    expect(invokeMock).not.toHaveBeenCalled();
    vi.unstubAllGlobals();
  });

  it("uses the operational render health commands", async () => {
    await getRenderSystemHealth();
    await checkRenderSystem();

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_render_system_health");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "check_render_system");
  });

  it("keeps capability queries separate from permission requests", async () => {
    await getNotificationCapability();
    await getUpdateHealth();

    expect(invokeMock).toHaveBeenNthCalledWith(1, "get_notification_capability");
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_update_health");
    expect(invokeMock).not.toHaveBeenCalledWith("request_notification_permission");

    await requestNotificationPermission();
    expect(invokeMock).toHaveBeenLastCalledWith(
      "request_notification_permission",
    );
  });
});
