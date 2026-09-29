import type { MediaFilter } from "@/lib/media/media-filters";
import type { GenerationPlacementIntent, JobExportSettings } from "@/lib/project";
import type { SelectionKind } from "@/lib/preview/selection-kind";
import type { EditorSliceCreator } from "./editor-store";
import { clamp, leftWidthBounds, loadEditorLayout, saveEditorLayout, type EditorTabId } from "./persisted-layout";

/** Clip tools that open a Properties sheet on mobile. */
export type PropertyToolId =
  | "speed"
  | "volume"
  | "animation"
  | "effects"
  | "adjust"
  | "text"
  | "style"
  | "content"
  | "transitionType"
  | "transitionDuration"
  | "ai";

/** A left tab sheet, or a clip tool's Properties sheet. */
type SheetId = EditorTabId | `property:${PropertyToolId}`;

/** An Audio tab cleanup card that Properties routes to; the card focuses itself and clears the request. */
type CleanupFocusTarget = "removeSilences";

/** A request for the AI tab (about timeline clips, or a drafted prompt); the AI tab consumes and clears it. */
export interface PendingAgentRequest {
  readonly itemIds: readonly string[];
  /** A prompt to draft in the composer, e.g. "Organize with AI" from the Media tab. */
  readonly prompt?: string;
  /** Asks "Send this request?" in the AI tab and sends the drafted prompt once confirmed. */
  readonly confirmSend?: boolean;
}

/** The Generate sub-view that replaces the Media or Audio grid while open. */
interface GenerateView {
  readonly open: boolean;
  readonly mode: "image" | "video" | "audio";
  /** Where the output goes; "Replace with generated…" sets `replace:<itemId>`. Absent means the library. */
  readonly placementIntent?: GenerationPlacementIntent;
}

/**
 * A transient property value shown while a Properties control is dragged. The preview applies
 * `patch` over the item's properties without mutating the project, so it never adds undo steps.
 */
interface PropertyPreview {
  readonly itemId: string;
  readonly patch: Readonly<Record<string, unknown>>;
}

/** Export popover options preset from an earlier job (Retry without a stored export plan). */
export interface ExportPreset {
  readonly jobId: string;
  readonly profile: string | null;
  readonly quality: string | null;
  readonly nleFormat: string | null;
  /** The job's recorded export settings; they restore every popover choice. */
  readonly settings: JobExportSettings | null;
}

interface ExportPopoverState {
  readonly preset: ExportPreset | null;
}

/** An optional toast button, e.g. "Show in folder". */
interface ToastAction {
  readonly label: string;
  onSelect(): void;
}

/** A completion or undoable background outcome; errors belong at their control, not in toasts. */
interface EditorToast {
  readonly id: string;
  readonly title: string;
  readonly description?: string;
  readonly action?: ToastAction;
}

/** A gear-menu overlay; one at a time. The native menu opens these through `openOverlay` too. */
export type EditorOverlay = "shortcuts" | "connectAgents" | "projectSkills";

/** Toasts beyond this many drop the oldest. */
const maxToasts = 3;
let toastSequence = 0;

export interface UiSlice {
  readonly activeTab: EditorTabId;
  readonly leftWidth: number;
  readonly openSheetId: SheetId | null;
  /** Clip the Media tab replaces when the user picks media ("Replace with media…"). */
  readonly replaceTargetItemId: string | null;
  /** Media asset the Media tab scrolls to and highlights ("Reveal in Media"). */
  readonly revealMediaId: string | null;
  readonly pendingAgentRequest: PendingAgentRequest | null;
  /** Attach mode from the AI composer: the Media tab's next tile pick becomes a composer mention. */
  readonly mediaAttachMode: boolean;
  readonly pendingCleanupFocus: CleanupFocusTarget | null;
  /** Media tab browsing state, kept while the tab or its mobile sheet is closed. */
  readonly mediaFilter: MediaFilter;
  readonly mediaFolderId: string | null;
  readonly mediaSearch: string;
  readonly generateView: GenerateView | null;
  readonly propertyPreview: PropertyPreview | null;
  /** Clip whose crop the canvas edits ("Edit on canvas"); the preview canvas consumes it. */
  readonly cropModeItemId: string | null;
  /** The Properties tab last chosen for each selection kind. */
  readonly propertiesTabByKind: Readonly<Partial<Record<SelectionKind, string>>>;
  /** The open Export popover; null when closed. */
  readonly exportPopover: ExportPopoverState | null;
  /** The background task whose details dialog is open (set by `openTaskDetails`). */
  readonly taskDetailsId: string | null;
  /** Visible toasts, oldest first. */
  readonly toasts: readonly EditorToast[];
  /** The open gear-menu overlay; null when none is open. */
  readonly overlay: EditorOverlay | null;
  setActiveTab(tab: EditorTabId): void;
  setLeftWidth(width: number): void;
  openSheet(sheet: SheetId): void;
  closeSheet(): void;
  setReplaceTargetItemId(itemId: string | null): void;
  setRevealMediaId(mediaId: string | null): void;
  setPendingAgentRequest(request: PendingAgentRequest | null): void;
  setMediaAttachMode(on: boolean): void;
  setPendingCleanupFocus(target: CleanupFocusTarget | null): void;
  setMediaFilter(filter: MediaFilter): void;
  setMediaFolderId(folderId: string | null): void;
  setMediaSearch(query: string): void;
  setGenerateView(view: GenerateView | null): void;
  setPropertyPreview(preview: PropertyPreview): void;
  clearPropertyPreview(): void;
  setCropModeItemId(itemId: string | null): void;
  setPropertiesTab(kind: SelectionKind, tab: string): void;
  openExportPopover(preset?: ExportPreset | null): void;
  closeExportPopover(): void;
  closeTaskDetails(): void;
  /** Shows a toast and returns its id. */
  pushToast(toast: Omit<EditorToast, "id">): string;
  dismissToast(id: string): void;
  openOverlay(overlay: EditorOverlay): void;
  closeOverlay(): void;
}

