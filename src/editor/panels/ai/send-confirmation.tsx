import { useEffect, useId, useRef, type KeyboardEvent } from "react";
import { isConversationBusy } from "@/lib/agent/conversation-state";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { CardButton } from "./card-parts";

interface SendConfirmationProps {
  /** Returns focus to the composer after Send or Cancel removes the row. */
  onDone(): void;
}

/**
 * "Send this request?" above the composer for a handoff that drafts a prompt to send (Organize with
 * AI). The user has to decide, so Send takes focus when the row appears; Escape cancels. Cancel keeps
 * the draft in the composer for editing.
 */
export function SendConfirmation({ onDone }: SendConfirmationProps) {
  const store = useEditorStoreApi();
  const confirming = useEditorStore((state) => state.agentConfirmSend && state.agentDraft.trim().length > 0);
  const busy = useEditorStore((state) => isConversationBusy(state.agentConversation));
  const labelId = useId();
  const sendRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (confirming) sendRef.current?.focus();
  }, [confirming]);

  if (!confirming) return null;

  function send() {
    if (busy) return;
    void store.getState().confirmAgentSend();
    onDone();
  }

  function cancel() {
    store.getState().cancelAgentSend();
    onDone();
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key !== "Escape") return;
    // Handled here, so the editor's Esc doesn't also close the AI sheet.
    event.preventDefault();
    cancel();
  }

  return (
    <div role="group" aria-labelledby={labelId} onKeyDown={onKeyDown} className="mx-2.5 mt-2 flex items-center gap-1.5 rounded-[10px] bg-raised py-1.5 pl-3 pr-1.5">
      <p id={labelId} className="min-w-0 flex-1 text-[12.5px] text-foreground">
        Send this request?
      </p>
      <CardButton variant="ghost" onClick={cancel}>
        Cancel
      </CardButton>
      <CardButton
        ref={sendRef}
        variant="primary"
        aria-disabled={busy || undefined}
        title={busy ? "Wait for the current edit to finish" : undefined}
        onClick={send}
      >
        Send
      </CardButton>
    </div>
  );
}
