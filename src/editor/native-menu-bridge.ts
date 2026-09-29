import { useEffect, useRef } from "react";
import type { NativeMenuCommand, NativeMenuRequest, NativeMenuState } from "@/lib/native-menu";
import { defaultSplitItemId, deleteGapAt, deleteItems, rippleDeleteItems, splitAtPlayhead, type CommandResult } from "@/lib/timeline-ops/clip-commands";
import { isEditableKeyboardTarget } from "@/lib/timeline-ops/navigation";
import { planRemoveTransition } from "@/lib/timeline-ops/transition-commands";
import { trimItemsToPlayhead } from "@/lib/timeline-ops/trim-to-playhead";
import { useMediaService, type MediaService } from "./services/media-service";
import type { EditorState, EditorStore } from "./store/editor-store";
import { useEditorStoreApi } from "./store/editor-store-context";
import type { EditorTabId } from "./store/persisted-layout";
import { frameSnappedSeconds, useTimelineCommands, type TimelineCommands } from "./timeline/timeline-commands";
import { deleteGapOrSelection, selectForward } from "./timeline/use-timeline-shortcuts";

const runnable = (result: CommandResult) => !("blocked" in result);

type TimelineMenuState = Pick<NativeMenuState, "canSplit" | "canTrimStart" | "canTrimEnd" | "canDelete" | "canRippleDelete" | "canSelectForward">;

/** The timeline items follow the same planners as the shortcuts they mirror: enabled when not blocked. */
function timelineMenuState({ project, selectedItemIds: ids, selectedGap, selectedTransitionId, playheadSeconds }: EditorState): TimelineMenuState {
  const splitSeconds = frameSnappedSeconds(project, playheadSeconds);
  // Delete and ripple delete remove a selected transition, else close a selected gap (see `deleteGapOrSelection`).
  const gapDeletable =
    selectedTransitionId !== null
      ? runnable(planRemoveTransition(project, selectedTransitionId))
      : selectedGap
        ? runnable(deleteGapAt(project, selectedGap.trackId, (selectedGap.startSeconds + selectedGap.endSeconds) / 2))
        : null;
  const leadId = ids[0];
  return {
    canSplit: runnable(splitAtPlayhead(project, ids, splitSeconds, (itemId) => defaultSplitItemId(itemId, splitSeconds))),
    canTrimStart: runnable(trimItemsToPlayhead(project, ids, playheadSeconds, "start")),
    canTrimEnd: runnable(trimItemsToPlayhead(project, ids, playheadSeconds, "end")),
    canDelete: gapDeletable ?? runnable(deleteItems(project, ids)),
    canRippleDelete: gapDeletable ?? runnable(rippleDeleteItems(project, ids)),
    canSelectForward: project.timeline.tracks.some((track) => track.items.some((item) => item.id === leadId)),
  };
}

/**
 * The editor's native menu capabilities. While a text field has focus, Undo and Redo stay enabled:
 * the editor's menu items replace macOS's predefined ones, so they carry the field's own undo too.
 */
export function editorNativeMenuState(
  state: EditorState,
  textEditing: boolean,
  timeline: TimelineMenuState = timelineMenuState(state),
): NativeMenuState {
  const projectLoaded = state.projectDir.trim().length > 0;
  return {
    view: "editor",
    canImport: projectLoaded,
    canExport: projectLoaded,
    canUndo: textEditing || state.canUndo(),
    canRedo: textEditing || state.canRedo(),
    ...timeline,
  };
}

export interface NativeEditorCommandContext {
  readonly store: EditorStore;
  readonly timeline: TimelineCommands;
  readonly media: MediaService;
  /** Phone layout: left tabs open as bottom sheets. */
  readonly mobile: boolean;
  /** The focused element when the command arrived. */
  readonly focused: Element | null;
}

const tabByCommand: Readonly<Partial<Record<NativeMenuCommand, EditorTabId>>> = {
  "showTab:ai": "ai",
  "showTab:media": "media",
  "showTab:audio": "audio",
  "showTab:text": "text",
  "showTab:captions": "captions",
  "showTab:effects": "effects",
};

function showTab({ store, mobile }: NativeEditorCommandContext, tab: EditorTabId) {
  store.getState().setActiveTab(tab);
  if (mobile) store.getState().openSheet(tab);
}

/**
 * Undo and Redo from the menu. Menu items only receive a key when the web view did not handle it:
 * macOS sends ⌘Z to the menu when the editor's keydown handler skipped it (a text field has focus),
 * and GTK menus register no Ctrl+Z accelerator. A focused field gets its own undo; otherwise the
 * editor's global undo runs (agent batches included).
 */
