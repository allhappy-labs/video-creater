import { describe, expect, it, vi } from "vitest";

import { uploadBrowserFile } from "./browser-file-upload";

class FakeRequest {
  method = "";
  url = "";
  body: unknown;
  status = 200;
  responseText = JSON.stringify({ uploadId: "upload-1" });
  withCredentials = false;
  headers = new Map<string, string>();
  upload = { onprogress: null as ((event: ProgressEvent) => void) | null };
  onload: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onabort: (() => void) | null = null;
  autoComplete = true;
  aborted = false;
  open(method: string, url: string) { this.method = method; this.url = url; }
  setRequestHeader(name: string, value: string) { this.headers.set(name, value); }
  send(body: unknown) {
    this.body = body;
    this.upload.onprogress?.({ lengthComputable: true, loaded: 8, total: 16 } as ProgressEvent);
    if (this.autoComplete) this.onload?.();
  }
  abort() { this.aborted = true; this.onabort?.(); }
}

describe("uploadBrowserFile", () => {
  it("uploads bytes with progress then imports the opaque staging ID", async () => {
    const request = new FakeRequest();
    const progress: number[] = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input) === "/api/v1/session") {
        return new Response(JSON.stringify({
          authenticated: true,
          sessionId: "session-1",
          displayName: "Browser",
          hostLabel: "Host",
          csrfToken: "csrf-1",
          protocolVersion: 1,
        }));
      }
      return new Response(JSON.stringify({ project: { id: "project-1" }, imported: [], skipped: [] }));
    });
    const file = new File([new Uint8Array(16)], "clip one.mp4", { type: "video/mp4" });

    const result = await uploadBrowserFile(file, {
      projectId: "opaque-project",
      expectedRevision: 4,
      fetcher,
      createRequest: () => request as unknown as XMLHttpRequest,
      onProgress: (value) => progress.push(value),
    });

    expect(request.method).toBe("POST");
    expect(request.url).toBe("/api/v1/uploads?name=clip+one.mp4");
    expect(request.withCredentials).toBe(true);
    expect(request.headers.get("x-csrf-token")).toBe("csrf-1");
    expect(request.body).toBe(file);
    expect(progress).toEqual([0.5, 1]);
    expect(fetcher).toHaveBeenLastCalledWith(
      "/api/v1/uploads/upload-1/import",
      expect.objectContaining({
        method: "POST",
        credentials: "same-origin",
        body: JSON.stringify({ projectId: "opaque-project", expectedRevision: 4 }),
      }),
    );
    expect(result.project).toEqual({ id: "project-1" });
  });

  it("cancels an in-flight upload through the request signal", async () => {
    const request = new FakeRequest();
    request.autoComplete = false;
    const controller = new AbortController();
    const fetcher = vi.fn(async () => new Response(JSON.stringify({
      authenticated: true,
      sessionId: "session-1",
      displayName: "Browser",
      hostLabel: "Host",
      csrfToken: "csrf-1",
      protocolVersion: 1,
    })));
    const pending = uploadBrowserFile(new File([new Uint8Array(16)], "clip.mp4"), {
      projectId: "opaque-project",
      expectedRevision: 4,
      fetcher,
      createRequest: () => request as unknown as XMLHttpRequest,
      signal: controller.signal,
    });

    await vi.waitFor(() => expect(request.body).toBeInstanceOf(File));
    controller.abort();

    await expect(pending).rejects.toMatchObject({ name: "AbortError" });
    expect(request.aborted).toBe(true);
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("surfaces a safe staging failure and permits a fresh retry", async () => {
    const failed = new FakeRequest();
    failed.status = 413;
    failed.responseText = JSON.stringify({ message: "Upload is larger than the host limit." });
    const retried = new FakeRequest();
    const requests = [failed, retried];
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      if (String(input) === "/api/v1/session") {
        return new Response(JSON.stringify({
          authenticated: true,
          sessionId: "session-1",
          displayName: "Browser",
          hostLabel: "Host",
          csrfToken: "csrf-1",
          protocolVersion: 1,
        }));
      }
      return new Response(JSON.stringify({ project: { id: "project-1" }, imported: [], skipped: [] }));
    });
    const options = {
      projectId: "opaque-project",
      expectedRevision: 4,
      fetcher,
      createRequest: () => requests.shift() as unknown as XMLHttpRequest,
    };
    const file = new File([new Uint8Array(16)], "clip.mp4", { type: "video/mp4" });

    await expect(uploadBrowserFile(file, options)).rejects.toThrow("Upload is larger than the host limit.");
    await expect(uploadBrowserFile(file, options)).resolves.toMatchObject({ project: { id: "project-1" } });
  });
});
