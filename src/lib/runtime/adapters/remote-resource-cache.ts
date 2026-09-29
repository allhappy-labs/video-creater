type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

const resourceUrls = new Map<string, string>();

export async function cacheMediaTicketsForResult(
  projectId: string,
  result: unknown,
  csrfToken: string,
  fetcher: Fetcher,
): Promise<void> {
  const relativePaths = [...collectRelativePaths(result)].sort();
  if (!projectId || relativePaths.length === 0) return;
  const response = await fetcher("/api/v1/resource-tickets/media", {
    method: "POST",
    credentials: "same-origin",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
      "x-csrf-token": csrfToken,
    },
    body: JSON.stringify({ projectId, relativePaths }),
  });
  const body = await response.json() as { readonly urls?: Record<string, string>; readonly message?: string };
  if (!response.ok || !body.urls) {
    throw new Error(body.message ?? "Project media could not be authorized for this browser.");
  }
  for (const [path, url] of Object.entries(body.urls)) {
    if (path && /^\/api\/v1\/media\/[a-f0-9]{32}$/.test(url)) resourceUrls.set(path, url);
  }
}

export function remoteResourceUrl(path: string): string {
  const url = resourceUrls.get(path);
  if (!url) throw new Error("Remote media is not authorized for this browser session.");
  return url;
}

export function clearRemoteResourceUrlsForTests(): void {
  resourceUrls.clear();
}

function collectRelativePaths(value: unknown, paths = new Set<string>()): Set<string> {
  if (Array.isArray(value)) {
    for (const item of value) collectRelativePaths(item, paths);
    return paths;
  }
  if (typeof value !== "object" || value === null) return paths;
  for (const [key, nested] of Object.entries(value)) {
    if (["relativePath", "previewFrame"].includes(key)
      && typeof nested === "string" && nested.length > 0) {
      paths.add(nested);
    } else {
      collectRelativePaths(nested, paths);
    }
  }
  return paths;
}
