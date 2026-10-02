import { beforeEach, describe, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { RemoteTransport } from "@/lib/runtime/adapters/remote-transport";
import { clearRemoteProjectAccessForTests } from "@/lib/runtime/adapters/remote-project-access";
import { clearRemoteResourceUrlsForTests, remoteMediaReadiness } from "@/lib/runtime/adapters/remote-resource-cache";
import { fixtureItem, fixtureProject } from "@/test-utils/editor-fixtures";

const backend = vi.hoisted(() => ({ current: null as RemoteTransport | null }));
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (operation: string, input: Record<string, unknown>) => backend.current!.request(operation, input),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => backend.current!.mediaUrl(path),
}));
const { createEditorStore } = await import("./editor-store");

describe("remote canonical project consistency", () => {
  beforeEach(() => {
    clearRemoteProjectAccessForTests();
    clearRemoteResourceUrlsForTests();
  });

  it("installs open, edits, undo and reconciled state even when media authorization fails", async () => {
    let canonical: VideoProject = { ...fixtureProject(), schemaVersion: 2, contentRevision: 3 };
    const requests: Array<Record<string, unknown>> = [];
    let ticketCalls = 0;
    const fetcher = async (url: RequestInfo | URL, init?: RequestInit) => {
      if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: 200 }));
      if (String(url).endsWith("/resource-tickets/media")) {
        ticketCalls += 1;
        if (ticketCalls === 1) return new Response(JSON.stringify({ urls: Object.fromEntries(canonical.media.map((asset) => [`remote-p/${asset.relativePath}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
        return new Response(JSON.stringify({ message: "Media temporarily unavailable" }), { status: 503 });
      }
      const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
      requests.push(request);
      const payload = request.payload as Record<string, unknown>;
      let result: unknown = canonical;
      if (request.operation === "apply_project_actions_to_split_project_folder") {
        canonical = { ...canonical, name: "edited", contentRevision: canonical.contentRevision! + 1, media: canonical.media.map((asset, i) => i === 0 ? { ...asset, relativePath: `media/edited-${canonical.contentRevision}.mp4` } : asset) };
        result = { project: canonical };
      } else if (request.operation === "save_split_project_to_folder") {
        canonical = { ...(payload.project as VideoProject), contentRevision: canonical.contentRevision! + 1 };
        result = { project: canonical };
      }
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result }));
    };
    const transport = new RemoteTransport({ csrfToken: "csrf", fetcher });
    backend.current = transport;
    const opened = await transport.request<VideoProject>("load_split_project_from_folder", { projectDir: "remote-p" });
    const store = createEditorStore({ projectDir: "remote-p", project: opened });
    const action = { type: "updateVisualClipOpacity" as const, itemId: fixtureItem(opened, "video").id, opacity: 0.5 };

    await store.getState().applyActions([action]);
    expect(store.getState()).toMatchObject({ project: { contentRevision: 4, name: "edited" }, saveStatus: "saved", lastError: null });
    expect(store.getState().history.past).toHaveLength(1);
    await vi.waitFor(() => expect(remoteMediaReadiness("remote-p").status).toBe("failed"));
    clearRemoteResourceUrlsForTests();
    await store.getState().undo();
    expect(store.getState().project.contentRevision).toBe(5);
    expect(store.getState().history.past).toHaveLength(0);
    expect(requests.at(-1)?.expectedRevision).toBe(4);

    canonical = { ...canonical, contentRevision: 6, name: "external" };
    const loaded = await transport.request<VideoProject>("load_split_project_from_folder", { projectDir: "remote-p" });
    await store.getState().mergeLoadedProject(loaded);
    expect(store.getState().project).toEqual(canonical);
    await store.getState().applyActions([action]);
    expect(requests.at(-1)?.expectedRevision).toBe(6);
    expect(store.getState()).toMatchObject({ project: { contentRevision: 7 }, saveStatus: "saved" });
    expect(requests.filter((request) => request.operation === "apply_project_actions_to_split_project_folder")).toHaveLength(2);
    expect(store.getState().history.past).toHaveLength(1);
  });
});
