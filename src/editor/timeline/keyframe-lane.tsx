import { ChevronDown, Diamond } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { ProjectActionKeyframe, ProjectActionKeyframeProperty } from "@/lib/project";
import type { Timeline } from "@/lib/timeline";
import { automationPointPosition } from "@/lib/timeline-ops/automation";
import { keyframeLaneFor, type KeyframeLaneModel } from "@/lib/timeline-ops/keyframe-lane";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import { keyframeLanePadding, useKeyframeLane } from "./use-keyframe-lane";
import type { TimelineGeometry } from "./use-timeline-geometry";

function locateItem(timeline: Timeline, itemId: string | undefined) {
  for (const track of timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return { item, track };
  }
  return null;
}

/** Keyframe diamonds draw at 10 px but take presses in a 24 px square, the minimum touch target. */
export const keyframeDiamondHitSize = 24;

function formatValue(value: number): string {
  return Number(value.toFixed(3)).toString();
}

function LanePropertyMenu({ model }: { readonly model: KeyframeLaneModel }) {
  const setLaneProperty = useEditorStore((state) => state.setLaneProperty);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={`Keyframe property: ${model.config.label}`}
          className="flex h-6 w-full min-w-0 items-center gap-1 rounded-[5px] pl-1.5 pr-1 text-[11px] text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <Diamond className="h-3 w-3 shrink-0 fill-keyframe text-keyframe" aria-hidden />
          <span className="min-w-0 flex-1 truncate text-left">{model.config.label}</span>
          <ChevronDown className="h-3 w-3 shrink-0 text-dim" aria-hidden />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="min-w-40">
        <DropdownMenuRadioGroup
          value={model.config.property}
          onValueChange={(value) => setLaneProperty(value as ProjectActionKeyframeProperty)}
        >
          {model.configs.map((config) => (
            <DropdownMenuRadioItem key={config.property} value={config.property}>
              {config.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/**
 * The keyframe lane under the selected clip's track (keyframes on, one clip selected): a
 * property menu in the header column and draggable, focusable diamonds over the clip span.
 * Double-click the lane to add a keyframe; Delete removes the focused one.
 */
export function KeyframeLane({ geometry }: { readonly geometry: TimelineGeometry }) {
  const timeline = useEditorStore((state) => state.project.timeline);
  const selectedItemId = useEditorStore((state) => (state.selectedItemIds.length === 1 ? state.selectedItemIds[0] : undefined));
  const laneProperty = useEditorStore((state) => state.laneProperty);
  const lane = geometry.keyframeLane;
  const location = locateItem(timeline, selectedItemId);
  const model = location ? keyframeLaneFor(location.item, laneProperty) : null;
  if (!lane || !location || !model || location.track.id !== lane.trackId) return null;
  return <KeyframeLaneRow geometry={geometry} height={lane.height} item={location.item} locked={location.track.locked} model={model} />;
}

interface KeyframeLaneRowProps {
  readonly geometry: TimelineGeometry;
  readonly height: number;
  readonly item: NonNullable<ReturnType<typeof locateItem>>["item"];
  readonly locked: boolean;
  readonly model: KeyframeLaneModel;
}

function KeyframeLaneRow({ geometry, height, item, locked, model }: KeyframeLaneRowProps) {
  const { pixelsPerSecond, headerWidth, contentWidth } = geometry;
  const laneRef = useRef<HTMLDivElement>(null);
  const { drag, pendingFocusSeconds, onDiamondPointerDown, onDiamondKeyDown, onLaneDoubleClick } = useKeyframeLane({
    item,
    model,
    pixelsPerSecond,
    laneHeight: height,
    locked,
  });
  const { config } = model;
  const innerHeight = Math.max(1, height - keyframeLanePadding * 2);

  useLayoutEffect(() => {
    const seconds = pendingFocusSeconds.current;
    if (seconds === null) return;
    const target = laneRef.current?.querySelector<HTMLElement>(`[data-keyframe-at="${seconds}"]`);
    if (!target) return;
    pendingFocusSeconds.current = null;
    target.focus({ preventScroll: true });
  });

  const points = model.keyframes
    .map((keyframe): ProjectActionKeyframe => (drag && keyframe.atSeconds === drag.fromSeconds ? { ...keyframe, atSeconds: drag.atSeconds, value: drag.value } : keyframe))
    .sort((left, right) => left.atSeconds - right.atSeconds);
  const placed = points.map((keyframe) => {
    const position = automationPointPosition(item, config, keyframe);
    return {
      keyframe,
      x: (item.startSeconds + keyframe.atSeconds) * pixelsPerSecond,
      y: keyframeLanePadding + (position.top / 100) * innerHeight,
    };
  });
  const clipLeft = item.startSeconds * pixelsPerSecond;
  const clipRight = (item.startSeconds + item.durationSeconds) * pixelsPerSecond;
  const first = placed[0];
  const last = placed.at(-1);
  const curve = first && last ? [{ x: clipLeft, y: first.y }, ...placed, { x: clipRight, y: last.y }] : [];
  const dragged = drag ? placed.find((point) => point.keyframe.atSeconds === drag.atSeconds) : undefined;

  return (
    <div className="flex" data-testid="keyframe-lane-row" style={{ height }}>
      {headerWidth > 0 && (
        <div className="sticky left-0 z-20 flex shrink-0 items-center border-r border-line bg-panel px-1" style={{ width: headerWidth }}>
          <LanePropertyMenu model={model} />
        </div>
      )}
      <div
        ref={laneRef}
        role="group"
        aria-label={`${config.label} keyframe lane for ${item.label}`}
        onDoubleClick={onLaneDoubleClick}
        className="relative shrink-0 border-y border-line bg-background/40"
        style={{ width: contentWidth, height }}
      >
        {headerWidth === 0 && (
          <div className="sticky left-1 z-10 inline-flex w-28 pt-2">
            <LanePropertyMenu model={model} />
          </div>
        )}
        <div
          aria-hidden
          className="absolute rounded-[4px] bg-keyframe/[0.07]"
          style={{ left: clipLeft, width: Math.max(2, clipRight - clipLeft), top: 2, bottom: 2 }}
        />
        {placed.length === 0 && !locked && clipRight - clipLeft > 160 && (
          <span aria-hidden className="pointer-events-none absolute top-1/2 -translate-y-1/2 truncate text-[11px] text-dim" style={{ left: clipLeft + 8, maxWidth: clipRight - clipLeft - 16 }}>
            Double-click to add a keyframe
          </span>
        )}
        {curve.length > 0 && (
          <svg aria-hidden className="pointer-events-none absolute left-0 top-0 overflow-visible text-keyframe/60" width={contentWidth} height={height}>
            <polyline fill="none" stroke="currentColor" strokeWidth={1.25} points={curve.map((point) => `${point.x},${point.y}`).join(" ")} />
          </svg>
        )}
        {placed.map(({ keyframe, x, y }) => (
          <button
            key={keyframe.atSeconds}
            type="button"
            data-keyframe-at={keyframe.atSeconds}
            aria-label={`${config.label} keyframe at ${keyframe.atSeconds.toFixed(2)} seconds, value ${formatValue(keyframe.value)}`}
            aria-disabled={locked || undefined}
            title={locked ? "Unlock the track to edit keyframes" : "Drag to change time and value. Arrow keys change time or value; Delete removes."}
            onPointerDown={(event) => onDiamondPointerDown(event, keyframe)}
            onKeyDown={(event) => onDiamondKeyDown(event, keyframe)}
            // An invisible touch-sized hit box centred on the point; the diamond inside it stays small.
            style={{ left: x, top: y, width: keyframeDiamondHitSize, height: keyframeDiamondHitSize }}
            className={cn(
              "group absolute z-10 grid -translate-x-1/2 -translate-y-1/2 touch-none place-items-center rounded-full focus-visible:outline-none",
              locked ? "cursor-default" : "cursor-grab active:cursor-grabbing",
            )}
          >
            <span
              aria-hidden
              data-testid="keyframe-diamond"
              className={cn(
                "h-2.5 w-2.5 rotate-45 rounded-[2px] border border-background bg-keyframe",
                "group-focus-visible:ring-2 group-focus-visible:ring-ring group-focus-visible:ring-offset-1 group-focus-visible:ring-offset-background",
                locked && "opacity-60",
              )}
            />
          </button>
        ))}
        {drag && dragged && (
          <span
            aria-hidden
            className="tabular-time pointer-events-none absolute z-20 -translate-y-1/2 whitespace-nowrap rounded-[4px] bg-popover px-1.5 text-[11px] leading-5 text-popover-foreground shadow-sm"
            style={{ left: dragged.x + 10, top: height / 2 }}
          >
            {drag.atSeconds.toFixed(2)}s · {formatValue(drag.value)}
          </span>
        )}
      </div>
    </div>
  );
}
