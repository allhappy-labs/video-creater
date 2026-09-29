import { ArrowUp, AtSign, Paperclip, Square, X } from "lucide-react";
import { forwardRef, useId, useMemo, useRef, useState, type KeyboardEvent, type MutableRefObject } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { Textarea } from "@/components/ui/textarea";
import { isConversationBusy } from "@/lib/agent/conversation-state";
import { conversationMentionTargets, insertMention, type MentionTarget } from "@/lib/agent/mentions";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { MentionPicker, useMentionPicker } from "./mention-picker";

/**
 * The AI composer: the draft (empty by default), the @ mention picker, the selection context chip,
 * attach, and Send, which becomes Stop while a turn runs. Enter sends; Shift+Enter adds a line.
 */
export const Composer = forwardRef<HTMLTextAreaElement>(function Composer(_props, forwardedRef) {
  const store = useEditorStoreApi();
  const draft = useEditorStore((state) => state.agentDraft);
  const chip = useEditorStore((state) => state.agentContextChip);
  const error = useEditorStore((state) => state.agentComposerError);
  const conversation = useEditorStore((state) => state.agentConversation);
  const project = useEditorStore((state) => state.project);
  const errorId = useId();
  const localRef = useRef<HTMLTextAreaElement | null>(null);
  const [caret, setCaret] = useState(0);
  const [focused, setFocused] = useState(false);
  const targets = useMemo(() => conversationMentionTargets(project), [project]);
  const busy = isConversationBusy(conversation);
  // An atomic apply can't be stopped midway; Stop only interrupts the agent's turn.
  const stoppable = conversation.status === "validating" || (conversation.status === "working" && conversation.phase === "reviewing");

  function setRef(element: HTMLTextAreaElement | null) {
    localRef.current = element;
    if (typeof forwardedRef === "function") forwardedRef(element);
    else if (forwardedRef) (forwardedRef as MutableRefObject<HTMLTextAreaElement | null>).current = element;
  }

  function placeCaret(next: number) {
    setCaret(next);
    requestAnimationFrame(() => {
      const element = localRef.current;
      if (!element) return;
      element.focus();
      element.setSelectionRange(next, next);
    });
  }

  const picker = useMentionPicker({
    draft,
    caret,
    focused,
    targets,
    onPick(target: MentionTarget) {
      const inserted = insertMention(draft, caret, target);
      store.getState().setAgentDraft(inserted.draft);
      placeCaret(inserted.caret);
    },
  });

  function send() {
    if (busy || !draft.trim()) return;
    void store.getState().submitAgentPrompt(draft);
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (picker.onKeyDown(event)) {
      event.preventDefault();
      event.stopPropagation();
      return;
    }
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      send();
    }
  }

  function mention() {
    const element = localRef.current;
    const at = element ? element.selectionStart : draft.length;
    const before = draft.slice(0, at);
    const prefix = before.length === 0 || /\s$/.test(before) ? "@" : " @";
    store.getState().setAgentDraft(`${before}${prefix}${draft.slice(at)}`);
    placeCaret(at + prefix.length);
  }

  return (
    // The borderless field drops its own ring, so the composer box carries the focus ring.
    <div className="relative mx-2.5 mb-2.5 mt-2 rounded-xl bg-raised px-2.5 pb-2 pt-2.5 focus-within:ring-2 focus-within:ring-ring">
      <MentionPicker controller={picker} />
      <Textarea
        ref={setRef}
        value={draft}
        rows={2}
        aria-label="Describe an edit"
        placeholder="Describe an edit…"
        aria-describedby={error ? errorId : undefined}
        aria-controls={picker.open ? picker.listboxId : undefined}
        aria-activedescendant={picker.activeOptionId}
        onChange={(event) => {
          store.getState().setAgentDraft(event.target.value);
          setCaret(event.target.selectionStart);
        }}
        onSelect={(event) => setCaret(event.currentTarget.selectionStart)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        onKeyDown={onKeyDown}
        className="min-h-[38px] resize-none border-0 bg-transparent px-0.5 py-0 text-[13px] shadow-none placeholder:text-dim focus-visible:ring-0"
      />
      {error && (
        <p id={errorId} role="alert" className="px-0.5 pb-1 text-[12px] text-destructive">
          {error}
        </p>
      )}
      <div className="flex items-center gap-1 pt-1">
        {chip && (
          <span className="flex min-w-0 items-center gap-1 rounded-xl bg-panel py-0.5 pl-2 pr-0.5 text-[11.5px] text-muted-foreground">
            <span className="min-w-0 truncate" title={chip.label}>
              {chip.label}
            </span>
            <button
              type="button"
              aria-label={`Remove ${chip.label} from the request`}
              onClick={() => store.getState().removeAgentContextChip()}
              className="grid h-5 w-5 shrink-0 place-items-center rounded-full hover:bg-hover hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <X className="h-3 w-3" aria-hidden />
            </button>
          </span>
        )}
        <IconButton label="Mention media or a clip" size="sm" onClick={mention}>
          <AtSign className="h-4 w-4" aria-hidden />
        </IconButton>
        <IconButton label="Attach media" size="sm" onClick={() => store.getState().startAgentMediaAttach()}>
          <Paperclip className="h-4 w-4" aria-hidden />
        </IconButton>
        <div className="flex-1" />
        {busy ? (
          <IconButton
            label="Stop"
            disabled={!stoppable}
            onClick={() => void store.getState().cancelAgentTurn()}
            className="h-[30px] w-[30px] rounded-full bg-foreground text-background hover:bg-foreground/90 hover:text-background"
          >
            <Square className="h-3 w-3 fill-current" aria-hidden />
          </IconButton>
        ) : (
          <IconButton
            label="Send"
            disabled={!draft.trim()}
            onClick={send}
            className="h-[30px] w-[30px] rounded-full bg-primary text-primary-foreground hover:bg-primary/90 hover:text-primary-foreground"
          >
            <ArrowUp className="h-4 w-4" aria-hidden />
          </IconButton>
        )}
      </div>
    </div>
  );
});
