import type {
  AppServerConversationEntry,
  AppServerConversationHistory,
  CodexConversationEditRequest,
  ProjectAgentSession,
  ProjectAgentSessionAction,
  ProjectAgentSessionManifest,
} from "../../project";

/**
 * The saved chats of the conversation fixture, kept in memory: the session manifest and the
 * app-server turn history, with the semantics of `apply_agent_session_action` and
 * `record_app_server_conversation_turn` in `src-tauri/src/project/split.rs`.
 */

const sessionSchemaVersion = 1;
const historySchemaVersion = 1;

export interface FixtureChats {
  manifest: ProjectAgentSessionManifest | null;
  readonly history: AppServerConversationEntry[];
}

export function createFixtureChats(): FixtureChats {
  return { manifest: null, history: [] };
}

export function sessionManifest(chats: FixtureChats, projectId: string): ProjectAgentSessionManifest {
  if (chats.manifest?.projectId !== projectId) {
    chats.manifest = { schemaVersion: sessionSchemaVersion, projectId, activeSessionId: null, sessions: [], deletedSessions: [] };
  }
  return chats.manifest;
}

export function conversationHistory(chats: FixtureChats): AppServerConversationHistory {
  return { schemaVersion: historySchemaVersion, entries: chats.history };
}

function session(id: string, title: string, threadId: string | null, timestamp: string): ProjectAgentSession {
  return { id, title, threadId, createdAt: timestamp, updatedAt: timestamp, turns: [], proposalStatus: "none", appliedActionIds: [] };
}

function findSession(manifest: ProjectAgentSessionManifest, sessionId: string): ProjectAgentSession {
  const found = manifest.sessions.find((candidate) => candidate.id === sessionId);
  if (!found) throw "agent session was not found";
  return found;
}

export function applySessionAction(chats: FixtureChats, projectId: string, action: ProjectAgentSessionAction): ProjectAgentSessionManifest {
  const manifest = sessionManifest(chats, projectId);
  switch (action.type) {
    case "create": {
      const exists = [...manifest.sessions, ...manifest.deletedSessions].some((candidate) => candidate.id === action.id);
      if (!action.id.trim() || !action.title.trim() || exists) throw "agent session id/title is invalid or already exists";
      manifest.sessions.push(session(action.id, action.title.trim(), action.threadId ?? null, action.timestamp));
      manifest.activeSessionId = action.id;
      break;
    }
    case "select":
      findSession(manifest, action.sessionId);
      manifest.activeSessionId = action.sessionId;
      break;
    case "rename": {
      const found = findSession(manifest, action.sessionId);
      if (!action.title.trim()) throw "agent session title cannot be empty";
      found.title = action.title.trim();
      found.updatedAt = action.timestamp;
      break;
    }
    case "delete": {
      const found = findSession(manifest, action.sessionId);
      manifest.sessions = manifest.sessions.filter((candidate) => candidate !== found);
      manifest.deletedSessions.push({ ...found, updatedAt: action.timestamp });
      if (manifest.activeSessionId === action.sessionId) manifest.activeSessionId = manifest.sessions[0]?.id ?? null;
      break;
    }
    case "restore": {
      const found = manifest.deletedSessions.find((candidate) => candidate.id === action.sessionId);
      if (!found) throw "deleted agent session was not found";
      manifest.deletedSessions = manifest.deletedSessions.filter((candidate) => candidate !== found);
      manifest.sessions.push({ ...found, updatedAt: action.timestamp });
      manifest.activeSessionId = found.id;
      break;
    }
  }
  return manifest;
}

/** The thread a turn runs on: the active chat's thread, or one named after the chat. */
export function turnThreadId(chats: FixtureChats, projectId: string): string {
  const manifest = sessionManifest(chats, projectId);
  const active = manifest.sessions.find((candidate) => candidate.id === manifest.activeSessionId);
  return active?.threadId ?? `fixture-thread-${active?.id ?? "1"}`;
}

/** Records a finished turn and files it under the chat on its thread, else the active chat, else a new chat. */
export function recordTurn(chats: FixtureChats, projectId: string, threadId: string, request: CodexConversationEditRequest, hasProposal: boolean): void {
  const id = `app-server-turn-${(chats.history.length + 1).toString()}`;
  const manifest = sessionManifest(chats, projectId);
  let target =
    manifest.sessions.find((candidate) => candidate.threadId === threadId) ??
    manifest.sessions.find((candidate) => candidate.id === manifest.activeSessionId);
  if (!target) {
    target = session(`agent-session-${(manifest.sessions.length + manifest.deletedSessions.length + 1).toString()}`, request.prompt.slice(0, 48), threadId, request.createdAt);
    manifest.sessions.push(target);
    manifest.activeSessionId = target.id;
  }
  chats.history.push({ id, projectId, threadId, turnId: id, turnStatus: "completed", prompt: request.prompt, request, hasProposal, threadResponse: {}, turnResponse: {}, sessionId: target.id });
  const proposalStatus = hasProposal ? "proposed" : "none";
  target.threadId = threadId;
  target.updatedAt = request.createdAt;
  target.proposalStatus = proposalStatus;
  target.turns.push({ id, createdAt: request.createdAt, fullTurn: { request }, toolResults: [], proposalStatus, appliedActionIds: [] });
}

/** Marks the session's last turn applied (with its action ids) or undone. */
export function markSessionTurn(chats: FixtureChats, projectId: string, sessionId: string | null, status: "applied" | "undone", actionIds: readonly string[] | null): string | null {
  const manifest = sessionManifest(chats, projectId);
  const target = manifest.sessions.find((candidate) => candidate.id === (sessionId ?? manifest.activeSessionId));
  if (!target) return null;
  target.proposalStatus = status;
  if (actionIds) target.appliedActionIds = [...actionIds];
  const turn = target.turns[target.turns.length - 1];
  if (turn) {
    turn.proposalStatus = status;
    if (actionIds) turn.appliedActionIds = [...actionIds];
  }
  return target.id;
}
