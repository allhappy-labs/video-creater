import {
  backendListen,
  backendRequest,
} from "@/lib/runtime/backend-client";
import type { BackendUnlisten } from "@/lib/runtime/backend-transport";

export const nativeMenuEventName = "video-creater://native-menu-command";

const nativeMenuCommands = [
  "openSettings",
  "openProjectSettings",
  "openAdvancedSettings",
  "openSystemHealth",
  "newProject",
  "openProject",
  "importMedia",
  "exportProject",
  "undo",
  "redo",
  "selectForwardTrack",
  "selectForwardAll",
  "split",
  "trimStart",
  "trimEnd",
  "delete",
  "rippleDelete",
  "showTab:ai",
  "showTab:media",
  "showTab:audio",
  "showTab:text",
  "showTab:captions",
  "showTab:effects",
  "openShortcuts",
  "openConnectAgents",
  "openProjectGuidance",
  "sendFeedback",
] as const;

export type NativeMenuCommand = (typeof nativeMenuCommands)[number];

export interface NativeMenuRequest {
  sequence: number;
  command: NativeMenuCommand;
}

/** What the native menu can do right now; mirrors `NativeMenuState` in `src-tauri/src/native_menu.rs`. */
export interface NativeMenuState {
  view: "home" | "editor" | "settings";
  canImport: boolean;
  canExport: boolean;
  canUndo: boolean;
  canRedo: boolean;
  canSplit: boolean;
  canTrimStart: boolean;
  canTrimEnd: boolean;
  canDelete: boolean;
  canRippleDelete: boolean;
  canSelectForward: boolean;
}

export function inactiveNativeMenuState(
  view: "home" | "settings",
): NativeMenuState {
  return {
    view,
    canImport: false,
    canExport: false,
    canUndo: false,
    canRedo: false,
    canSplit: false,
    canTrimStart: false,
    canTrimEnd: false,
    canDelete: false,
    canRippleDelete: false,
    canSelectForward: false,
  };
}

export function syncNativeMenuState(state: NativeMenuState): Promise<void> {
  return backendRequest("sync_native_menu_state", { state });
}

const nativeMenuCommandSet = new Set<string>(nativeMenuCommands);

export function parseNativeMenuRequest(value: unknown): NativeMenuRequest | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  const record = value as Record<string, unknown>;
  if (
    !Number.isSafeInteger(record.sequence) ||
    (record.sequence as number) < 1 ||
    typeof record.command !== "string" ||
    !nativeMenuCommandSet.has(record.command)
  ) {
    return null;
  }
  return {
    sequence: record.sequence as number,
    command: record.command as NativeMenuCommand,
  };
}

export function listenForNativeMenuCommands(
  onRequest: (request: NativeMenuRequest) => void,
): Promise<BackendUnlisten> {
  return backendListen<unknown>(nativeMenuEventName, (payload) => {
    const request = parseNativeMenuRequest(payload);
    if (request) {
      onRequest(request);
    }
  });
}
