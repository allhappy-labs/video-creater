import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { clearRemoteResourceUrlsForTests } from "./remote-resource-cache";
import { clearRemoteProjectAccessForTests, remoteEditorLeaseToken } from "./remote-project-access";
import { RemoteTransport } from "./remote-transport";
import { resetRemoteCsrfTokenForTests, setRemoteCsrfToken } from "./remote-credentials";

describe("RemoteTransport", () => {
  beforeEach(() => { window.sessionStorage.clear(); resetRemoteCsrfTokenForTests(); });

  it("uses a refreshed CSRF token shared by browser-only operations", async () => {
    const fetcher = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
      const request = JSON.parse(String(init?.body)) as { requestId: string };
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: null }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });
    setRemoteCsrfToken("csrf-2");

    await transport.request("get_platform_info");

    expect(fetcher).toHaveBeenCalledWith("/api/v1/rpc", expect.objectContaining({
      headers: expect.objectContaining({ "x-csrf-token": "csrf-2" }),
    }));
  });
  beforeEach(() => {
    clearRemoteResourceUrlsForTests();
    clearRemoteProjectAccessForTests();
  });
  afterEach(() => vi.unstubAllGlobals());

  it("sends same-origin credentialed RPC with request identity and CSRF", async () => {
    const fetcher = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { platform: "linux" } }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });

    await expect(transport.request("get_platform_info")).resolves.toEqual({ platform: "linux" });
    expect(fetcher).toHaveBeenCalledWith("/api/v1/rpc", expect.objectContaining({
      method: "POST",
      credentials: "same-origin",
      headers: expect.objectContaining({ "x-csrf-token": "csrf-1" }),
    }));
    const body = JSON.parse(String(fetcher.mock.calls[0]?.[1]?.body));
    expect(body.operation).toBe("get_platform_info");
    expect(body.requestId).toMatch(/^browser-/);
  });

  it("surfaces a stable remote error without leaking response internals", async () => {
    const transport = new RemoteTransport({
      csrfToken: "csrf-1",
      fetcher: async (_url, init) => new Response(JSON.stringify({
        requestId: (JSON.parse(String(init?.body)) as { requestId: string }).requestId,
        ok: false,
        error: { code: "forbidden", message: "operation is not permitted" },
      })),
    });
    await expect(transport.request("get_app_preferences")).rejects.toThrow("operation is not permitted");
  });

  it("binds the browser's native fetch receiver", async () => {
    let receiver: unknown;
    vi.stubGlobal("fetch", function nativeLikeFetch(this: unknown, _input: RequestInfo | URL, init?: RequestInit) {
      receiver = this;
      const request = JSON.parse(String(init?.body)) as { requestId: string };
      return Promise.resolve(new Response(JSON.stringify({
        requestId: request.requestId,
        ok: true,
        result: { platform: "linux" },
      })));
    });

    const transport = new RemoteTransport({ csrfToken: "csrf-1" });
    await transport.request("get_platform_info");

    expect(receiver).toBe(globalThis);
  });

  it("prepares opaque media tickets independently of returning a loaded project", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input) === "/api/v1/resource-tickets/media") {
        return new Response(JSON.stringify({
          urls: { "opaque-project/media/clip.mp4": "/api/v1/media/0123456789abcdef0123456789abcdef" },
        }));
      }
      if (String(input) === "/api/v1/projects/opaque-project/lease") {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      const request = JSON.parse(String(init?.body)) as { requestId: string };
      return new Response(JSON.stringify({
        requestId: request.requestId,
        ok: true,
        result: { media: [{ relativePath: "media/clip.mp4" }] },
      }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });

    await transport.request("load_split_project_from_folder", { projectDir: "opaque-project" });

    await vi.waitFor(() => expect(transport.mediaUrl("opaque-project/media/clip.mp4")).not.toBe(""));

    expect(transport.mediaUrl("opaque-project/media/clip.mp4")).toBe("/api/v1/media/0123456789abcdef0123456789abcdef");
    expect(fetcher).toHaveBeenCalledTimes(3);
    expect(remoteEditorLeaseToken("opaque-project")).toBe("lease-1");
  });

  it("does not let a stalled ticket request block committed edits or the next edit", async () => {
    const requests: Array<Record<string, unknown>> = [];
    const transport = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
      if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: 200 }));
      if (String(url).endsWith("/resource-tickets/media")) return new Promise<Response>(() => undefined);
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      requests.push(request);
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { project: { contentRevision: requests.length, media: [{ relativePath: "media/a.mp4" }] } } }));
    } });
    await transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] });
    await transport.request("apply_project_actions_to_split_project_folder", { projectDir: "p", actions: [] });
    expect(requests).toHaveLength(2);
    expect(requests[1]?.expectedRevision).toBe(1);
    expect(transport.mediaUrl("p/media/a.mp4")).toBe("");
  });

  it("adds the private editor lease to project mutations", async () => {
    let rpcCalls = 0;
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/lease")) {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      rpcCalls += 1;
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: rpcCalls === 1 ? { media: [] } : { saved: true } }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });
    await transport.request("load_split_project_from_folder", { projectDir: "opaque-project" });

    await transport.request("save_split_project_to_folder", { projectDir: "opaque-project", expectedRevision: 1, project: {} });

    const mutation = JSON.parse(String(fetcher.mock.calls.at(-1)?.[1]?.body));
    expect(mutation.editorLeaseToken).toBe("lease-1");
  });

  it("sends cancellation immediately without waiting to renew the project lease", async () => {
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/lease")) {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      if (String(input) === "/api/v1/resource-tickets/media") {
        return new Response(JSON.stringify({ urls: {} }));
      }
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      return new Response(JSON.stringify({
        requestId: request.requestId,
        ok: true,
        result: request.operation === "load_split_project_from_folder" ? { media: [] } : false,
      }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });
    await transport.request("load_split_project_from_folder", { projectDir: "opaque-project" });
    fetcher.mockClear();

    await transport.request("cancel_codex_conversation_edit_for_project", {
      projectDir: "opaque-project",
    });

    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith("/api/v1/rpc", expect.anything());
  });

  it("sends cancellation while a same-project foreground request is still running", async () => {
    let releaseForeground!: () => void;
    const foregroundGate = new Promise<void>((resolve) => { releaseForeground = resolve; });
    const startedOperations: string[] = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/lease")) {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
      startedOperations.push(request.operation);
      if (request.operation === "render_media_to_split_project_folder") await foregroundGate;
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: false }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });

    const foreground = transport.request("render_media_to_split_project_folder", {
      projectDir: "opaque-project",
      expectedRevision: 4,
    });
    await vi.waitFor(() => expect(startedOperations).toEqual(["render_media_to_split_project_folder"]));

    const cancellation = transport.request("cancel_render_job_in_split_project_folder", {
      projectDir: "opaque-project",
      jobId: "job-1",
      expectedRevision: 4,
    });
    await expect(cancellation).resolves.toBe(false);
    expect(startedOperations).toEqual([
      "render_media_to_split_project_folder",
      "cancel_render_job_in_split_project_folder",
    ]);

    releaseForeground();
    await foreground;
  });

  it("uses the host revision learned from a derived-artifact response when the caller omits one", async () => {
    const rpcBodies: Array<Record<string, unknown>> = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/lease")) {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      if (String(input) === "/api/v1/resource-tickets/media") {
        return new Response(JSON.stringify({ urls: {} }));
      }
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      rpcBodies.push(request);
      const result = rpcBodies.length === 1
        ? { contentRevision: 4, media: [] }
        : rpcBodies.length === 2
          ? { project: { contentRevision: 6 }, previewFrame: "renders/frame.png" }
          : { project: { contentRevision: 7 } };
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });
    await transport.request("load_split_project_from_folder", { projectDir: "opaque-project" });
    await transport.request("capture_canonical_preview_frame_in_split_project_folder", {
      projectDir: "opaque-project",
      expectedRevision: 4,
    });

    await transport.request("render_media_to_split_project_folder", {
      projectDir: "opaque-project",
    });

    expect(rpcBodies.at(-1)?.expectedRevision).toBe(6);
  });

  it("serializes same-project operations without rewriting the caller's revision precondition", async () => {
    let releasePreview!: () => void;
    const previewGate = new Promise<void>((resolve) => { releasePreview = resolve; });
    const rpcBodies: Array<Record<string, unknown>> = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/lease")) {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      if (String(input) === "/api/v1/resource-tickets/media") {
        return new Response(JSON.stringify({ urls: {} }));
      }
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      rpcBodies.push(request);
      if (request.operation === "capture_canonical_preview_frame_in_split_project_folder") {
        await previewGate;
        return new Response(JSON.stringify({
          requestId: request.requestId,
          ok: true,
          result: { project: { contentRevision: 6 }, previewFrame: "renders/frame.png" },
        }));
      }
      return new Response(JSON.stringify({
        requestId: request.requestId,
        ok: true,
        result: { project: { contentRevision: 7 } },
      }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });

    const preview = transport.request("capture_canonical_preview_frame_in_split_project_folder", {
      projectDir: "opaque-project",
      expectedRevision: 5,
    });
    const render = transport.request("render_media_to_split_project_folder", {
      projectDir: "opaque-project",
      expectedRevision: 5,
    });
    await vi.waitFor(() => expect(rpcBodies).toHaveLength(1));
    releasePreview();
    await Promise.all([preview, render]);

    expect(rpcBodies).toHaveLength(2);
    expect(rpcBodies[1]?.expectedRevision).toBe(5);
  });

  it("preserves a stale caller revision so the host can reject the mutation", async () => {
    const rpcBodies: Array<Record<string, unknown>> = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith("/lease")) {
        return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease-1", expiresAt: 200 }));
      }
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      rpcBodies.push(request);
      return new Response(JSON.stringify({
        requestId: request.requestId,
        ok: true,
        result: { project: { contentRevision: 9 } },
      }));
    });
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });

    await transport.request("render_media_to_split_project_folder", {
      projectDir: "opaque-project",
      expectedRevision: 4,
    });
    await transport.request("save_split_project_to_folder", {
      projectDir: "opaque-project",
      expectedRevision: 3,
      project: {},
    });

    expect(rpcBodies.at(-1)?.expectedRevision).toBe(3);
  });

  it("mints an opaque attachment URL for a recorded artifact", async () => {
    const fetcher = vi.fn(async () => new Response(JSON.stringify({
      url: "/api/v1/artifacts/fedcba9876543210fedcba9876543210",
    })));
    const transport = new RemoteTransport({ csrfToken: "csrf-1", fetcher });

    await expect(transport.artifactUrl("opaque-project", "export-a"))
      .resolves.toBe("/api/v1/artifacts/fedcba9876543210fedcba9876543210");
    expect(fetcher).toHaveBeenCalledWith("/api/v1/resource-tickets/artifact", expect.objectContaining({
      body: JSON.stringify({ projectId: "opaque-project", artifactId: "export-a" }),
    }));
  });

  it("requests a fresh application snapshot when the event resume window is gone", async () => {
    let socket!: WebSocket;
    const snapshotRequiredHandler = vi.fn();
    const transport = new RemoteTransport({
      csrfToken: "csrf-1",
      snapshotRequiredHandler,
      webSocketFactory: () => {
        socket = { close: vi.fn() } as unknown as WebSocket;
        return socket;
      },
    });

    await transport.listen("project-changed", vi.fn());
    socket.onmessage?.({ data: JSON.stringify({ type: "snapshotRequired" }) } as MessageEvent);

    expect(snapshotRequiredHandler).toHaveBeenCalledOnce();
  });
});
