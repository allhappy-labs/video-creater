import "@testing-library/jest-dom/vitest";
import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { VideoProject } from "@/lib/project";
import { RemoteTransport } from "@/lib/runtime/adapters/remote-transport";
import { clearRemoteProjectAccessForTests } from "@/lib/runtime/adapters/remote-project-access";
import { clearRemoteResourceUrlsForTests, remoteMediaReadiness } from "@/lib/runtime/adapters/remote-resource-cache";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { renderWithEditorStore } from "@/test-utils/editor-render";

const backend = vi.hoisted(() => ({ current: null as RemoteTransport | null }));
vi.mock("@/lib/runtime/backend-client", () => ({
  backendRequest: (operation: string, input: Record<string, unknown>) => backend.current!.request(operation, input),
  backendListen: vi.fn(),
  backendMediaUrl: (path: string) => backend.current!.mediaUrl(path),
}));
const { PreviewPanel } = await import("./preview-panel");

beforeEach(() => {
  clearRemoteProjectAccessForTests();
  clearRemoteResourceUrlsForTests();
});

it("renders a recoverable media error and refreshes timeline and source URLs without another edit", async () => {
  const project = { ...fixtureProject(), schemaVersion: 2, contentRevision: 4 };
  let unavailable = true;
  const fetcher = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: 200 }));
    if (String(url).endsWith("/resource-tickets/media")) {
      if (unavailable) return new Response(JSON.stringify({ message: "Project media could not be loaded." }), { status: 503 });
      return new Response(JSON.stringify({ urls: Object.fromEntries(project.media.map((asset) => [`remote-p/${asset.relativePath}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
    }
    const request = JSON.parse(String(init?.body)) as Record<string, unknown>;
    return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: project }));
  });
  backend.current = new RemoteTransport({ csrfToken: "csrf", fetcher });
  const opened = await backend.current.request<VideoProject>("load_split_project_from_folder", { projectDir: "remote-p" });
  await vi.waitFor(() => expect(remoteMediaReadiness("remote-p").status).toBe("failed"));
  const { store } = renderWithEditorStore(<PreviewPanel />, { projectDir: "remote-p", project: opened });

  expect(screen.getByRole("alert", { name: "Project media issue" })).toHaveTextContent("Project media could not be loaded.");
  expect(screen.queryByLabelText("Timeline video Opening clip")).not.toBeInTheDocument();
  unavailable = false;
  fireEvent.click(screen.getByRole("button", { name: "Retry media" }));
  await waitFor(() => expect(screen.getByLabelText("Timeline video Opening clip")).toHaveAttribute("src", "/api/v1/media/0123456789abcdef0123456789abcdef"));
  expect(screen.queryByRole("alert", { name: "Project media issue" })).not.toBeInTheDocument();
  expect(store.getState().project).toBe(opened);
  expect(store.getState().saveStatus).toBe("saved");
  expect(store.getState().history.past).toHaveLength(0);
  act(() => store.getState().previewAsset("media-1"));
  expect(screen.getByLabelText("Video preview input.mp4")).toHaveAttribute("src", "/api/v1/media/0123456789abcdef0123456789abcdef");
  expect(fetcher.mock.calls.filter(([url]) => url === "/api/v1/rpc")).toHaveLength(1);
});

it("shows an unconfirmed edit and refreshes canonical state without replaying it", async () => {
  vi.useFakeTimers();
  try {
    let canonical = { ...fixtureProject(), schemaVersion: 2, contentRevision: 3 };
    const operations: string[] = [];
    let mutations = 0;
    backend.current = new RemoteTransport({ csrfToken: "csrf", deadlines: { mutation: 10 }, fetcher: async (url, init) => {
      if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
      if (String(url).endsWith("/resource-tickets/media")) return new Response(JSON.stringify({ urls: Object.fromEntries(canonical.media.map((asset) => [`remote-p/${asset.relativePath}`, "/api/v1/media/0123456789abcdef0123456789abcdef"])) }));
      const request = JSON.parse(String(init?.body)) as { requestId: string; operation: string };
      operations.push(request.operation);
      if (request.operation === "apply_project_actions_to_split_project_folder") {
        mutations += 1;
        canonical = { ...canonical, contentRevision: canonical.contentRevision + 1, name: "Host edit" };
        if (mutations === 1) return new Promise<Response>(() => undefined);
        return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: { project: canonical } }));
      }
      return new Response(JSON.stringify({ requestId: request.requestId, ok: true, result: canonical }));
    } });
    const opened = await backend.current.request<VideoProject>("load_split_project_from_folder", { projectDir: "remote-p" });
    const { store } = renderWithEditorStore(<PreviewPanel />, { projectDir: "remote-p", project: opened });
    const action = { type: "updateVisualClipOpacity" as const, itemId: "item-1", opacity: 0.5 };
    let edit!: Promise<VideoProject | null>;
    act(() => { edit = store.getState().applyActions([action]); });
    await act(async () => { await vi.advanceTimersByTimeAsync(11); await edit; });
    expect(store.getState()).toMatchObject({ saveStatus: "uncertain", project: { contentRevision: 3 } });
    expect(screen.getByRole("alert", { name: "Unconfirmed edit" })).toHaveTextContent("may have completed");
    fireEvent.click(screen.getByRole("button", { name: "Refresh project" }));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(store.getState()).toMatchObject({ saveStatus: "saved", project: { contentRevision: 4, name: "Host edit" } });
    expect(screen.queryByRole("alert", { name: "Unconfirmed edit" })).not.toBeInTheDocument();
    expect(mutations).toBe(1);
    expect(operations).toContain("read_project_snapshot_from_split_project_folder");
    await act(async () => { await store.getState().applyActions([action]); });
    expect(store.getState().project.contentRevision).toBe(5);
    expect(mutations).toBe(2);
  } finally { vi.useRealTimers(); }
});

it("renews a failed media ticket once and leaves repeated load failures for explicit Retry", async () => {
  const canonical = { ...fixtureProject(), schemaVersion: 2, contentRevision: 3 };
  let ticketBatches = 0;
  let rpcCalls = 0;
  backend.current = new RemoteTransport({ csrfToken: "csrf", fetcher: async (url, init) => {
    if (String(url).endsWith("/lease")) return new Response(JSON.stringify({ mode: "editor", editorLeaseToken: "lease", expiresAt: Date.now() / 1000 + 30 }));
    if (String(url).endsWith("/resource-tickets/media")) {
      ticketBatches += 1;
      return new Response(JSON.stringify({ urls: Object.fromEntries(canonical.media.map((asset, i) => [`remote-p/${asset.relativePath}`, `/api/v1/media/${`${ticketBatches}${i}`.padStart(32, "0")}`])) }));
    }
    rpcCalls += 1;
    return new Response(JSON.stringify({ requestId: (JSON.parse(String(init?.body)) as { requestId: string }).requestId, ok: true, result: canonical }));
  } });
  const opened = await backend.current.request<VideoProject>("load_split_project_from_folder", { projectDir: "remote-p" });
  await vi.waitFor(() => expect(remoteMediaReadiness("remote-p").status).toBe("ready"));
  renderWithEditorStore(<PreviewPanel />, { projectDir: "remote-p", project: opened });
  fireEvent.error(screen.getByLabelText("Timeline video Opening clip"));
  await waitFor(() => expect(screen.getByLabelText("Timeline video Opening clip")).toHaveAttribute("src", "/api/v1/media/00000000000000000000000000000020"));
  fireEvent.error(screen.getByLabelText("Timeline video Opening clip"));
  expect(screen.getByRole("alert", { name: "Preview issue" })).toHaveTextContent("Preview failed");
  expect(ticketBatches).toBe(2);
  fireEvent.click(screen.getByRole("button", { name: "Retry preview" }));
  await waitFor(() => expect(screen.getByLabelText("Timeline video Opening clip")).toHaveAttribute("src", "/api/v1/media/00000000000000000000000000000030"));
  expect(rpcCalls).toBe(1);
});
