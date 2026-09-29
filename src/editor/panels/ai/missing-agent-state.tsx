import { PlugZap } from "lucide-react";
import { useEditorEnvironment } from "../../services/editor-environment";

/** Settings > Advanced > Agent, where the agent is chosen and checked. */
const agentSettingsTarget = { category: "advanced", item: "agent" } as const;

/** Shown when no agent can be reached: what happened and where to set one up. */
export function MissingAgentState() {
  const { openSettings } = useEditorEnvironment();
  return (
    <section aria-label="AI agent unavailable" className="mt-2 flex flex-col items-start gap-2 rounded-[10px] bg-raised p-3">
      <div className="flex items-center gap-2">
        <PlugZap className="h-4 w-4 shrink-0 text-warning" aria-hidden />
        <p className="text-[13px] font-semibold text-foreground">Agent unavailable</p>
      </div>
      <p className="text-[12px] text-muted-foreground">
        No AI agent is available. Open Agent settings to choose one and check that it is ready.
      </p>
      {openSettings && (
        <button
          type="button"
          onClick={(event) => openSettings(agentSettingsTarget, event.currentTarget)}
          className="h-8 rounded-control bg-panel px-3 text-[12.5px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Open Agent settings
        </button>
      )}
    </section>
  );
}
