export type RemoteProjectAccess =
  | { readonly mode: "unknown" }
  | { readonly mode: "unavailable"; readonly message: string }
  | { readonly mode: "editing"; readonly expiresAt: number }
  | { readonly mode: "readOnly"; readonly editorDisplayName: string };

type StoredAccess = RemoteProjectAccess | {
  readonly mode: "editing";
  readonly expiresAt: number;
  readonly token: string;
};

const unknownAccess: RemoteProjectAccess = { mode: "unknown" };
const accessByProject = new Map<string, StoredAccess>();
const publicAccessByProject = new Map<string, RemoteProjectAccess>();
const revisionByProject = new Map<string, number>();
const listeners = new Set<() => void>();
let takeoverHandler: ((projectId: string) => Promise<void>) | null = null;
let refreshHandler: ((projectId: string) => Promise<void>) | null = null;

export function setRemoteProjectAccess(projectId: string, access: StoredAccess): void {
  accessByProject.set(projectId, access);
  publicAccessByProject.set(projectId, access.mode === "editing"
    ? { mode: "editing", expiresAt: access.expiresAt }
    : access);
  for (const listener of listeners) listener();
}

export function remoteProjectAccess(projectId: string): RemoteProjectAccess {
  return publicAccessByProject.get(projectId) ?? unknownAccess;
}

export function remoteEditorLeaseToken(projectId: string): string | undefined {
  const access = accessByProject.get(projectId);
  return access?.mode === "editing" && "token" in access ? access.token : undefined;
}

export function setRemoteProjectRevision(projectId: string, revision: number): void {
  if (Number.isSafeInteger(revision) && revision >= (revisionByProject.get(projectId) ?? 0)) revisionByProject.set(projectId, revision);
}

export function remoteProjectRevision(projectId: string): number | undefined {
  return revisionByProject.get(projectId);
}

export function subscribeRemoteProjectAccess(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function setRemoteTakeoverHandler(handler: ((projectId: string) => Promise<void>) | null): void {
  takeoverHandler = handler;
}

export function setRemoteLeaseRefreshHandler(handler: (projectId: string) => Promise<void>): void {
  refreshHandler = handler;
}

export async function refreshRemoteProjectAccess(projectId: string): Promise<void> {
  if (!refreshHandler) throw new Error("The remote editor is not connected.");
  await refreshHandler(projectId);
}

export async function takeOverRemoteProject(projectId: string): Promise<void> {
  if (!takeoverHandler) throw new Error("The remote editor is not connected.");
  await takeoverHandler(projectId);
}

export function resetRemoteProjectAccess(): void {
  accessByProject.clear();
  publicAccessByProject.clear();
  revisionByProject.clear();
  takeoverHandler = null;
  refreshHandler = null;
  for (const listener of listeners) listener();
}

export function clearRemoteProjectAccessForTests(): void { resetRemoteProjectAccess(); }
