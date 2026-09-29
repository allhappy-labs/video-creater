import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppServerConversationEntry, ProjectAgentSession, ProjectAgentSessionAction, ProjectAgentSessionManifest, VideoProject } from "@/lib/project";
import { backendRequest } from "@/lib/runtime/backend-client";
import { BackendUnavailableError } from "@/lib/runtime/backend-transport";
import { fixtureProject } from "@/test-utils/editor-fixtures";
import { sessionHistoryEntries } from "./agent-session-actions";
import { createEditorStore } from "./editor-store";

vi.mock("@/lib/runtime/backend-client", () => ({ backendRequest: vi.fn(), backendListen: vi.fn(), backendMediaUrl: (p: string) => p }));

type Handler = (input: Record<string, unknown>) => unknown;

const projectDir = "/projects/demo";
const loadSessions = "load_agent_sessions_from_split_project_folder";
const sessionAction = "apply_agent_session_action_to_split_project_folder";
const loadHistory = "load_app_server_conversation_history_from_split_project_folder";

function session(id: string, title: string, createdAt: string, threadId: string | null = null): ProjectAgentSession {
  return { id, title, threadId, createdAt, updatedAt: createdAt, turns: [], proposalStatus: "none", appliedActionIds: [] };
}

function entry(id: string, prompt: string, createdAt: string | null, threadId = "thread-1"): AppServerConversationEntry {
  return {
    id,
    projectId: "project-1",
    threadId,
    prompt,
    request: createdAt ? { prompt, focus: { mediaIds: [], timelineItemIds: [] }, createdAt } : {},
    hasProposal: true,
    threadResponse: {},
    turnResponse: {},
  };
}

const pacing = session("session-a", "Pacing", "2026-09-15T09:00:00.000Z");
const captions = session("session-b", "Captions", "2026-09-15T10:00:00.000Z");
const entries = [
  entry("turn-1", "Tighten the pacing", "2026-09-15T09:30:00.000Z"),
  entry("turn-2", "Add clean captions", "2026-09-15T10:30:00.000Z"),
];

function manifest(activeSessionId: string | null, sessions = [pacing, captions]): ProjectAgentSessionManifest {
  return { schemaVersion: 1, projectId: "project-1", activeSessionId, sessions, deletedSessions: [] };
}

function splitProject(): VideoProject {
  return { ...fixtureProject(), id: "project-1", schemaVersion: 2, contentRevision: 3 };
}

function setup(dir = projectDir) {
  const backend = { manifest: manifest("session-b") };
  const handlers: Record<string, Handler> = {
    [loadSessions]: () => backend.manifest,
    [loadHistory]: () => ({ schemaVersion: 1, entries }),
    [sessionAction]: (input) => {
      const action = input.action as ProjectAgentSessionAction;
      if (action.type === "select") backend.manifest = { ...backend.manifest, activeSessionId: action.sessionId };
      if (action.type === "create") {
        const created = session(action.id, action.title, action.timestamp);
        backend.manifest = { ...backend.manifest, activeSessionId: action.id, sessions: [...backend.manifest.sessions, created] };
      }
      if (action.type === "rename") {
        backend.manifest = { ...backend.manifest, sessions: backend.manifest.sessions.map((s) => (s.id === action.sessionId ? { ...s, title: action.title } : s)) };
      }
      return backend.manifest;
    },
  };
  vi.mocked(backendRequest).mockImplementation(async (command: string, input?: Record<string, unknown>) => {
    const handler = handlers[command];
    if (!handler) throw new BackendUnavailableError();
    return handler(input ?? {});
  });
  return { backend, store: createEditorStore({ projectDir: dir, project: splitProject() }) };
}

function calls(command: string) {
  return vi.mocked(backendRequest).mock.calls.filter(([name]) => name === command);
}

