import { useEffect, useRef, type ReactNode } from "react";
import { conversationStatusLabel, isConversationBusy } from "@/lib/agent/conversation-state";
import { parseMentions } from "@/lib/agent/mentions";
import type { AgentAssistantMessage, AgentMessage, AgentUserMessage } from "../../store/agent-slice";
import { useEditorStore } from "../../store/editor-store-context";
import { AppliedCard } from "./applied-card";
import { FailureCard } from "./failure-card";
import { ProgressRow } from "./progress-row";
import { ReviewCard } from "./review-card";
import { VariationResultCard } from "./variation-result-card";

/** The user's words, with each mention shown inline by its human name. */
function UserBubble({ message }: { readonly message: AgentUserMessage }) {
  const parts: ReactNode[] = [];
  let cursor = 0;
  for (const mention of parseMentions(message.text)) {
    if (mention.start > cursor) parts.push(message.text.slice(cursor, mention.start));
    parts.push(
      <span key={mention.start} className="font-medium text-primary">
        @{mention.name}
      </span>,
    );
    cursor = mention.end;
  }
  if (cursor < message.text.length) parts.push(message.text.slice(cursor));
  return (
    <li className="max-w-[88%] self-end whitespace-pre-wrap break-words rounded-[14px_14px_4px_14px] bg-raised px-3 py-2 text-[13px] text-foreground">
      <span className="sr-only">You: </span>
      {parts}
    </li>
  );
}

interface AssistantProps {
  readonly message: AgentAssistantMessage;
  readonly latestTurnId: string | null;
  onRevise(): void;
  onFocusLost(): void;
}

function AssistantReply({ message, latestTurnId, onRevise, onFocusLost }: AssistantProps) {
  const { card, failure } = message;
  let body: ReactNode = null;
  if (message.status === "failed" && failure) {
    body = <FailureCard message={message} failure={failure} canRetry={latestTurnId === message.replyTo} />;
  } else if (card && (message.status === "awaitingReview" || (message.status === "applying" && card.approved))) {
    body = <ReviewCard message={message} card={card} onRevise={onRevise} onFocusLost={onFocusLost} />;
  } else if (card?.result && (message.status === "applied" || message.status === "undone")) {
    body = (
      <>
        <AppliedCard message={message} card={{ ...card, result: card.result }} />
        {message.status === "applied" && <VariationResultCard actions={card.prepared.actions} />}
      </>
    );
  } else if (message.status === "dismissed") {
    body = <p className="mt-1 text-[12px] text-dim">Dismissed</p>;
  }
  return (
    <li className="max-w-full text-[13px] text-foreground/90">
      {message.text && <p className="whitespace-pre-wrap break-words">{message.text}</p>}
      {body}
    </li>
  );
}

function isAssistant(message: AgentMessage): message is AgentAssistantMessage {
  return message.role === "assistant";
}

interface MessageListProps {
  onRevise(): void;
  onFocusLost(): void;
}

/** The conversation: the user's words, assistant replies with their cards, and one progress row. */
export function MessageList({ onRevise, onFocusLost }: MessageListProps) {
  const messages = useEditorStore((state) => state.agentMessages);
  const conversation = useEditorStore((state) => state.agentConversation);
  const scrollRef = useRef<HTMLDivElement>(null);
  const busy = isConversationBusy(conversation);
  const latestTurnId = "turn" in conversation ? conversation.turn.messageId : null;
  const last = messages[messages.length - 1];
  const stopped = conversation.status === "idle" && conversation.note === "Stopped" && last?.role === "user";

  useEffect(() => {
    const element = scrollRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages, busy]);

  return (
    <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-3 py-1.5">
      {messages.length === 0 ? (
        <p className="px-1 pt-2 text-[12.5px] text-dim">Describe an edit and the agent will make it on the timeline.</p>
      ) : (
        <ol aria-label="Conversation" className="flex flex-col gap-3">
          {messages.map((message) =>
            isAssistant(message) ? (
              <AssistantReply key={message.id} message={message} latestTurnId={latestTurnId} onRevise={onRevise} onFocusLost={onFocusLost} />
            ) : (
              <UserBubble key={message.id} message={message} />
            ),
          )}
        </ol>
      )}
      <div className="pt-3">
        {busy && <ProgressRow label={conversationStatusLabel(conversation)} />}
        {stopped && (
          <p role="status" className="text-[12px] text-dim">
            Stopped
          </p>
        )}
      </div>
    </div>
  );
}
