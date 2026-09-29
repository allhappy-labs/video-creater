import { useSyncExternalStore } from "react";

import { backendRequest } from "./backend-client";

export type HostPlatform = "macos" | "linux" | "windows" | "other";

export interface PlatformInfo {
  platform: HostPlatform;
}

const hostPlatforms = new Set<HostPlatform>(["macos", "linux", "windows", "other"]);

/**
 * Browser and fixture runtimes have no native host, so they keep the macOS
 * presentation that existing visual baselines were captured with.
 */
export const defaultHostPlatform: HostPlatform = "macos";

let currentHostPlatform: HostPlatform = defaultHostPlatform;
const listeners = new Set<() => void>();

function parsePlatformInfo(value: unknown): HostPlatform {
  if (typeof value === "object" && value !== null && "platform" in value) {
    const platform = (value as { platform: unknown }).platform;
    if (typeof platform === "string" && hostPlatforms.has(platform as HostPlatform)) {
      return platform as HostPlatform;
    }
  }
  throw new Error("Backend returned an unsupported platform description");
}

export async function getPlatformInfo(): Promise<PlatformInfo> {
  return { platform: parsePlatformInfo(await backendRequest<unknown>("get_platform_info")) };
}

export function getHostPlatform(): HostPlatform {
  return currentHostPlatform;
}

export function installHostPlatform(platform: HostPlatform): void {
  if (platform === currentHostPlatform) return;
  currentHostPlatform = platform;
  for (const listener of listeners) listener();
}

/** Resolves the host platform once at startup; failures keep the default. */
export async function loadHostPlatform(): Promise<HostPlatform> {
  try {
    installHostPlatform((await getPlatformInfo()).platform);
  } catch {
    // Older or disconnected backends do not describe their platform.
  }
  return currentHostPlatform;
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function useHostPlatform(): HostPlatform {
  return useSyncExternalStore(subscribe, getHostPlatform, getHostPlatform);
}

export interface HostPlatformCopy {
  /** Sentence-case name of the secure credential store, e.g. "macOS Keychain". */
  readonly credentialStore: string;
  /** Short adjective form used before "credential", e.g. "Keychain". */
  readonly credentialStoreShort: string;
  /** Status label for a stored provider credential. */
  readonly credentialSavedLabel: string;
  /** Where a removed credential is deleted from. */
  readonly credentialRemovalDetail: string;
  readonly credentialValuesHidden: string;
  /** Verb phrase for revealing a path, e.g. "Reveal … in Finder". */
  revealLabel(itemLabel: string): string;
  revealFailure(itemLabel: string, detail: string): string;
  /** Primary modifier key name for shortcut hints. */
  readonly primaryModifier: string;
  readonly transcriptionHelperReady: string;
  readonly notificationDescription: string;
}

const macosCopy: HostPlatformCopy = {
  credentialStore: "macOS Keychain",
  credentialStoreShort: "Keychain",
  credentialSavedLabel: "Saved in macOS Keychain",
  credentialRemovalDetail: "The Keychain credential will be removed from this Mac.",
  credentialValuesHidden: "Keychain values remain hidden.",
  revealLabel: (itemLabel) => `Reveal ${itemLabel} in Finder`,
  revealFailure: (itemLabel, detail) => `Finder could not reveal ${itemLabel}: ${detail}`,
  primaryModifier: "Cmd",
  transcriptionHelperReady: "The on-device Core ML helper is ready.",
  notificationDescription: "Show a native notification when a background render finishes.",
};

const desktopCopy: HostPlatformCopy = {
  credentialStore: "the system keyring",
  credentialStoreShort: "system keyring",
  credentialSavedLabel: "Saved in system keyring",
  credentialRemovalDetail:
    "The credential will be removed from the system keyring on this computer.",
  credentialValuesHidden: "System keyring values remain hidden.",
  revealLabel: (itemLabel) => `Show ${itemLabel} in file manager`,
  revealFailure: (itemLabel, detail) =>
    `The file manager could not show ${itemLabel}: ${detail}`,
  primaryModifier: "Ctrl",
  transcriptionHelperReady: "The on-device transcription helper is ready.",
  notificationDescription: "Show a desktop notification when a background render finishes.",
};

export function hostPlatformCopy(platform: HostPlatform): HostPlatformCopy {
  return platform === "macos" ? macosCopy : desktopCopy;
}

export function useHostPlatformCopy(): HostPlatformCopy {
  return hostPlatformCopy(useHostPlatform());
}
