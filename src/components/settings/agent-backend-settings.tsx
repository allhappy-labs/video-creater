import { useState } from "react";
import { Bot, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import type {
  AgentBackend,
  AppPreferenceIntent,
  AppSettingsPreferences,
  ClaudeModel,
} from "@/lib/app-settings";
import { getAgentHealth, runAgentComponentSelfTest } from "@/lib/settings/agent";
import type { SettingsComponentHealth } from "@/lib/settings/health";

export interface AgentBackendSettingsProps {
  preferences: AppSettingsPreferences;
  onChange: (patch: AppPreferenceIntent) => void;
}

const CODEX_COMPONENT_ID = "agent.codex";
const CLAUDE_COMPONENT_ID = "agent.claude";

const backendOptions: ReadonlyArray<{ value: AgentBackend; label: string }> = [
  { value: "automatic", label: "Automatic" },
  { value: "codex", label: "Codex" },
  { value: "claude", label: "Claude" },
];

const modelOptions: ReadonlyArray<{ value: ClaudeModel; label: string }> = [
  { value: "sonnet", label: "Sonnet" },
  { value: "haiku", label: "Haiku" },
  { value: "opus", label: "Opus" },
];

const controlClassName =
  "h-8 rounded-md border border-input bg-background px-2 text-xs text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-not-allowed disabled:opacity-60";

type ResolvedBackend = "codex" | "claude" | null;

const backendNames = { codex: "Codex", claude: "Claude" } as const;

/**
 * Mirrors the backend's turn-time resolution (`resolve_agent_backend` in
 * `src-tauri/src/agent/mod.rs`): a preference is honoured when that agent is
 * ready, and otherwise yields to the one that is, so a turn is never refused
 * while a working agent is installed. Automatic prefers Claude, which runs on
 * the user's own subscription with nothing bundled.
 */
function resolveAgentBackend(
  preference: AgentBackend,
  codexReady: boolean,
  claudeReady: boolean,
): ResolvedBackend {
  const order =
    preference === "codex"
      ? (["codex", "claude"] as const)
      : (["claude", "codex"] as const);
  const ready = { codex: codexReady, claude: claudeReady };
  return order.find((backend) => ready[backend]) ?? null;
}

/**
 * What the current choice means. Readiness is unknown until the user asks for it,
 * because opening a preference page must not start agent processes.
 */
function agentResolutionMessage(
  preference: AgentBackend,
  resolved: ResolvedBackend | "unknown",
): string {
  if (resolved === "unknown") {
    return preference === "automatic"
      ? "Automatic uses whichever agent is ready."
      : `Turns will use ${backendNames[preference]} when it is ready.`;
  }
  if (!resolved) return "No agent is available. Install Claude or Codex.";
  if (preference === "automatic") {
    return `Automatic is using ${backendNames[resolved]}.`;
  }
  if (preference !== resolved) {
    return `${backendNames[preference]} isn't ready, so turns will use ${backendNames[resolved]}.`;
  }
  return `Turns will use ${backendNames[resolved]}.`;
}

function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return typeof error === "string" ? error : String(error);
}

