import { remoteCsrfToken } from "./remote-credentials";

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
export interface RemoteMediaReadiness {
  readonly status: "idle" | "loading" | "ready" | "failed";
  readonly version: number;
  readonly message?: string;
}
interface Resource {
  url?: string;
  expiresAt: number;
  pending?: Promise<void>;
  error?: string;
  refreshedAfterLoadError?: boolean;
}
interface ProjectResources {
  readonly resources: Map<string, Resource>;
  csrfToken: string;
  fetcher: Fetcher;
  timer?: ReturnType<typeof setTimeout>;
  canonicalPaths?: Set<string>;
  budgetError?: string;
}

const maxProjects = 8;
const maxResourcesPerProject = 8192;
const ticketBatchSize = 1024;
const expiryMarginMs = 15_000;
const legacyTicketLifetimeMs = 4 * 60_000;
const projects = new Map<string, ProjectResources>();
const readiness = new Map<string, RemoteMediaReadiness>();
const activeProjects = new Map<string, number>();
const listeners = new Set<() => void>();
const idleReadiness: RemoteMediaReadiness = { status: "idle", version: 0 };
let sequence = 0;

export function remoteMediaReadiness(projectId: string): RemoteMediaReadiness {
  return readiness.get(projectId) ?? idleReadiness;
}

export function subscribeRemoteMediaReadiness(listener: () => void, projectId?: string): () => void {
  listeners.add(listener);
  if (projectId) {
    activeProjects.set(projectId, (activeProjects.get(projectId) ?? 0) + 1);
    const project = projects.get(projectId);
    if (project && [...project.resources.values()].some((resource) => resource.url && resource.expiresAt <= Date.now())) void retryRemoteMediaTickets(projectId);
  }
  return () => {
    listeners.delete(listener);
    if (projectId) {
      const remaining = (activeProjects.get(projectId) ?? 1) - 1;
      if (remaining > 0) activeProjects.set(projectId, remaining);
      else activeProjects.delete(projectId);
    }
  };
}

export async function retryRemoteMediaTickets(projectId: string): Promise<void> {
  const project = projects.get(projectId);
  if (!project) return;
  const paths = [...project.resources.entries()].filter(([, resource]) => resource.error || resource.expiresAt <= Date.now()).map(([relativePath]) => ({ relativePath }));
  await cacheMediaTicketsForResult(projectId, paths, remoteCsrfToken(project.csrfToken), project.fetcher);
}

/** Reset on transport/session replacement; old credentials and pending responses cannot seed it. */
export function resetRemoteResourceSession(): void {
  for (const project of projects.values()) if (project.timer) clearTimeout(project.timer);
  projects.clear();
  readiness.clear();
  for (const listener of listeners) listener();
}

function publish(projectId: string, project: ProjectResources): void {
  if (projects.get(projectId) !== project) return;
  const resources = [...project.resources.values()];
  const error = project.budgetError ?? resources.find((resource) => resource.error)?.error;
  const status = resources.some((resource) => resource.pending) ? "loading" : error ? "failed" : "ready";
  readiness.set(projectId, { status, version: ++sequence, ...(error ? { message: error } : {}) });
  for (const listener of listeners) listener();
  if (project.timer) clearTimeout(project.timer);
  const earliest = Math.min(...resources.filter((resource) => resource.url && !resource.error && resource.expiresAt > Date.now()).map((resource) => resource.expiresAt));
  if (Number.isFinite(earliest)) {
    project.timer = setTimeout(() => {
      if (projects.get(projectId) !== project) return;
      if ((activeProjects.get(projectId) ?? 0) > 0) void retryRemoteMediaTickets(projectId);
      else {
        for (const resource of project.resources.values()) if (resource.expiresAt <= Date.now()) resource.error = "Project media has expired. Retry to reconnect.";
        publish(projectId, project);
      }
    }, Math.max(1, earliest - Date.now()));
  }
}

function trim(): void {
  while (projects.size > maxProjects) {
    const projectId = [...projects.keys()].find((id) => !activeProjects.has(id)) ?? projects.keys().next().value;
    if (projectId === undefined) break;
    const project = projects.get(projectId);
    if (project?.timer) clearTimeout(project.timer);
    projects.delete(projectId);
    readiness.delete(projectId);
  }
}

