import { open } from "@tauri-apps/plugin-dialog";

import { backendRequest } from "../backend-client";
import { BackendUnavailableError } from "../backend-transport";
import { getRuntimeMode } from "../runtime-mode";

/**
 * The fixture backend's stand-in for the native media chooser. In the DEV fixture runtime the chooser
 * request goes to the fixture transport, which answers with the fixture file paths (or null).
 */
export const fixtureMediaChooserOperation = "open_media_file_dialog";

/** The fixture backend's stand-in for the native export folder chooser; it answers a folder path or null. */
export const fixtureExportDirectoryChooserOperation = "open_export_directory_dialog";

interface FileDialogFilter {
  readonly name: string;
  readonly extensions: readonly string[];
}

export interface MediaFileDialogRequest {
  readonly title: string;
  readonly filters: readonly FileDialogFilter[];
}

/**
 * Opens the native multiple-file chooser. Resolves `null` when the user cancels, and rejects
 * with `BackendUnavailableError` when no desktop host is present (the browser runtime, and fixture
 * runtimes whose fixture backend has no chooser).
 */
export async function openMediaFiles(request: MediaFileDialogRequest): Promise<string[] | null> {
  if (import.meta.env.DEV && getRuntimeMode() === "fixture") {
    return backendRequest<string[] | null>(fixtureMediaChooserOperation, {
      title: request.title,
      filters: request.filters.map((filter) => ({ name: filter.name, extensions: [...filter.extensions] })),
    });
  }
  if (typeof window === "undefined" || window.__TAURI_INTERNALS__ === undefined) {
    throw new BackendUnavailableError();
  }
  const selected = await open({
    multiple: true,
    directory: false,
    title: request.title,
    filters: request.filters.map((filter) => ({
      name: filter.name,
      extensions: [...filter.extensions],
    })),
  });
  if (selected === null) return null;
  return (Array.isArray(selected) ? selected : [selected]).filter(
    (path): path is string => typeof path === "string" && path.trim().length > 0,
  );
}

/**
 * Opens the native folder chooser for an export destination, starting at `defaultPath` when given.
 * Resolves `null` when the user cancels, and rejects with `BackendUnavailableError` when no desktop
 * host is present (the browser runtime, and fixture runtimes whose fixture backend has no chooser).
 */
export async function chooseExportDirectory(defaultPath?: string): Promise<string | null> {
  if (import.meta.env.DEV && getRuntimeMode() === "fixture") {
    return backendRequest<string | null>(fixtureExportDirectoryChooserOperation, { defaultPath: defaultPath ?? null });
  }
  if (typeof window === "undefined" || window.__TAURI_INTERNALS__ === undefined) {
    throw new BackendUnavailableError();
  }
  const selected = await open({
    multiple: false,
    directory: true,
    title: "Choose export folder",
    ...(defaultPath?.trim() ? { defaultPath } : {}),
  });
  const path = Array.isArray(selected) ? selected[0] : selected;
  return typeof path === "string" && path.trim().length > 0 ? path : null;
}