function runHistory(context: NativeEditorCommandContext, direction: "undo" | "redo") {
  if (isEditableKeyboardTarget(context.focused)) {
    // The field's native undo stack has no other script entry point.
    document.execCommand(direction);
    return;
  }
  const state = context.store.getState();
  void (direction === "undo" ? state.undo() : state.redo());
}

async function importMedia(context: NativeEditorCommandContext) {
  showTab(context, "media");
  // Failures land in `lastError`, which the editor shows at the timeline.
  await context.media.importMediaFiles();
}

/** Runs one native menu command in the editor. App-level commands are handled by `App` and ignored here. */
export function runNativeEditorCommand(command: NativeMenuCommand, context: NativeEditorCommandContext): void {
  const { store, timeline } = context;
  const tab = tabByCommand[command];
  if (tab) {
    showTab(context, tab);
    return;
  }
  switch (command) {
    case "undo":
    case "redo":
      runHistory(context, command);
      return;
    case "importMedia":
      void importMedia(context);
      return;
    case "exportProject":
      store.getState().openExportPopover();
      return;
    case "openShortcuts":
      store.getState().openOverlay("shortcuts");
      return;
    case "openConnectAgents":
      store.getState().openOverlay("connectAgents");
      return;
    case "openProjectGuidance":
      store.getState().openOverlay("projectSkills");
      return;
    case "split":
      void timeline.splitAtPlayhead();
      return;
    case "trimStart":
    case "trimEnd":
      void timeline.trimToPlayhead(command === "trimStart" ? "start" : "end");
      return;
    case "delete":
    case "rippleDelete":
      void deleteGapOrSelection(store, timeline, command === "rippleDelete");
      return;
    case "selectForwardTrack":
    case "selectForwardAll":
      selectForward(store, command === "selectForwardAll");
      return;
    default:
      return;
  }
}

function sameMenuState(a: NativeMenuState | null, b: NativeMenuState): boolean {
  return a !== null && (Object.keys(b) as (keyof NativeMenuState)[]).every((key) => a[key] === b[key]);
}

interface NativeMenuBridgeOptions {
  readonly isActive: boolean;
  readonly mobile: boolean;
  readonly request: NativeMenuRequest | null;
  readonly onStateChange: (state: NativeMenuState) => void;
}

/** Reports the editor's menu capabilities while it is the active view and runs the commands App forwards. */
export function useNativeMenuBridge({ isActive, mobile, request, onStateChange }: NativeMenuBridgeOptions): void {
  const store = useEditorStoreApi();
  const timeline = useTimelineCommands();
  const media = useMediaService();
  // A request present at mount was sent to an earlier editor session.
  const handledSequence = useRef(request?.sequence ?? 0);

  useEffect(() => {
    if (!isActive) return;
    let reported: NativeMenuState | null = null;
    let textEditing = isEditableKeyboardTarget(document.activeElement);
    let timelineInputs: readonly unknown[] = [];
    let timelineState: TimelineMenuState | null = null;
    let focusTimer: number | undefined;
    const report = () => {
      const state = store.getState();
      // Playback moves the playhead every frame; only re-plan when a planner input changed.
      const inputs = [state.project, state.selectedItemIds, state.selectedGap, state.selectedTransitionId, state.playheadSeconds];
      if (!timelineState || inputs.some((input, index) => input !== timelineInputs[index])) {
        timelineInputs = inputs;
        timelineState = timelineMenuState(state);
      }
      const next = editorNativeMenuState(state, textEditing, timelineState);
      if (sameMenuState(reported, next)) return;
      reported = next;
      onStateChange(next);
    };
    const onFocusChange = () => {
      // Focus moves after `focusout`; read the settled element on the next task.
      window.clearTimeout(focusTimer);
      focusTimer = window.setTimeout(() => {
        textEditing = isEditableKeyboardTarget(document.activeElement);
        report();
      }, 0);
    };
    report();
    const unsubscribe = store.subscribe(report);
    document.addEventListener("focusin", onFocusChange);
    document.addEventListener("focusout", onFocusChange);
    return () => {
      window.clearTimeout(focusTimer);
      unsubscribe();
      document.removeEventListener("focusin", onFocusChange);
      document.removeEventListener("focusout", onFocusChange);
    };
  }, [isActive, onStateChange, store]);

  useEffect(() => {
    if (!isActive || !request || request.sequence === handledSequence.current) return;
    handledSequence.current = request.sequence;
    runNativeEditorCommand(request.command, { store, timeline, media, mobile, focused: document.activeElement });
  }, [isActive, media, mobile, request, store, timeline]);
}
