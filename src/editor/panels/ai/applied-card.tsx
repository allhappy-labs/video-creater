import { Undo2 } from "lucide-react";
import { useId } from "react";
import type { AgentAssistantMessage, AgentProposalCard } from "../../store/agent-slice";
import { useEditorStoreApi } from "../../store/editor-store-context";
import { CardActions, CardButton, FactChips, ResultCard } from "./card-parts";
import { ResultFrames } from "./result-frames";
import { useOpenViewer, useShowChanges } from "./show-changes";

interface AppliedCardProps {
  readonly message: AgentAssistantMessage;
  readonly card: AgentProposalCard & { readonly result: NonNullable<AgentProposalCard["result"]> };
}

/**
 * An applied (or undone) agent edit: status, facts, post-apply frames, Show changes and Undo.
 * It never takes focus; the composer keeps it.
 */
export function AppliedCard({ message, card }: AppliedCardProps) {
  const store = useEditorStoreApi();
  const reasonId = useId();
  const showChanges = useShowChanges();
  const openViewer = useOpenViewer();
  const { result, undo } = card;
  const undone = message.status === "undone";
  const undoBlocked = !undo.available || undo.pending;

  if (undone) {
    return (
      <ResultCard status="Undone" tone="neutral">
        <p className="px-3 pb-3 text-[12px] text-muted-foreground">The project is back to how it was before this edit.</p>
      </ResultCard>
    );
  }

  return (
    <ResultCard status={`Applied to ${result.timelineName}`} tone="success">
      <FactChips facts={card.facts} />
      <ResultFrames result={result} onOpenViewer={openViewer} />
      <CardActions>
        <CardButton onClick={() => showChanges(result.impact)}>Show changes</CardButton>
        <CardButton
          variant="ghost"
          aria-disabled={undoBlocked || undefined}
          aria-describedby={undo.reason ? reasonId : undefined}
          onClick={() => {
            if (!undoBlocked) void store.getState().undoAgentEdit(message.id);
          }}
        >
          <Undo2 className="h-3.5 w-3.5" aria-hidden />
          {undo.pending ? "Undoing…" : "Undo"}
        </CardButton>
      </CardActions>
      {undo.reason && (
        <p id={reasonId} className="px-3 pb-3 text-[12px] text-muted-foreground">
          {undo.reason}
        </p>
      )}
      {undo.error && (
        <p role="alert" className="px-3 pb-3 text-[12px] text-destructive">
          {undo.error}
        </p>
      )}
    </ResultCard>
  );
}