export async function cacheMediaTicketsForResult(projectId: string, result: unknown, csrfToken: string, fetcher: Fetcher): Promise<void> {
  const relativePaths = [...collectRelativePaths(result)].sort();
  const canonicalPaths = canonicalResourcePaths(result);
  if (!projectId || (relativePaths.length === 0 && !canonicalPaths)) return;
  let project = projects.get(projectId);
  if (!project) {
    project = { resources: new Map(), csrfToken, fetcher };
    projects.set(projectId, project);
    trim();
  }
  project.csrfToken = csrfToken;
  project.fetcher = fetcher;
  if (canonicalPaths) {
    for (const path of project.canonicalPaths ?? []) if (!canonicalPaths.has(path)) project.resources.delete(path);
    project.canonicalPaths = new Set([...canonicalPaths].slice(-maxResourcesPerProject));
  }
  delete project.budgetError;
  const missing: string[] = [];
  const pending = new Set<Promise<void>>();
  for (const relativePath of relativePaths) {
    let resource = project.resources.get(relativePath);
    if (!resource) {
      if (project.resources.size >= maxResourcesPerProject) {
        const oldest = [...project.resources.entries()].find(([, entry]) => !entry.pending)?.[0];
        if (oldest) project.resources.delete(oldest);
        else {
          project.budgetError = "Too many media previews are loading. Retry after they settle.";
          continue;
        }
      }
      resource = { expiresAt: 0 };
      project.resources.set(relativePath, resource);
    }
    if (resource.pending) pending.add(resource.pending);
    else if (!resource.url || resource.expiresAt <= Date.now()) missing.push(relativePath);
  }
  const current = project;
  const availableMissing = missing.filter((path) => current.resources.has(path));
  if (availableMissing.length < missing.length) current.budgetError = "This project exceeds the media preview cache limit.";
  for (let offset = 0; offset < availableMissing.length; offset += ticketBatchSize) {
    const batch = availableMissing.slice(offset, offset + ticketBatchSize);
    const request = Promise.resolve().then(async () => {
      try {
        const response = await fetcher("/api/v1/resource-tickets/media", {
          method: "POST", credentials: "same-origin",
          headers: { accept: "application/json", "content-type": "application/json", "x-csrf-token": remoteCsrfToken(csrfToken) },
          body: JSON.stringify({ projectId, relativePaths: batch }),
        });
        const body = await response.json() as { readonly urls?: Record<string, string>; readonly expiresAt?: Record<string, number>; readonly message?: string };
        if (!response.ok || !body.urls) throw new Error(body.message ?? "Project media could not be loaded. Retry to reconnect.");
        if (projects.get(projectId) !== current) return;
        for (const relativePath of batch) {
          const path = `${projectId}/${relativePath}`;
          const resource = current.resources.get(relativePath);
          if (!resource) continue;
          const url = body.urls[path];
          if (url && /^\/api\/v1\/media\/[a-f0-9]{32}$/.test(url)) {
            resource.url = url;
            const expiry = body.expiresAt?.[path];
            resource.expiresAt = typeof expiry === "number" && Number.isFinite(expiry) ? expiry * 1000 - expiryMarginMs : Date.now() + legacyTicketLifetimeMs;
            if (resource.expiresAt <= Date.now()) resource.error = "Project media has expired. Retry to reconnect.";
            else delete resource.error;
          } else resource.error = "Some project media could not be loaded. Retry to reconnect.";
        }
      } catch (error) {
        if (projects.get(projectId) !== current) return;
        for (const relativePath of batch) {
          const resource = current.resources.get(relativePath);
          if (resource) resource.error = error instanceof Error ? error.message : "Project media could not be loaded. Retry to reconnect.";
        }
      } finally {
        for (const relativePath of batch) {
          const resource = current.resources.get(relativePath);
          if (resource?.pending === request) delete resource.pending;
        }
        for (const [path, resource] of current.resources) {
          if (current.resources.size <= maxResourcesPerProject) break;
          if (!resource.pending) current.resources.delete(path);
        }
        publish(projectId, current);
      }
    });
    for (const relativePath of batch) {
      const resource = current.resources.get(relativePath)!;
      resource.pending = request;
      delete resource.error;
    }
    pending.add(request);
  }
  if (pending.size > 0 || current.budgetError || canonicalPaths) publish(projectId, current);
  await Promise.all(pending);
}

/** No filesystem path fallback or render-time exception when a ticket is pending or expired. */
export function remoteResourceUrl(path: string): string {
  for (const [projectId, project] of projects) {
    if (!path.startsWith(`${projectId}/`)) continue;
    const resource = project.resources.get(path.slice(projectId.length + 1));
    if (resource?.url && resource.expiresAt > Date.now()) return resource.url;
  }
  return "";
}

/** At most one automatic ticket renewal per resource load failure; decoding failures stay retryable. */
export function refreshRemoteMediaUrl(url: string, manual = false): boolean {
  for (const [projectId, project] of projects) {
    const resource = [...project.resources.values()].find((entry) => entry.url === url);
    if (!resource || (!manual && resource.refreshedAfterLoadError)) continue;
    resource.refreshedAfterLoadError = !manual;
    resource.expiresAt = 0;
    void retryRemoteMediaTickets(projectId);
    return true;
  }
  return false;
}

export function clearRemoteResourceUrlsForTests(): void { resetRemoteResourceSession(); }

function collectRelativePaths(value: unknown, paths = new Set<string>()): Set<string> {
  if (Array.isArray(value)) {
    for (const item of value) collectRelativePaths(item, paths);
    return paths;
  }
  if (typeof value !== "object" || value === null) return paths;
  for (const [key, nested] of Object.entries(value)) {
    if (key === "framePaths" && Array.isArray(nested)) {
      for (const path of nested) if (typeof path === "string" && path.length > 0) paths.add(path);
    } else if (["relativePath", "previewFrame"].includes(key) && typeof nested === "string" && nested.length > 0) paths.add(nested);
    else collectRelativePaths(nested, paths);
  }
  return paths;
}

function canonicalResourcePaths(value: unknown): Set<string> | null {
  if (typeof value !== "object" || value === null) return null;
  const record = value as Record<string, unknown>;
  if (Array.isArray(record.media)) return collectRelativePaths(record);
  return typeof record.project === "object" && record.project !== null && Array.isArray((record.project as Record<string, unknown>).media)
    ? collectRelativePaths(record.project) : null;
}
