import { backendRequest } from "@/lib/runtime/backend-client";
import { getRuntimeMode } from "@/lib/runtime/runtime-mode";

import type {
  SettingsComponentHealth,
  SettingsHealthState,
} from "./health";
import type { SettingsOperation } from "./operations";

export interface RenderSystemHealth {
  checkedAt: string;
  state: SettingsHealthState;
  compositionReady: boolean;
  nativeDeliveryDegraded: boolean;
  compatibilityDegraded: boolean;
  items: SettingsComponentHealth[];
}

export interface UpdateHealth {
  state: SettingsHealthState;
  installedVersion: string;
  summary: string;
}

type NotificationPermissionStatus =
  | "notDetermined"
  | "denied"
  | "authorized"
  | "provisional"
  | "ephemeral"
  | "unavailable";

export interface NotificationCapability {
  state: SettingsHealthState;
  deliveryAvailable: boolean;
  permissionStatus: NotificationPermissionStatus;
  canRequest: boolean;
  summary: string;
  diagnosticCode: string | null;
  diagnosticDetail: string | null;
}

export function getRenderSystemHealth(): Promise<RenderSystemHealth> {
  return backendRequest<RenderSystemHealth>("get_render_system_health");
}

export function checkRenderSystem(): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("check_render_system");
}

export function getUpdateHealth(): Promise<UpdateHealth> {
  return backendRequest<UpdateHealth>("get_update_health");
}

export function getNotificationCapability(): Promise<NotificationCapability> {
  if (getRuntimeMode() === "browser") return Promise.resolve(browserNotificationCapability());
  return backendRequest<NotificationCapability>("get_notification_capability");
}

export async function requestNotificationPermission(): Promise<NotificationCapability> {
  if (getRuntimeMode() === "browser") {
    if (!browserNotifications()) return browserNotificationCapability();
    await Notification.requestPermission();
    return browserNotificationCapability();
  }
  return backendRequest<NotificationCapability>("request_notification_permission");
}

/** Delivers only after the browser has already granted permission; it never prompts implicitly. */
export function showBrowserCompletionNotification(title: string): boolean {
  if (getRuntimeMode() !== "browser" || !browserNotifications() || Notification.permission !== "granted") return false;
  new Notification(title, { body: "The export is ready to download." });
  return true;
}

function browserNotifications(): boolean {
  return typeof window !== "undefined" && "Notification" in window;
}

function browserNotificationCapability(): NotificationCapability {
  if (!browserNotifications()) {
    return {
      state: "unavailable",
      deliveryAvailable: false,
      permissionStatus: "unavailable",
      canRequest: false,
      summary: "Browser notifications are unavailable here.",
      diagnosticCode: "notifications.browserUnavailable",
      diagnosticDetail: "Use Background tasks to follow render progress.",
    };
  }
  if (Notification.permission === "granted") {
    return {
      state: "ready",
      deliveryAvailable: true,
      permissionStatus: "authorized",
      canRequest: false,
      summary: "Browser notifications are authorized.",
      diagnosticCode: null,
      diagnosticDetail: null,
    };
  }
  const denied = Notification.permission === "denied";
  return {
    state: "actionRequired",
    deliveryAvailable: !denied,
    permissionStatus: denied ? "denied" : "notDetermined",
    canRequest: !denied,
    summary: denied
      ? "Browser notifications are blocked in site settings."
      : "Browser notification permission has not been requested.",
    diagnosticCode: denied ? "notifications.permissionDenied" : null,
    diagnosticDetail: denied ? "Enable notifications for this site in your browser settings." : null,
  };
}
