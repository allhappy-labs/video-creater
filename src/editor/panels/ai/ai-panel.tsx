import { useEffect, useRef } from "react";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { AiHeader } from "./ai-header";
import { Composer } from "./composer";
import { MessageList } from "./message-list";
import { QuickEdits } from "./quick-edits";
import { SendConfirmation } from "./send-confirmation";

/**
 * The AI tab: chat header, the conversation with its result and review cards, quick edits for an
 * empty conversation, and the composer. It consumes handoffs from other panels (a pending request)
 * when it mounts and whenever a new one arrives: "Ask AI" fills the context chip and focuses the
 * composer; "Organize with AI" drafts its prompt and asks "Send this request?" before sending.
 */
export function AiPanel() {
  const store = useEditorStoreApi();
  const pendingRequest = useEditorStore((state) => state.pendingAgentRequest);
  const empty = useEditorStore((state) => state.agentMessages.length === 0);
  const confirming = useEditorStore((state) => state.agentConfirmSend);
  const composerRef = useRef<HTMLTextAreaElement>(null);

  function focusComposer(caretAtEnd = true) {
    const element = composerRef.current;
    if (!element) return;
    element.focus();
    if (caretAtEnd) element.setSelectionRange(element.value.length, element.value.length);
  }

  // "Ask AI" focuses the composer to type the question; a request to send asks first (the
  // confirmation row takes focus itself).
  useEffect(() => {
    if (!pendingRequest || !store.getState().consumePendingAgentRequest()) return;
    if (pendingRequest.confirmSend && pendingRequest.prompt !== undefined) return;
    // Consuming clears the request (re-running this effect), so the frame isn't cancelled on cleanup.
    requestAnimationFrame(() => {
      const element = composerRef.current;
      if (!element) return;
      element.focus();
      element.setSelectionRange(element.value.length, element.value.length);
    });
  }, [store, pendingRequest]);

  function prefill(text: string) {
    store.getState().setAgentDraft(text);
    // The draft renders first, so the caret lands after the new text.
    requestAnimationFrame(() => focusComposer());
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <AiHeader />
      <MessageList onRevise={() => prefill("Revise: ")} onFocusLost={() => focusComposer()} />
      {empty && !confirming && <QuickEdits onChoose={(edit) => prefill(edit.prompt)} />}
      <SendConfirmation onDone={() => focusComposer()} />
      <Composer ref={composerRef} />
    </div>
  );
}
