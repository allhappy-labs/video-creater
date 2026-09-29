import { X } from "lucide-react";
import { Fragment, useId, useLayoutEffect, useRef, type UIEvent } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { useShortcutPlatform } from "../shell/use-shortcut-platform";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { KeyframeLane } from "./keyframe-lane";
import { MainTrackAddButton } from "./main-track-add-button";
import { OverviewBar } from "./overview-bar";
import { Playhead } from "./playhead";
import { TimelineContextMenu } from "./timeline-context-menu";
import { TimelineInteractionLayer } from "./timeline-interaction-layer";
import { TimelineRuler } from "./timeline-ruler";
import { TimelineToolbar } from "./timeline-toolbar";
import { TrackHeader } from "./track-header";
import { TrackLane, type LaneInteractions } from "./track-lane";
import { useClipDrag } from "./use-clip-drag";
import { useClipTrim, type LivePlacement } from "./use-clip-trim";
import { useMarquee } from "./use-marquee";
import { useTimelineDrop } from "./use-timeline-drop";
import { timelineRulerHeight, useTimelineGeometry } from "./use-timeline-geometry";
import { useTimelineShortcuts } from "./use-timeline-shortcuts";
import { useTouchTimeline } from "./use-touch-timeline";

/** Scroll echoes within this distance of the stored value are ignored. */
const scrollEpsilon = 0.5;
const noItemIds: ReadonlySet<string> = new Set();
const noPlacements: ReadonlyMap<string, LivePlacement> = new Map();

/** One-line, dismissible feedback for blocked or failed timeline edits (`project.lastError`). */
function TimelineFeedback() {
  const lastError = useEditorStore((state) => state.lastError);
  const setLastError = useEditorStore((state) => state.setLastError);
  return (
    <div className={lastError ? "flex h-7 shrink-0 items-center gap-2 border-t border-line pl-3 pr-1.5" : "h-0 shrink-0 overflow-hidden"}>
      <p role="status" aria-live="polite" className="min-w-0 flex-1 truncate text-[12px] text-destructive" title={lastError ?? undefined}>
        {lastError}
      </p>
      {lastError && (
        <IconButton size="sm" label="Dismiss message" onClick={() => setLastError(null)}>
          <X className="h-3.5 w-3.5" aria-hidden />
        </IconButton>
      )}
    </div>
  );
}

/**
 * The timeline: toolbar, then the scrollable ruler and tracks, then the overview bar. Mobile
 * drops the headers, toolbar and overview (the clip tools bar covers editing), pads the lanes
 * by half the viewport and fixes the playhead at the centre.
 */
