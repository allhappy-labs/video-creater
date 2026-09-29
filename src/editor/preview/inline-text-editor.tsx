import { useCallback, useEffect, useMemo, useRef, type KeyboardEvent } from "react";
import { captionTextAction } from "@/lib/properties/caption-properties";
import { textOverlayUpdateAction } from "@/lib/properties/text-properties";
import { findTimelineItem } from "@/lib/preview/canvas-geometry";
import { getTimelineItemText, type Timeline } from "@/lib/timeline";
import { usePropertyCommit } from "../properties/use-property-commit";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { inlineTextEditableItemIds } from "./canvas-editing";

/** How caption and text overlay layers select, start and finish inline editing. */
export interface OverlayTextInteraction {
  readonly editingItemId: string | null;
  readonly editableItemIds: ReadonlySet<string>;
  onSelect(itemId: string): void;
  onStartEdit(itemId: string): void;
  onCommit(itemId: string, text: string): void;
  onCancel(): void;
}

/**
 * Inline text editing for the timeline preview. A commit is one undo step: captions use the caption
 * text edit (a transcript repair only when the cue is exactly one transcript word); plain text
 * overlays use the legacy overlay update, which fills missing guidance metadata with the defaults.
 * Text is trimmed; blank or unchanged text only closes the editor.
 */
export function useInlineTextEditing(timeline: Timeline, frameOverlayItemIds: readonly string[]): OverlayTextInteraction {
  const store = useEditorStoreApi();
  const { commit } = usePropertyCommit();
  const editingItemId = useEditorStore((state) => state.editingTextItemId);
  const editableItemIds = useMemo(() => inlineTextEditableItemIds(timeline), [timeline]);
  const editingOnCanvas = editingItemId !== null && frameOverlayItemIds.includes(editingItemId);

  // Editing ends, without a commit, when the layer leaves the frame (the playhead moved past it).
  useEffect(() => {
    if (editingItemId !== null && !editingOnCanvas) store.getState().setEditingTextItemId(null);
  }, [editingItemId, editingOnCanvas, store]);

  const onSelect = useCallback((itemId: string) => store.getState().selectItems([itemId]), [store]);
  const onStartEdit = useCallback(
    (itemId: string) => {
      const state = store.getState();
      state.selectItems([itemId]);
      state.setEditingTextItemId(itemId);
    },
    [store],
  );
  const onCancel = useCallback(() => store.getState().setEditingTextItemId(null), [store]);
  const onCommit = useCallback(
    (itemId: string, text: string) => {
      const state = store.getState();
      state.setEditingTextItemId(null);
      const item = findTimelineItem(state.project.timeline, itemId);
      const trimmed = text.trim();
      if (!item || trimmed.length === 0 || trimmed === getTimelineItemText(item)) return;
      void commit(item.kind === "caption" ? captionTextAction(item, trimmed, state.project) : textOverlayUpdateAction(item, { text: trimmed }));
    },
    [commit, store],
  );

  return useMemo(
    () => ({ editingItemId, editableItemIds, onSelect, onStartEdit, onCommit, onCancel }),
    [editableItemIds, editingItemId, onCancel, onCommit, onSelect, onStartEdit],
  );
}

/**
 * A `contentEditable` span in place of a layer's text, kept single-line: Enter or blur commits,
 * Escape cancels. Keys stay inside the editor so preview and editor shortcuts do not run.
 */
export function InlineTextEditor({
  label,
  text,
  onCommit,
  onCancel,
}: {
  readonly label: string;
  readonly text: string;
  readonly onCommit: (text: string) => void;
  readonly onCancel: () => void;
}) {
  const ref = useRef<HTMLSpanElement>(null);
  const finished = useRef(false);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    element.focus();
    const selection = window.getSelection();
    if (selection) {
      const range = document.createRange();
      range.selectNodeContents(element);
      selection.removeAllRanges();
      selection.addRange(range);
    }
  }, []);

  function finish(committed: boolean) {
    if (finished.current) return;
    finished.current = true;
    if (committed) onCommit(ref.current?.textContent ?? "");
    else onCancel();
  }

  function onKeyDown(event: KeyboardEvent<HTMLSpanElement>) {
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      finish(true);
    } else if (event.key === "Escape") {
      event.preventDefault();
      finish(false);
    }
  }

  return (
    <span
      ref={ref}
      role="textbox"
      aria-label={label}
      contentEditable
      suppressContentEditableWarning
      spellCheck={false}
      className="pointer-events-auto cursor-text rounded-sm outline-none ring-2 ring-accent"
      onKeyDown={onKeyDown}
      onBlur={() => finish(true)}
      onPointerDown={(event) => event.stopPropagation()}
      onDoubleClick={(event) => event.stopPropagation()}
    >
      {text}
    </span>
  );
}