export function AgentBackendSettings({
  preferences,
  onChange,
}: AgentBackendSettingsProps) {
  const [items, setItems] = useState<SettingsComponentHealth[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [executablePath, setExecutablePath] = useState(
    preferences.claudeExecutablePath,
  );

  async function loadHealth() {
    const health = await getAgentHealth(null);
    setItems(health.items);
    setError(null);
  }

  async function runCheck(busy: string, before?: () => Promise<unknown>) {
    if (busyId) return;
    setBusyId(busy);
    try {
      await before?.();
      await loadHealth();
    } catch (checkError) {
      setError(errorMessage(checkError));
    } finally {
      setBusyId(null);
    }
  }

  const rows = [CODEX_COMPONENT_ID, CLAUDE_COMPONENT_ID].flatMap((id) => {
    const item = items?.find((candidate) => candidate.id === id);
    return item ? [item] : [];
  });
  const isReady = (id: string) =>
    items?.some((item) => item.id === id && item.state === "ready") ?? false;
  const resolved = items
    ? resolveAgentBackend(
        preferences.agentBackend,
        isReady(CODEX_COMPONENT_ID),
        isReady(CLAUDE_COMPONENT_ID),
      )
    : ("unknown" as const);
  const resolvedPath =
    rows.find((item) => item.id === CLAUDE_COMPONENT_ID)?.provenance.executable ??
    "";

  function commitExecutablePath() {
    const trimmed = executablePath.trim();
    setExecutablePath(trimmed);
    if (trimmed !== preferences.claudeExecutablePath) {
      onChange({ claudeExecutablePath: trimmed });
    }
  }

  return (
    <section
      data-settings-target="advanced:agent"
      tabIndex={-1}
      aria-labelledby="advanced-agent-heading"
      className="grid gap-3 border-t py-3 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring md:grid-cols-[minmax(12rem,0.9fr)_minmax(16rem,1.1fr)]"
    >
      <div className="flex min-w-0 items-start gap-2">
        <Bot className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <div>
          <h2 id="advanced-agent-heading" className="text-xs font-semibold">Agent</h2>
          <p className="text-[11px] leading-5 text-muted-foreground">
            Choose which agent answers AI requests, and check that it is ready.
          </p>
        </div>
      </div>

      <div className="grid min-w-0 gap-2 md:justify-items-end">
        <select
          aria-label="Agent backend"
          value={preferences.agentBackend}
          className={controlClassName}
          onChange={(event) =>
            onChange({ agentBackend: event.currentTarget.value as AgentBackend })
          }
        >
          {backendOptions.map((option) => (
            <option key={option.value} value={option.value}>{option.label}</option>
          ))}
        </select>
        <span
          className={`text-[11px] ${
            resolved === null
              ? "text-red-700"
              : resolved !== "unknown" && resolved !== preferences.agentBackend
                ? "text-amber-700"
                : "text-muted-foreground"
          }`}
        >
          {agentResolutionMessage(preferences.agentBackend, resolved)}
        </span>

        <select
          aria-label="Claude model"
          value={preferences.claudeModel}
          disabled={resolved !== "claude" && resolved !== "unknown"}
          className={controlClassName}
          onChange={(event) =>
            onChange({ claudeModel: event.currentTarget.value as ClaudeModel })
          }
        >
          {modelOptions.map((option) => (
            <option key={option.value} value={option.value}>{option.label}</option>
          ))}
        </select>

        <input
          type="text"
          aria-label="Claude executable path"
          value={executablePath}
          placeholder={resolvedPath || "Resolved from PATH"}
          spellCheck={false}
          className={`w-full max-w-xs font-mono ${controlClassName}`}
          onChange={(event) => setExecutablePath(event.currentTarget.value)}
          onBlur={commitExecutablePath}
        />
      </div>

      <div className="grid gap-2 md:col-span-2">
        <div className="flex items-center justify-between gap-2">
          <p className="text-[11px] text-muted-foreground">
            {items
              ? "Readiness was checked on request."
              : "Readiness is checked on request, so opening settings starts no agent."}
          </p>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-2"
            disabled={busyId !== null}
            onClick={() => void runCheck("all")}
          >
            {busyId === "all" ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
            ) : null}
            Check agents
          </Button>
        </div>
        {rows.map((item) => (
          <div
            key={item.id}
            role="status"
            className="flex flex-wrap items-center justify-between gap-2 border-t pt-2"
          >
            <div className="min-w-0">
              <h3 className="font-semibold text-foreground">{item.label}</h3>
              <p className="text-[11px] leading-5 text-muted-foreground">{item.summary}</p>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 px-2"
              disabled={busyId !== null}
              onClick={() =>
                void runCheck(item.id, () => runAgentComponentSelfTest(item.id))
              }
              aria-label={`Re-check ${item.label}`}
            >
              {busyId === item.id ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none" aria-hidden="true" />
              ) : null}
              Re-check
            </Button>
          </div>
        ))}
        {error ? (
          <p role="alert" className="border-l-2 border-red-500 pl-2 text-red-700">
            Agent readiness could not be checked: {error}
          </p>
        ) : null}
      </div>
    </section>
  );
}