export function TimelinePanel({ mobile = false }: { readonly mobile?: boolean }) {
  const store = useEditorStoreApi();
  const setScrollLeft = useEditorStore((state) => state.setScrollLeft);
  const scrollerRef = useRef<HTMLDivElement>(null);
  const scrollerId = useId();
  const geometry = useTimelineGeometry(scrollerRef, mobile);
  const { headerWidth, contentWidth, scrollLeft, centerPadding } = geometry;
  const rowsRef = useRef<HTMLDivElement>(null);
  const { drag, onClipPointerDown } = useClipDrag({ geometry });
  const { trim, onTrimPointerDown, onTrimKeyDown } = useClipTrim({ geometry });
  const { onRowsPointerDown } = useMarquee({ geometry, rowsRef });
  const { dropIndicator, transitionDropCut, dropHandlers } = useTimelineDrop({ geometry, rowsRef });
  const onCanvasKeyDown = useTimelineShortcuts(useShortcutPlatform());
  const touch = useTouchTimeline({ enabled: mobile, geometry, scrollerRef });
  const interactions: LaneInteractions = {
    // An Alt-drag duplicate leaves the originals in place, so only a move dims them.
    draggingItemIds: drag && !drag.duplicate ? drag.itemIds : noItemIds,
    livePlacements: trim?.placements ?? noPlacements,
    onClipPointerDown,
    onTrimPointerDown,
    onTrimKeyDown,
  };
  const dropTargetTrackId = drag ? drag.preview.targetTrackId : (dropIndicator?.placement.hoveredTrackId ?? null);
  const dropTargetState = (drag ? drag.preview.state === "rejected" : dropIndicator?.rejected) ? "rejected" : "accepted";

  useLayoutEffect(() => {
    const element = scrollerRef.current;
    if (element && Math.abs(element.scrollLeft - scrollLeft) > scrollEpsilon) element.scrollLeft = scrollLeft;
  }, [scrollLeft]);

  function onScroll(event: UIEvent<HTMLDivElement>) {
    const next = event.currentTarget.scrollLeft;
    // Scrolls that the stored offset already matches are echoes of programmatic scrolling.
    const manual = Math.abs(next - store.getState().scrollLeft) > scrollEpsilon;
    if (manual) setScrollLeft(next);
    touch.onScroll(next, manual);
  }

  // Any timeline pointer interaction returns the preview from asset preview to the timeline.
  function onPointerDownCapture() {
    const state = store.getState();
    if (state.previewSource.kind === "asset") state.previewTimeline();
  }

  return (
    <section aria-label="Timeline" onPointerDownCapture={onPointerDownCapture} className="flex h-full min-h-0 flex-col overflow-hidden rounded-panel bg-panel">
      {!mobile && <TimelineToolbar viewportWidth={geometry.viewportWidth} />}
      {/* Focusable so clicks on empty lanes keep timeline shortcuts active. */}
      <div role="region" aria-label="Timeline canvas" tabIndex={-1} onKeyDown={onCanvasKeyDown} className="flex min-h-0 flex-1 flex-col outline-none">
        <div className="relative min-h-0 flex-1">
          <div
            id={scrollerId}
            ref={scrollerRef}
            data-testid="timeline-scroller"
            onScroll={onScroll}
            {...touch.touchHandlers}
            className={
              mobile
                ? "absolute inset-0 touch-pan-x touch-pan-y select-none overflow-auto overscroll-contain [-webkit-touch-callout:none] [scrollbar-width:none]"
                : "absolute inset-0 overflow-auto overscroll-x-contain [scrollbar-width:thin]"
            }
          >
            <div
              className="relative min-h-full"
              style={{ width: headerWidth + contentWidth + centerPadding * 2, paddingLeft: centerPadding, paddingRight: centerPadding }}
            >
              <div className="sticky top-0 z-30 flex bg-panel" style={{ marginLeft: -centerPadding, marginRight: -centerPadding, paddingLeft: centerPadding, paddingRight: centerPadding }}>
                {headerWidth > 0 && (
                  <div
                    className="sticky left-0 z-10 shrink-0 border-b border-r border-line bg-panel"
                    style={{ width: headerWidth, height: timelineRulerHeight }}
                  />
                )}
                <TimelineRuler geometry={geometry} />
              </div>
              <TimelineContextMenu pixelsPerSecond={geometry.pixelsPerSecond}>
                <div ref={rowsRef} className="relative" onPointerDown={onRowsPointerDown} {...dropHandlers}>
                  {geometry.rows.map((row) => (
                    <Fragment key={row.track.id}>
                      <div className="flex">
                        {headerWidth > 0 && (
                          <div className="sticky left-0 z-20 shrink-0 border-r border-line bg-panel" style={{ width: headerWidth }}>
                            <TrackHeader track={row.track} name={row.name} height={row.height} />
                          </div>
                        )}
                        <TrackLane
                          row={row}
                          geometry={geometry}
                          touch={mobile}
                          interactions={interactions}
                          dropTarget={dropTargetTrackId === row.track.id ? dropTargetState : null}
                        />
                      </div>
                      {geometry.keyframeLane?.trackId === row.track.id && <KeyframeLane geometry={geometry} />}
                    </Fragment>
                  ))}
                  <TimelineInteractionLayer geometry={geometry} drag={drag} trim={trim} drop={dropIndicator} transitionDropCut={transitionDropCut} />
                  {mobile && <MainTrackAddButton geometry={geometry} />}
                </div>
              </TimelineContextMenu>
            </div>
          </div>
          <Playhead
            pixelsPerSecond={geometry.pixelsPerSecond}
            // Mobile content starts after the centre padding; a paused follow draws the playhead at its time.
            scrollLeft={scrollLeft - centerPadding}
            offsetLeft={headerWidth}
            viewportWidth={geometry.viewportWidth}
            centered={mobile && !touch.followPaused}
          />
        </div>
        {!mobile && (
          <div className="shrink-0 pb-1.5 pr-2 pt-1" style={{ paddingLeft: headerWidth + 8 }}>
            <OverviewBar geometry={geometry} controlsId={scrollerId} />
          </div>
        )}
      </div>
      <TimelineFeedback />
    </section>
  );
}
