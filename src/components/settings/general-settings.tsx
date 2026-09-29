import { useEffect, useState, type ReactNode } from "react";
import { Bell, Loader2, ShieldCheck } from "lucide-react";

import { Button } from "@/components/ui/button";
import type {
  AppPreferencesPatch,
  AppSettingsPreferences,
} from "@/lib/app-settings";
import {
  getNotificationCapability,
  requestNotificationPermission,
  type NotificationCapability,
} from "@/lib/settings/render-system";
import { useHostPlatformCopy } from "@/lib/runtime/platform";
import { useRuntimeMode } from "@/editor/services/use-runtime-mode";

export interface GeneralSettingsProps {
  preferences: AppSettingsPreferences;
  onPreferencesChange: (preferences: AppPreferencesPatch) => void;
}

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : String(error);
}

function SettingRow({
  title,
  description,
  icon,
  children,
}: {
  title: string;
  description: string;
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="grid gap-3 border-t py-3 md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)] md:items-center">
      <div className="flex min-w-0 items-start gap-2">
        <span className="mt-0.5 text-muted-foreground" aria-hidden="true">
          {icon}
        </span>
        <div>
          <h2 className="text-xs font-semibold text-foreground">{title}</h2>
          <p className="text-[11px] leading-5 text-muted-foreground">
            {description}
          </p>
        </div>
      </div>
      <div className="flex min-w-0 flex-wrap items-center gap-2 md:justify-end">
        {children}
      </div>
    </div>
  );
}

function notificationLabel(
  capability: NotificationCapability | null,
  enabled: boolean,
) {
  if (!capability) return "Checking";
  if (enabled && capability.deliveryAvailable) return "Enabled";
  if (capability.permissionStatus === "denied") return "Permission denied";
  if (capability.permissionStatus === "unavailable") return "Unavailable";
  return "Not enabled";
}

function isNotificationPermissionAuthorized(
  status: NotificationCapability["permissionStatus"] | undefined,
) {
  return status === "authorized" || status === "provisional" || status === "ephemeral";
}

export function GeneralSettings({
  preferences,
  onPreferencesChange,
}: GeneralSettingsProps) {
  const [capability, setCapability] =
    useState<NotificationCapability | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [requestError, setRequestError] = useState<string | null>(null);
  const [requesting, setRequesting] = useState(false);
  const platformCopy = useHostPlatformCopy();
  const runtimeMode = useRuntimeMode();

  useEffect(() => {
    let cancelled = false;
    setLoadError(null);
    void getNotificationCapability()
      .then((next) => {
        if (!cancelled) setCapability(next);
      })
      .catch((error) => {
        if (!cancelled) setLoadError(errorMessage(error));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  async function enableNotifications() {
    setRequesting(true);
    setRequestError(null);
    try {
      const next = await requestNotificationPermission();
      setCapability(next);
      if (isNotificationPermissionAuthorized(next.permissionStatus) && next.deliveryAvailable) {
        onPreferencesChange({ renderCompletionNotifications: true });
      } else {
        setRequestError(next.summary);
      }
    } catch (error) {
      setRequestError(errorMessage(error));
    } finally {
      setRequesting(false);
    }
  }

  const notificationsEnabled =
    preferences.renderCompletionNotifications &&
    isNotificationPermissionAuthorized(capability?.permissionStatus) &&
    capability.deliveryAvailable;
  const canRequest = capability?.canRequest === true;
  const canToggle =
    isNotificationPermissionAuthorized(capability?.permissionStatus) &&
    capability.deliveryAvailable;

  return (
    <section aria-label="General settings" className="grid text-xs">
      <div
        data-settings-target="general:privacy"
        tabIndex={-1}
        className="outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <SettingRow
          title="Provider upload confirmation"
          description="Ask before project media is sent to a configured remote provider."
          icon={<ShieldCheck className="h-4 w-4" />}
        >
          <label className="flex items-center gap-2 font-medium text-foreground">
            <input
              type="checkbox"
              aria-label="Confirm provider uploads"
              checked={preferences.requireProviderUploadConfirmation}
              onChange={(event) =>
                onPreferencesChange({
                  requireProviderUploadConfirmation: event.currentTarget.checked,
                })
              }
              className="h-4 w-4 accent-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            Confirm every upload
          </label>
        </SettingRow>
      </div>

      <div
        data-settings-target="general:notifications"
        tabIndex={-1}
        className="outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <SettingRow
          title="Render completion notifications"
          description={runtimeMode === "browser"
            ? "Show a browser notification when an export finishes. Background tasks remains the source of truth."
            : platformCopy.notificationDescription}
          icon={<Bell className="h-4 w-4" />}
        >
          <span className="text-[11px] font-medium text-muted-foreground">
            {notificationLabel(capability, notificationsEnabled)}
          </span>
          {canRequest ? (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 px-2"
              disabled={requesting}
              onClick={() => void enableNotifications()}
              aria-label="Enable notifications"
            >
              {requesting ? (
                <Loader2
                  className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none"
                  aria-hidden="true"
                />
              ) : null}
              Enable
            </Button>
          ) : (
            <label className="flex items-center gap-2 font-medium text-foreground">
              <input
                type="checkbox"
                aria-label="Render completion notifications"
                checked={notificationsEnabled}
                disabled={!canToggle}
                onChange={(event) =>
                  onPreferencesChange({
                    renderCompletionNotifications: event.currentTarget.checked,
                  })
                }
                className="h-4 w-4 accent-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              />
              Notify after renders
            </label>
          )}
        </SettingRow>
      </div>

      {loadError ? (
        <p role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">
          Notification preferences could not be loaded: {loadError}
        </p>
      ) : null}
      {requestError ? (
        <p role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">
          {requestError}
        </p>
      ) : null}
    </section>
  );
}
