import type { RemoteOperationError } from "../backend-transport";

const outcomes = new Map<string, RemoteOperationError>();
const hostOutcomes = new Map<string, RemoteOperationError>();
const confirmedCreationRecovery = new Set<string>();
const listeners = new Set<() => void>();
let reconciler: ((projectId: string, expectedProjectId?: string) => Promise<unknown>) | null = null;

export function remoteUnknownOutcome(projectId: string): RemoteOperationError | null {
  return outcomes.get(projectId) ?? null;
}
export function remoteHostUnknownOutcome(hostLabel: string | undefined): RemoteOperationError | null {
  return hostLabel ? hostOutcomes.get(hostLabel) ?? null : null;
}
export function setRemoteHostUnknownOutcome(hostLabel: string | undefined, error: RemoteOperationError | null): void {
  if (!hostLabel) return;
  if (error) hostOutcomes.set(hostLabel, error);
  else hostOutcomes.delete(hostLabel);
  if (error?.operation === "remote_create_project") confirmedCreationRecovery.delete(hostLabel);
  for (const listener of listeners) listener();
}
export function confirmRemoteHostCreationRecovery(hostLabel: string | undefined): void {
  if (hostLabel) confirmedCreationRecovery.add(hostLabel);
}
export function remoteHostCreationRecoveryConfirmed(hostLabel: string): boolean {
  return confirmedCreationRecovery.has(hostLabel);
}
export function setRemoteUnknownOutcome(projectId: string, error: RemoteOperationError | null): void {
  if (error) outcomes.set(projectId, error);
  else outcomes.delete(projectId);
  for (const listener of listeners) listener();
}
export function subscribeRemoteUnknownOutcome(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
export function setRemoteOutcomeReconciler(handler: (projectId: string, expectedProjectId?: string) => Promise<unknown>): void {
  outcomes.clear();
  hostOutcomes.clear();
  confirmedCreationRecovery.clear();
  reconciler = handler;
  for (const listener of listeners) listener();
}
export async function reconcileRemoteProject<Result>(projectId: string, expectedProjectId?: string): Promise<Result> {
  if (!reconciler) throw new Error("The host is not connected.");
  return await reconciler(projectId, expectedProjectId) as Result;
}
