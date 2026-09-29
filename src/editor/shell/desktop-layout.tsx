import type { ReactNode } from "react";
import { PreviewPanel } from "../preview/preview-panel";
import { useEditorStore } from "../store/editor-store-context";
import { TimelinePanel } from "../timeline/timeline-panel";
import { leftWidthBounds, splitHeightBounds } from "../store/persisted-layout";
import { LeftPanel } from "./left-panel";
import { PropertiesRegion, propertiesPanelWidth } from "./properties-region";
import { SplitResizer } from "./split-resizer";

const minimumPreviewHeight = 240;
const topBarHeight = 48;

export function DesktopLayout({ mode, topBar }: { mode: "desktop-docked" | "desktop-overlay"; topBar: ReactNode }) {
  const leftWidth = useEditorStore((state) => state.leftWidth);
  const setLeftWidth = useEditorStore((state) => state.setLeftWidth);
  const splitHeight = useEditorStore((state) => state.splitHeight);
  const setSplitHeight = useEditorStore((state) => state.setSplitHeight);
  const hasSelection = useEditorStore((state) => state.selectedItemIds.length > 0 || state.selectedTransitionId !== null);
  const docked = mode === "desktop-docked";
  const maxSplit = Math.max(splitHeightBounds.min, window.innerHeight - topBarHeight - minimumPreviewHeight);
  const timelineHeight = Math.min(splitHeight, maxSplit);

  return (
    <main aria-label="Video editor workspace" className="flex h-full flex-col overflow-hidden bg-background text-foreground">
      {topBar}
      <div className="flex min-h-0 flex-1 px-1.5">
        <div style={{ width: leftWidth }} className="min-h-0 shrink-0">
          <LeftPanel />
        </div>
        <SplitResizer
          orientation="vertical"
          label="Resize left panel"
          value={leftWidth}
          min={leftWidthBounds.min}
          max={leftWidthBounds.max}
          onResize={setLeftWidth}
        />
        <div className="relative flex min-h-0 min-w-0 flex-1 gap-1.5">
          <div className="min-h-0 min-w-0 flex-1">
            <PreviewPanel insetRight={hasSelection && !docked ? propertiesPanelWidth : 0} />
          </div>
          {hasSelection && <PropertiesRegion overlay={!docked} />}
        </div>
      </div>
      <SplitResizer
        orientation="horizontal"
        invert
        label="Resize timeline"
        value={timelineHeight}
        min={splitHeightBounds.min}
        max={maxSplit}
        onResize={setSplitHeight}
      />
      <div style={{ height: timelineHeight }} className="shrink-0 px-1.5 pb-1.5">
        <TimelinePanel />
      </div>
    </main>
  );
}
