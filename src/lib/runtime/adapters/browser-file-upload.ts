import type { ImportMediaResult } from "@/lib/project";
import { discoverRemoteSession } from "../remote-session";
import { cacheMediaTicketsForResult } from "./remote-resource-cache";
import { setRemoteCsrfToken } from "./remote-credentials";
import { setRemoteProjectAccess, setRemoteProjectRevision } from "./remote-project-access";

type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

interface BrowserUploadOptions {
  readonly projectId: string;
  readonly expectedRevision: number;
  readonly fetcher?: Fetcher;
  readonly createRequest?: () => XMLHttpRequest;
  readonly onProgress?: (fraction: number) => void;
  readonly signal?: AbortSignal;
}

interface StagedUpload {
  readonly uploadId: string;
}

export async function uploadBrowserFile(
  file: File,
  options: BrowserUploadOptions,
): Promise<ImportMediaResult> {
  const fetcher = options.fetcher ?? globalThis.fetch.bind(globalThis);
  const session = await discoverRemoteSession(fetcher);
  if (session.kind !== "connected") {
    throw new Error("This browser is no longer connected to the host.");
  }
  setRemoteCsrfToken(session.csrfToken);
  const staged = await stageFile(file, session.csrfToken, options);
  const response = await fetcher(`/api/v1/uploads/${staged.uploadId}/import`, {
    method: "POST",
    credentials: "same-origin",
    headers: {
      accept: "application/json",
      "content-type": "application/json",
      "x-csrf-token": session.csrfToken,
    },
    body: JSON.stringify({
      projectId: options.projectId,
      expectedRevision: options.expectedRevision,
    }),
    ...(options.signal ? { signal: options.signal } : {}),
  });
  const body = await response.json() as ImportMediaResult & { message?: string; editorLeaseToken?: string };
  if (!response.ok) throw new Error(body.message ?? "Uploaded media could not be imported.");
  if (body.editorLeaseToken) {
    setRemoteProjectAccess(options.projectId, {
      mode: "editing",
      token: body.editorLeaseToken,
      expiresAt: Math.floor(Date.now() / 1_000) + 30,
    });
  }
  if (typeof body.project.contentRevision === "number") {
    setRemoteProjectRevision(options.projectId, body.project.contentRevision);
  }
  await cacheMediaTicketsForResult(
    options.projectId,
    body,
    session.csrfToken,
    fetcher,
  );
  return body;
}

function stageFile(
  file: File,
  csrfToken: string,
  options: BrowserUploadOptions,
): Promise<StagedUpload> {
  return new Promise((resolve, reject) => {
    const request = options.createRequest?.() ?? new XMLHttpRequest();
    const abort = () => request.abort();
    const cleanup = () => options.signal?.removeEventListener("abort", abort);
    if (options.signal?.aborted) {
      reject(new DOMException("Upload cancelled", "AbortError"));
      return;
    }
    options.signal?.addEventListener("abort", abort, { once: true });
    request.open("POST", `/api/v1/uploads?${new URLSearchParams({ name: file.name })}`);
    request.withCredentials = true;
    request.setRequestHeader("x-csrf-token", csrfToken);
    request.setRequestHeader("accept", "application/json");
    request.upload.onprogress = (event) => {
      if (event.lengthComputable && event.total > 0) {
        options.onProgress?.(Math.min(1, event.loaded / event.total));
      }
    };
    request.onload = () => {
      cleanup();
      try {
        const body = JSON.parse(request.responseText) as StagedUpload & { message?: string };
        if (request.status < 200 || request.status >= 300 || !body.uploadId) {
          reject(new Error(body.message ?? "Upload failed."));
          return;
        }
        options.onProgress?.(1);
        resolve(body);
      } catch {
        reject(new Error("Upload returned an invalid response."));
      }
    };
    request.onerror = () => {
      cleanup();
      reject(new Error("Upload connection failed."));
    };
    request.onabort = () => {
      cleanup();
      reject(new DOMException("Upload cancelled", "AbortError"));
    };
    request.send(file);
  });
}
