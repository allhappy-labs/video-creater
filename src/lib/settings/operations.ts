import { backendRequest } from "@/lib/runtime/backend-client";

export const settingsOperationEvent = "settings-operation";

export type SettingsOperationKind =
  | "modelDownload"
  | "speechModelsDownload"
  | "healthCheck"
  | "skillRepair"
  | "storageRefresh"
  | "storageCleanup"
  | "providerRefresh";

export type SettingsOperationState =
  | "queued"
  | "running"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled";

interface SettingsOperationError {
  code: string;
  message: string;
  recoveryAction: string | null;
  detail: string | null;
}

export interface SettingsOperation {
  id: string;
  kind: SettingsOperationKind;
  targetId: string;
  phase: string;
  state: SettingsOperationState;
  completedUnits: number;
  totalUnits: number | null;
  unit: string | null;
  cancellable: boolean;
  message: string;
  error: SettingsOperationError | null;
  startedAt: string;
  updatedAt: string;
}

interface SettingsOperationsCommandError {
  code: string;
  message: string;
  detail: string;
}

export function listSettingsOperations(): Promise<SettingsOperation[]> {
  return backendRequest<SettingsOperation[]>("list_settings_operations");
}

export function cancelSettingsOperation(
  operationId: string,
): Promise<SettingsOperation> {
  return backendRequest<SettingsOperation>("cancel_settings_operation", { operationId });
}
