import { backendRequest } from "@/lib/runtime/backend-client";

import type { SettingsCategoryHealth } from "./health";
import type { SettingsOperation } from "./operations";

export type StorageCleanupTarget =
  | { kind: "disposableAppCache" }
  | { kind: "projectRenderArtifacts"; artifactIds: string[] };

interface StorageCleanupPreviewItem {
  path: string;
  bytes: number;
}

export interface StorageCleanupPreview {
  target: StorageCleanupTarget;
  items: StorageCleanupPreviewItem[];
  totalBytes: number;
  previewNonce: string;
  confirmationToken: string;
  projectGeneration: number | null;
}

export function getStorageHealth(
  activeProjectDir: string | null,
): Promise<SettingsCategoryHealth> {
  return backendRequest<SettingsCategoryHealth>("get_storage_health", { activeProjectDir });
}

export function refreshStorageInventory(
  activeProjectDir: string | null,
): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("refresh_storage_inventory", { activeProjectDir });
}

export function previewStorageCleanup(
  target: StorageCleanupTarget,
  activeProjectDir: string | null,
): Promise<StorageCleanupPreview> {
  return backendRequest<StorageCleanupPreview>("preview_storage_cleanup", {
    target,
    activeProjectDir,
  });
}

export function runStorageCleanup(
  target: StorageCleanupTarget,
  confirmationToken: string,
  activeProjectDir: string | null,
): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("run_storage_cleanup", {
    target,
    confirmationToken,
    activeProjectDir,
  });
}

export function revealStorageInventoryItem(
  path: string,
  activeProjectDir: string | null,
): Promise<void> {
  return backendRequest<void>("reveal_storage_inventory_item", { path, activeProjectDir });
}
