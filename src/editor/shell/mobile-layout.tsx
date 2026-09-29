import { useEffect, type ReactNode } from "react";
import { cn } from "@/lib/utils";
import { EditorTabPanel } from "../panels/editor-tab-panel";
import { PreviewPanel } from "../preview/preview-panel";
import { MobilePropertySheet } from "../properties/mobile-property-sheets";
import { useEditorStore } from "../store/editor-store-context";
import type { EditorTabId } from "../store/persisted-layout";
import { MobileClipTools } from "../timeline/mobile-clip-tools";
import { TimelinePanel } from "../timeline/timeline-panel";
import { BottomSheet, mobileToolBarOuterHeight } from "./bottom-sheet";
import { editorTabs } from "./editor-tabs";
import { safeAreaBottomClass } from "./use-safe-area";

const tallSheets: ReadonlySet<EditorTabId> = new Set(["ai", "media", "audio"]);

export function MobileLayout({ topBar }: { topBar: ReactNode }) {
  const openSheetId = useEditorStore((state) => state.openSheetId);
  const openSheet = useEditorStore((state) => state.openSheet);
  const closeSheet = useEditorStore((state) => state.closeSheet);
  const hasSelection = useEditorStore((state) => state.selectedItemIds.length > 0 || state.selectedTransitionId !== null);
  const clearSelection = useEditorStore((state) => state.clearSelection);
  const openTab = editorTabs.find((tab) => tab.id === openSheetId);
  const agentRequested = useEditorStore((state) => state.pendingAgentRequest !== null);

  // A handoff to the AI tab (Ask AI, Organize with AI) opens its sheet; the panel consumes the request on mount.
  useEffect(() => {
    if (agentRequested) openSheet("ai");
  }, [agentRequested, openSheet]);

  return (
    <main aria-label="Video editor workspace" className="flex h-full flex-col overflow-hidden bg-background text-foreground">
      {topBar}
      <div className="flex min-h-0 flex-1 flex-col gap-1.5 px-1.5 pb-1.5">
        {/* A definite height lets the preview frame fit 16:9 to the full width, capped so the timeline keeps room. */}
        <div className="h-[min(45%,calc(56.25vw_+_51px))] shrink-0">
          <PreviewPanel />
        </div>
        <div className="min-h-0 flex-1">
          <TimelinePanel mobile />
        </div>
      </div>
      <nav aria-label="Bottom tool bar" style={{ height: mobileToolBarOuterHeight }} className={cn("shrink-0 border-t border-line bg-panel px-1 pt-2", safeAreaBottomClass)}>
        {hasSelection ? (
          <MobileClipTools onBack={clearSelection} />
        ) : (
          <div role="toolbar" aria-label="Editor tools" className="flex h-full">
            {editorTabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => openSheet(tab.id)}
                className="flex flex-1 flex-col items-center gap-1 rounded-control py-1.5 text-[11px] text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <tab.icon className="h-[21px] w-[21px]" aria-hidden />
                {tab.label}
              </button>
            ))}
          </div>
        )}
      </nav>
      {openTab && (
        <BottomSheet title={openTab.label} open height={tallSheets.has(openTab.id) ? "tall" : "compact"} onClose={closeSheet}>
          <EditorTabPanel tab={openTab.id} />
        </BottomSheet>
      )}
      <MobilePropertySheet />
    </main>
  );
}
