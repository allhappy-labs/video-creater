import { initialConversationState, isConversationBusy } from "@/lib/agent/conversation-state";
import {
  applyAgentSessionActionToSplitProjectFolder,
  loadAgentSessionsFromSplitProjectFolder,
  loadAppServerConversationHistoryFromSplitProjectFolder,
  type AppServerConversationEntry,
  type ProjectAgentSessionAction,
  type ProjectAgentSessionManifest,
} from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import type { AgentMessage, AgentRuntime } from "./agent-slice";
import type { EditorState, EditorStore } from "./editor-store";
import { pollable } from "./jobs-slice";

export interface AgentSessionActions {
  /** Loads the chat sessions and the active session's history; split project folders only. */
  loadAgentSessions(): Promise<void>;
  createAgentSession(title?: string): Promise<boolean>;
  selectAgentSession(sessionId: string): Promise<boolean>;
  renameAgentSession(sessionId: string, title: string): Promise<boolean>;
  deleteAgentSession(sessionId: string): Promise<boolean>;
  restoreAgentSession(sessionId: string): Promise<boolean>;
}

const newChatTitle = "New chat";

const sessionFailureCopy: Record<ProjectAgentSessionAction["type"], string> = {
  create: "A new chat couldn't be started.",
  select: "That chat couldn't be opened.",
  rename: "The chat couldn't be renamed.",
  delete: "The chat couldn't be deleted.",
  restore: "The chat couldn't be restored.",
};

function requestCreatedAt(entry: AppServerConversationEntry): number | null {
  const createdAt = (entry.request as { createdAt?: unknown }).createdAt;
  const time = typeof createdAt === "string" ? Date.parse(createdAt) : Number.NaN;
  return Number.isFinite(time) ? time : null;
}

/**
 * History entries of one session. A turn that recorded its session belongs to that session only.
 * Older turns didn't record one, so they are inferred: a session with a Codex thread matches by
 * thread; otherwise it owns the turns sent between its creation and the next session's (the oldest
 * session also owns undated and earlier turns). No sessions: all turns.
 */
export function sessionHistoryEntries(
  entries: readonly AppServerConversationEntry[],
  manifest: ProjectAgentSessionManifest | null,
  sessionId: string | null,
): AppServerConversationEntry[] {
  const sessions = [...(manifest?.sessions ?? []), ...(manifest?.deletedSessions ?? [])].sort(
    (left, right) => Date.parse(left.createdAt) - Date.parse(right.createdAt),
  );
  const index = sessions.findIndex((session) => session.id === sessionId);
  const session = sessions[index];
  if (!session) return sessionId === null ? [...entries] : [];
  const start = index === 0 ? Number.NEGATIVE_INFINITY : Date.parse(session.createdAt);
  const next = sessions[index + 1];
  const end = next ? Date.parse(next.createdAt) : Number.POSITIVE_INFINITY;
  const inferredMatch = (entry: AppServerConversationEntry): boolean => {
    if (session.threadId) return entry.threadId === session.threadId;
    const time = requestCreatedAt(entry) ?? Number.NEGATIVE_INFINITY;
    return time >= start && time < end;
  };
  return entries.filter((entry) => (typeof entry.sessionId === "string" ? entry.sessionId === session.id : inferredMatch(entry)));
}

/** Earlier turns show as the user's words; their cards are not stored. */
function historyMessages(entries: readonly AppServerConversationEntry[]): AgentMessage[] {
  return entries
    .filter((entry) => entry.prompt.trim())
    .map((entry): AgentMessage => {
      const time = requestCreatedAt(entry);
      return { role: "user", id: entry.id, text: entry.prompt, createdAt: time === null ? "" : new Date(time).toISOString() };
    });
}

export function createAgentSessionActions(
  set: EditorStore["setState"],
  get: () => EditorState,
  runtime: AgentRuntime,
): AgentSessionActions {
  const available = () => pollable(get().project, get().projectDir);

  /** Switching sessions mid-turn would strand the turn's cards, so it waits for the turn. */
  const turnRunning = () =>
    isConversationBusy(get().agentConversation) || get().agentMessages.some((message) => message.role === "assistant" && message.status === "applying");

  async function loadHistory(manifest: ProjectAgentSessionManifest, sessionId: string | null): Promise<void> {
    const { projectDir } = get();
    try {
      const history = await loadAppServerConversationHistoryFromSplitProjectFolder({ projectDir });
      const current = get();
      // A switch or a new message since the request makes this history stale.
      if ((current.agentSessions?.activeSessionId ?? null) !== sessionId || current.agentMessages.length > 0) return;
      const entries = Array.isArray(history.entries) ? history.entries : [];
      set({ agentMessages: historyMessages(sessionHistoryEntries(entries, manifest, sessionId)) });
    } catch (error) {
      if (!isBackendUnavailableError(error)) set({ agentSessionError: "Earlier messages couldn't be loaded." });
    }
  }

  async function show(manifest: ProjectAgentSessionManifest): Promise<void> {
    const state = get();
    const next = manifest.activeSessionId ?? null;
    const previous = state.agentSessions ? (state.agentSessions.activeSessionId ?? null) : undefined;
    if (previous === next) {
      set({ agentSessions: manifest, agentSessionError: null });
      return;
    }
    if (previous === undefined) {
      // First load: messages sent before sessions arrived stay in the conversation.
      set({ agentSessions: manifest, agentSessionError: null });
      if (state.agentMessages.length === 0) await loadHistory(manifest, next);
      return;
    }
    if (previous !== null) runtime.messagesBySession.set(previous, state.agentMessages);
    const cached = next === null ? undefined : runtime.messagesBySession.get(next);
    set({ agentSessions: manifest, agentSessionError: null, agentMessages: cached ?? [], agentConversation: initialConversationState });
    if (!cached) await loadHistory(manifest, next);
  }

  async function run(action: ProjectAgentSessionAction): Promise<boolean> {
    if (!available()) return false;
    if (action.type !== "rename" && turnRunning()) return false;
    const { project, projectDir } = get();
    try {
      await show(await applyAgentSessionActionToSplitProjectFolder({ projectDir, projectId: project.id, action }));
      return true;
    } catch (error) {
      set({ agentSessionError: isBackendUnavailableError(error) ? "Chats need the desktop app." : sessionFailureCopy[action.type] });
      return false;
    }
  }

  const now = () => new Date().toISOString();

  return {
    async loadAgentSessions() {
      if (!available()) return;
      try {
        await show(await loadAgentSessionsFromSplitProjectFolder({ projectDir: get().projectDir }));
      } catch (error) {
        if (!isBackendUnavailableError(error)) set({ agentSessionError: "Chats couldn't be loaded." });
      }
    },
    createAgentSession: (title = newChatTitle) =>
      run({ type: "create", id: runtime.nextId("session"), title: title.trim() || newChatTitle, threadId: null, timestamp: now() }),
    selectAgentSession: (sessionId) => run({ type: "select", sessionId }),
    async renameAgentSession(sessionId, title) {
      if (!title.trim()) return false;
      return run({ type: "rename", sessionId, title: title.trim(), timestamp: now() });
    },
    deleteAgentSession: (sessionId) => run({ type: "delete", sessionId, timestamp: now() }),
    restoreAgentSession: (sessionId) => run({ type: "restore", sessionId, timestamp: now() }),
  };
}
