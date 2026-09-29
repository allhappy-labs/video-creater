import { useEffect, useLayoutEffect, useRef } from "react";
import type { AgentAssistantMessage, AgentProposalCard } from "../../store/agent-slice";
import { useEditorStoreApi } from "../../store/editor-store-context";
import { CardActions, CardButton, FactChips, ResultCard } from "./card-parts";

/** Review cards already focused once, so remounting (switching tabs) doesn't pull focus again. */
const focusedReviews = new Set<string>();

function primaryLabel(card: AgentProposalCard): string {
  return card.prepared.actions.some((action) => action.type === "recordGeneratedAsset") ? "Generate & place" : "Apply";
}

interface ReviewCardProps {
  readonly message: AgentAssistantMessage;
  readonly card: AgentProposalCard;
  /** Prefills the composer with "Revise: " and focuses it. */
  onRevise(): void;
  /** Called when the card goes away (applied, dismissed) while it holds focus, to return focus to the composer. */
  onFocusLost(): void;
}

/**
 * An edit waiting for the user: facts, placements, the primary action, Revise and Dismiss. The user
 * has to act, so focus moves to the primary action when the card appears.
 */
export function ReviewCard({ message, card, onRevise, onFocusLost }: ReviewCardProps) {
  const store = useEditorStoreApi();
  const rootRef = useRef<HTMLElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);
  const onFocusLostRef = useRef(onFocusLost);
  onFocusLostRef.current = onFocusLost;
  const applying = message.status === "applying";

  // Layout cleanup runs before React detaches the card, so focus inside it is still detectable. Once
  // the card is really gone (not a Strict Mode re-run), the browser has moved focus to the body, or a
  // modal sheet's focus trap has moved it to the sheet around the conversation.
  useLayoutEffect(() => {
    const root = rootRef.current;
    const anchor = root?.parentElement ?? null;
    return () => {
      if (!root?.contains(document.activeElement)) return;
      requestAnimationFrame(() => {
        const active = document.activeElement;
        const lost = active === null || active === document.body || (anchor !== null && active.contains(anchor));
        if (!root.isConnected && lost) onFocusLostRef.current();
      });
    };
  }, []);

  useEffect(() => {
    if (focusedReviews.has(message.id) || applying) return;
    focusedReviews.add(message.id);
    primaryRef.current?.focus();
  }, [message.id, applying]);

  return (
    <ResultCard ref={rootRef} status={applying ? "Applying changes" : "Needs your review"} tone="warning">
      <FactChips facts={card.facts} />
      {card.placements.length > 0 && (
        <ol aria-label="Planned changes" className="flex flex-col gap-1.5 px-3 pb-2.5 text-[12px] text-muted-foreground">
          {card.placements.map((line, index) => (
            <li key={`${index.toString()}-${line}`} className="break-words">
              {card.placements.length > 1 ? `${(index + 1).toString()} · ${line}` : line}
            </li>
          ))}
        </ol>
      )}
      <CardActions>
        <CardButton
          ref={primaryRef}
          variant="primary"
          aria-disabled={applying || undefined}
          onClick={() => {
            if (!applying) void store.getState().approveAgentProposal(message.id);
          }}
        >
          {applying ? "Applying…" : primaryLabel(card)}
        </CardButton>
        {!applying && (
          <>
            <CardButton onClick={onRevise}>Revise</CardButton>
            <CardButton variant="ghost" onClick={() => store.getState().rejectAgentProposal(message.id)}>
              Dismiss
            </CardButton>
          </>
        )}
      </CardActions>
    </ResultCard>
  );
}
