import { RotateCcw } from "lucide-react";
import { conversationFailureLabel } from "@/lib/agent/conversation-state";
import type { AgentAssistantMessage, AgentFailure } from "../../store/agent-slice";
import { useEditorStoreApi } from "../../store/editor-store-context";
import { CardActions, CardButton, ResultCard } from "./card-parts";
import { MissingAgentState } from "./missing-agent-state";

function withoutFinalPunctuation(text: string): string {
  return text.trim().replace(/[.!?\s]+$/, "");
}

/** What failed and the next step, in plain language. */
function failureCopy(failure: AgentFailure): string {
  switch (failure.kind) {
    case "validationBlocked":
      return `The edit couldn't be validated: ${withoutFinalPunctuation(failure.message)}. Try rephrasing or narrowing the request.`;
    case "applyFailed":
      return `Nothing was changed. ${failure.message.trim()}`;
    case "turnFailed":
    case "agentUnavailable":
      return failure.message.trim();
  }
}

interface FailureCardProps {
  readonly message: AgentAssistantMessage;
  readonly failure: AgentFailure;
  /** Retry only runs for the latest turn, and only when another attempt is safe. */
  readonly canRetry: boolean;
}

export function FailureCard({ message, failure, canRetry }: FailureCardProps) {
  const store = useEditorStoreApi();
  if (failure.kind === "agentUnavailable") return <MissingAgentState />;
  return (
    <ResultCard status={conversationFailureLabel(failure.kind)} tone="danger">
      <p className="break-words px-3 pb-2.5 text-[12px] text-muted-foreground">{failureCopy(failure)}</p>
      {canRetry && failure.retryable && (
        <CardActions>
          <CardButton onClick={() => void store.getState().retryAgentTurn(message.id)}>
            <RotateCcw className="h-3.5 w-3.5" aria-hidden />
            Retry
          </CardButton>
        </CardActions>
      )}
    </ResultCard>
  );
}