export const createUiSlice: EditorSliceCreator<UiSlice> = (set, get) => {
  const layout = loadEditorLayout();
  const persist = () => saveEditorLayout({ activeTab: get().activeTab, leftWidth: get().leftWidth });
  return {
    activeTab: layout.activeTab,
    leftWidth: layout.leftWidth,
    openSheetId: null,
    replaceTargetItemId: null,
    revealMediaId: null,
    pendingAgentRequest: null,
    mediaAttachMode: false,
    pendingCleanupFocus: null,
    mediaFilter: "all",
    mediaFolderId: null,
    mediaSearch: "",
    generateView: null,
    propertyPreview: null,
    cropModeItemId: null,
    propertiesTabByKind: {},
    exportPopover: null,
    taskDetailsId: null,
    toasts: [],
    overlay: null,
    setActiveTab: (tab) => {
      set({ activeTab: tab });
      persist();
    },
    setLeftWidth: (width) => {
      set({ leftWidth: clamp(Math.round(width), leftWidthBounds.min, leftWidthBounds.max) });
      persist();
    },
    openSheet: (sheet) => set({ openSheetId: sheet }),
    closeSheet: () => set({ openSheetId: null }),
    setReplaceTargetItemId: (itemId) => set({ replaceTargetItemId: itemId }),
    setRevealMediaId: (mediaId) => set({ revealMediaId: mediaId }),
    setPendingAgentRequest: (request) =>
      set({ pendingAgentRequest: request ? { ...request, itemIds: [...new Set(request.itemIds)] } : null }),
    setMediaAttachMode: (on) => set({ mediaAttachMode: on }),
    setPendingCleanupFocus: (target) => set({ pendingCleanupFocus: target }),
    setMediaFilter: (filter) => set({ mediaFilter: filter }),
    setMediaFolderId: (folderId) => set({ mediaFolderId: folderId }),
    setMediaSearch: (query) => set({ mediaSearch: query }),
    setGenerateView: (view) => set({ generateView: view }),
    setPropertyPreview: (preview) => set({ propertyPreview: { itemId: preview.itemId, patch: { ...preview.patch } } }),
    clearPropertyPreview: () => {
      if (get().propertyPreview !== null) set({ propertyPreview: null });
    },
    setCropModeItemId: (itemId) => set({ cropModeItemId: itemId }),
    setPropertiesTab: (kind, tab) =>
      set((state) => ({ propertiesTabByKind: { ...state.propertiesTabByKind, [kind]: tab } })),
    openExportPopover: (preset = null) => set({ exportPopover: { preset } }),
    closeExportPopover: () => set({ exportPopover: null }),
    closeTaskDetails: () => set({ taskDetailsId: null }),
    pushToast: (toast) => {
      toastSequence += 1;
      const id = `toast-${toastSequence}`;
      set((state) => ({ toasts: [...state.toasts, { ...toast, id }].slice(-maxToasts) }));
      return id;
    },
    dismissToast: (id) => set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),
    openOverlay: (overlay) => set({ overlay }),
    closeOverlay: () => set({ overlay: null }),
  };
};