describe("agent sessions", () => {
  beforeEach(() => {
    vi.mocked(backendRequest).mockReset();
    window.localStorage.clear();
  });

  it("loads sessions on mount with the active session's history", async () => {
    const { store } = setup();

    store.getState().startAgent();

    await vi.waitFor(() => expect(store.getState().agentMessages).toHaveLength(1));
    expect(store.getState().agentSessions?.activeSessionId).toBe("session-b");
    expect(store.getState().agentMessages).toEqual([{ role: "user", id: "turn-2", text: "Add clean captions", createdAt: "2026-09-15T10:30:00.000Z" }]);
  });

  it("switches, creates, and renames sessions through the session action command", async () => {
    const { store } = setup();
    await store.getState().loadAgentSessions();

    await expect(store.getState().selectAgentSession("session-a")).resolves.toBe(true);
    expect(calls(sessionAction)[0]?.[1]).toEqual({ projectDir, projectId: "project-1", action: { type: "select", sessionId: "session-a" } });
    expect(store.getState().agentMessages.map((message) => message.id)).toEqual(["turn-1"]);

    await store.getState().createAgentSession();
    expect(calls(sessionAction)[1]?.[1]).toMatchObject({ action: { type: "create", title: "New chat", threadId: null } });
    expect(store.getState().agentMessages).toEqual([]);

    const createdId = store.getState().agentSessions?.activeSessionId ?? "";
    await expect(store.getState().renameAgentSession(createdId, "  Titles  ")).resolves.toBe(true);
    expect(calls(sessionAction)[2]?.[1]).toMatchObject({ action: { type: "rename", sessionId: createdId, title: "Titles" } });
    await expect(store.getState().renameAgentSession(createdId, "   ")).resolves.toBe(false);

    // Switching back restores the earlier session's messages without reloading them.
    const historyLoads = calls(loadHistory).length;
    await store.getState().selectAgentSession("session-a");
    expect(store.getState().agentMessages.map((message) => message.id)).toEqual(["turn-1"]);
    expect(calls(loadHistory)).toHaveLength(historyLoads);
  });

  it("shows plain copy when a session action fails", async () => {
    const { store } = setup();
    await store.getState().loadAgentSessions();
    vi.mocked(backendRequest).mockRejectedValueOnce(new Error("agent session was not found"));

    await expect(store.getState().deleteAgentSession("session-z")).resolves.toBe(false);

    expect(store.getState().agentSessionError).toBe("The chat couldn't be deleted.");
  });

  it("skips sessions for browser projects", async () => {
    const { store } = setup("browser://sample");

    store.getState().startAgent();
    await expect(store.getState().createAgentSession()).resolves.toBe(false);

    expect(vi.mocked(backendRequest)).not.toHaveBeenCalled();
    expect(store.getState().agentSessions).toBeNull();
  });

  it("assigns history to sessions by thread, else by creation window", () => {
    const threaded = session("session-t", "Threaded", "2026-09-15T11:00:00.000Z", "thread-9");
    const all = [...entries, entry("turn-3", "Undated", null), entry("turn-9", "Other thread", "2026-09-15T08:00:00.000Z", "thread-9")];
    const sessions = manifest("session-a", [captions, pacing, threaded]);

    expect(sessionHistoryEntries(all, sessions, "session-a").map((item) => item.id)).toEqual(["turn-1", "turn-3", "turn-9"]);
    expect(sessionHistoryEntries(all, sessions, "session-b").map((item) => item.id)).toEqual(["turn-2"]);
    expect(sessionHistoryEntries(all, sessions, "session-t").map((item) => item.id)).toEqual(["turn-9"]);
    expect(sessionHistoryEntries(all, manifest(null, []), null)).toHaveLength(4);
  });

  it("assigns history by recorded session, and infers only for entries without one", () => {
    const threaded = session("session-t", "Threaded", "2026-09-15T11:00:00.000Z", "thread-9");
    const removed = session("session-d", "Removed", "2026-09-15T12:00:00.000Z");
    const recorded = { ...entry("turn-4", "Recorded in captions", "2026-09-15T09:45:00.000Z", "thread-9"), sessionId: "session-b" };
    const orphaned = { ...entry("turn-5", "Recorded in a deleted chat", "2026-09-15T12:30:00.000Z"), sessionId: "session-d" };
    const legacy = entry("turn-6", "Legacy", "2026-09-15T09:50:00.000Z");
    const all = [recorded, orphaned, legacy];
    const sessions = { ...manifest("session-b", [pacing, captions, threaded]), deletedSessions: [removed] };

    expect(sessionHistoryEntries(all, sessions, "session-b").map((item) => item.id)).toEqual(["turn-4"]);
    expect(sessionHistoryEntries(all, sessions, "session-t").map((item) => item.id)).toEqual([]);
    expect(sessionHistoryEntries(all, sessions, "session-d").map((item) => item.id)).toEqual(["turn-5"]);
    expect(sessionHistoryEntries(all, sessions, "session-a").map((item) => item.id)).toEqual(["turn-6"]);
    expect(sessionHistoryEntries(all, manifest(null, []), null)).toHaveLength(3);
  });
});
