import { useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";
import type { TimelineItem } from "@/lib/timeline";
import { minimumClipDurationSeconds } from "@/lib/timeline-ops/clip-drag";
import type { ClipTrimEdge } from "@/lib/timeline-ops/clip-trim";
import { trackEnabled } from "@/lib/timeline-ops/navigation";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import { ClipTrimHandle } from "./clip-trim-handle";
import { TimelineClip } from "./timeline-clip";
import { TransitionBadge } from "./transition-badge";
import type { LivePlacement } from "./use-clip-trim";
import { timelineClipInset, visibleTimelineItems, type TimelineGeometry, type TimelineRow } from "./use-timeline-geometry";

/** Pointer and keyboard wiring from the timeline panel's drag, trim and marquee hooks. */
export interface LaneInteractions {
  readonly draggingItemIds: ReadonlySet<string>;
  /** Live clip boxes while trimming (including ripple-shifted neighbours). */
  readonly livePlacements: ReadonlyMap<string, LivePlacement>;
  onClipPointerDown(event: PointerEvent<HTMLDivElement>, item: TimelineItem): void;
  onTrimPointerDown(event: PointerEvent<HTMLDivElement>, item: TimelineItem, edge: ClipTrimEdge): void;
  onTrimKeyDown(event: KeyboardEvent<HTMLDivElement>, item: TimelineItem, edge: ClipTrimEdge): void;
}

interface TrackLaneProps {
  readonly row: TimelineRow;
  readonly geometry: Pick<TimelineGeometry, "pixelsPerSecond" | "window" | "contentWidth" | "renderedDurationSeconds">;
  /** Touch layouts widen trim handle hit areas. */
  readonly touch: boolean;
  readonly interactions?: LaneInteractions;
  /** This row is the target of a drag: tinted when it accepts, outlined red when it rejects. */
  readonly dropTarget?: "accepted" | "rejected" | null;
}

const noInteractions: LaneInteractions = {
  draggingItemIds: new Set(),
  livePlacements: new Map(),
  onClipPointerDown: () => undefined,
  onTrimPointerDown: () => undefined,
  onTrimKeyDown: () => undefined,
};

/** Keeps very short changed ranges visible at low zoom. */
const minimumHighlightWidth = 2;

function byStart(left: TimelineItem, right: TimelineItem): number {
  return left.startSeconds - right.startSeconds || left.id.localeCompare(right.id);
}

/**
 * One track as a horizontal listbox of its clips inside the render window. Options use a roving
 * tab stop: Arrow Left/Right move focus in time order (rendering off-window clips on demand) and
 * Enter or Space selects, toggling with Shift, Ctrl or Cmd.
 */
export function TrackLane({ row, geometry, touch, interactions = noInteractions, dropTarget = null }: TrackLaneProps) {
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const selectedGap = useEditorStore((state) => (state.selectedGap?.trackId === row.track.id ? state.selectedGap : null));
  const highlightedItemIds = useEditorStore((state) => state.highlightedItemIds);
  const highlightedRanges = useEditorStore((state) => state.highlightedRanges);
  const selectItems = useEditorStore((state) => state.selectItems);
  const toggleItemSelection = useEditorStore((state) => state.toggleItemSelection);
  const [focusedItemId, setFocusedItemId] = useState<string | null>(null);
  const pendingFocusId = useRef<string | null>(null);
  const laneRef = useRef<HTMLDivElement>(null);
  const { track } = row;

  const orderedItems = useMemo(() => [...track.items].sort(byStart), [track.items]);
  const selected = useMemo(() => new Set(selectedItemIds), [selectedItemIds]);
  const highlighted = useMemo(() => new Set(highlightedItemIds), [highlightedItemIds]);
  // A range without track ids spans every track.
  const ranges = highlightedRanges.filter((range) => range.trackIds.length === 0 || range.trackIds.includes(track.id));
  const persistentIds = useMemo(() => {
    const ids = new Set(selectedItemIds);
    if (focusedItemId) ids.add(focusedItemId);
    return ids;
  }, [selectedItemIds, focusedItemId]);
  const visibleItems = visibleTimelineItems(orderedItems, geometry.window, persistentIds);
  const tabStopId =
    visibleItems.find((item) => item.id === focusedItemId)?.id ??
    visibleItems.find((item) => selected.has(item.id))?.id ??
    visibleItems[0]?.id;

  function optionElement(itemId: string): HTMLElement | null {
    const options = laneRef.current?.querySelectorAll<HTMLElement>('[role="option"]') ?? [];
    return Array.from(options).find((option) => option.dataset.itemId === itemId) ?? null;
  }

  useLayoutEffect(() => {
    const itemId = pendingFocusId.current;
    if (!itemId) return;
    pendingFocusId.current = null;
    optionElement(itemId)?.focus();
  });

  function focusItem(item: TimelineItem) {
    const rendered = optionElement(item.id);
    if (rendered) rendered.focus();
    else pendingFocusId.current = item.id;
    setFocusedItemId(item.id);
  }

  function select(item: TimelineItem, additive: boolean) {
    if (additive) toggleItemSelection(item.id);
    else selectItems([item.id]);
    setFocusedItemId(item.id);
  }

  function onOptionKeyDown(event: KeyboardEvent<HTMLDivElement>, item: TimelineItem) {
    const additive = event.shiftKey || event.metaKey || event.ctrlKey;
    if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
      if (event.altKey || additive) return;
      const next = orderedItems[orderedItems.findIndex((entry) => entry.id === item.id) + (event.key === "ArrowRight" ? 1 : -1)];
      if (next) focusItem(next);
    } else if (event.key === "Enter" || event.key === " ") {
      select(item, additive);
    } else {
      return;
    }
    event.preventDefault();
  }

  const handleItems = track.locked
    ? []
    : visibleItems.filter((item) => selected.has(item.id) && !interactions.draggingItemIds.has(item.id));

  return (
    <div
      data-track-id={track.id}
      data-drop-target={dropTarget ?? undefined}
      style={{ width: geometry.contentWidth, height: row.height }}
      className={cn(
        "relative isolate shrink-0",
        dropTarget === "accepted" && "bg-foreground/[0.04]",
        dropTarget === "rejected" && "bg-destructive/[0.06] shadow-[inset_0_0_0_1px_hsl(var(--destructive))]",
      )}
    >
      {/* Changed-range bands paint first, below the clips, selection outlines, transition badges and
          trim handles, so they tint the lane around clips without washing out clip content. */}
      {ranges.map((range) => (
        <div
          key={`${range.startSeconds.toString()}-${range.endSeconds.toString()}`}
          data-testid="timeline-highlight-range"
          aria-hidden
          className="pointer-events-none absolute inset-y-0 border-x border-accent/70 bg-accent-soft"
          style={{
            left: range.startSeconds * geometry.pixelsPerSecond,
            width: Math.max(minimumHighlightWidth, (range.endSeconds - range.startSeconds) * geometry.pixelsPerSecond),
          }}
        />
      ))}
      <div
        ref={laneRef}
        role="listbox"
        aria-label={row.name}
        aria-multiselectable="true"
        aria-orientation="horizontal"
        data-locked={track.locked || undefined}
        style={{ height: row.height }}
        className="absolute inset-0"
      >
        {visibleItems.map((item) => (
          <TimelineClip
            key={item.id}
            item={item}
            pixelsPerSecond={geometry.pixelsPerSecond}
            rowHeight={row.height}
            selected={selected.has(item.id)}
            highlighted={highlighted.has(item.id)}
            placement={interactions.livePlacements.get(item.id)}
            dragging={interactions.draggingItemIds.has(item.id)}
            dimmed={!trackEnabled(track)}
            tabIndex={item.id === tabStopId ? 0 : -1}
            onPointerDown={interactions === noInteractions ? undefined : interactions.onClipPointerDown}
            onSelect={select}
            onKeyDown={onOptionKeyDown}
            onFocus={(focused) => setFocusedItemId(focused.id)}
          />
        ))}
      </div>
      {selectedGap && (
        <div
          data-testid="timeline-selected-gap"
          title="Selected empty gap. Press Delete to ripple-close it."
          aria-hidden
          className="pointer-events-none absolute rounded-clip border border-dashed border-accent bg-accent/10"
          style={{
            left: selectedGap.startSeconds * geometry.pixelsPerSecond,
            width: (selectedGap.endSeconds - selectedGap.startSeconds) * geometry.pixelsPerSecond,
            top: timelineClipInset,
            bottom: timelineClipInset,
          }}
        />
      )}
      {(track.transitions ?? []).map((transition) => (
        <TransitionBadge
          key={transition.id}
          track={track}
          transition={transition}
          pixelsPerSecond={geometry.pixelsPerSecond}
          rowHeight={row.height}
          touch={touch}
        />
      ))}
      {handleItems.map((item) => {
        const placement = interactions.livePlacements.get(item.id) ?? item;
        const end = placement.startSeconds + placement.durationSeconds;
        return (["left", "right"] as const).map((edge) => (
          <ClipTrimHandle
            key={`${item.id}:${edge}`}
            item={item}
            edge={edge}
            placement={placement}
            pixelsPerSecond={geometry.pixelsPerSecond}
            rowHeight={row.height}
            maxSeconds={edge === "left" ? end - minimumClipDurationSeconds : geometry.renderedDurationSeconds}
            touch={touch}
            onPointerDown={interactions.onTrimPointerDown}
            onKeyDown={interactions.onTrimKeyDown}
          />
        ));
      })}
    </div>
  );
}
