import { ArrowRightToLine, Blend, Moon, Sun, type LucideIcon } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent, type PointerEvent } from "react";
import type { TimelineTrack, TimelineTransition, TransitionKind } from "@/lib/timeline";
import { clampTransitionSeconds, transitionDurationRange, transitionName } from "@/lib/timeline-ops/transition-commands";
import { formatTransitionMaxSeconds, formatTransitionSeconds, TRANSITION_SECONDS_EPSILON } from "@/lib/timeline-ops/transitions";
import { cn } from "@/lib/utils";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { followPointer } from "./pointer-session";
import { laneLayerClass } from "./timeline-layers";
import { useTransitionCommands } from "./transition-commands";
import { timelineClipInset } from "./use-timeline-geometry";

export const transitionKindIcons: Readonly<Record<TransitionKind, LucideIcon>> = {
  crossfade: Blend,
  dipToBlack: Moon,
  dipToWhite: Sun,
  wipe: ArrowRightToLine,
};

const badgeSize = { desktop: { width: 22, height: 18 }, touch: { width: 30, height: 26 } } as const;
/** Edge handle hit widths; touch handles are at least 24 px. */
export const transitionHandleHitWidth = { desktop: 8, touch: 24 } as const;

type Edge = "left" | "right";
const edges: readonly Edge[] = ["left", "right"];
/** Visible width of a selected transition's edge grip; the hit box around it is wider. */
const gripWidth = 3;

interface TransitionBadgeProps {
  readonly track: TimelineTrack;
  readonly transition: TimelineTransition;
  readonly pixelsPerSecond: number;
  readonly rowHeight: number;
  /** Touch layouts enlarge the badge and its edge handles. */
  readonly touch: boolean;
}

/**
 * A transition on its cut: a subtle window over both clips and a badge button centered on the
 * cut. The window and the selected edge grips paint under clip labels; the badge and the edge hit
 * boxes paint on top. Clicking selects it (Properties shows its Transition tab). While selected on an unlocked
 * track, dragging either window edge changes the duration symmetrically, clamped to one frame and
 * the cut's maximum, with a live readout; release commits one `updateTransition`.
 */
export function TransitionBadge({ track, transition, pixelsPerSecond, rowHeight, touch }: TransitionBadgeProps) {
  const store = useEditorStoreApi();
  const commands = useTransitionCommands();
  const selected = useEditorStore((state) => state.selectedTransitionId === transition.id);
  const project = useEditorStore((state) => state.project);
  const range = useMemo(() => transitionDurationRange(project, track, transition), [project, track, transition]);
  const [liveSeconds, setLiveSeconds] = useState<number | null>(null);
  const stopDrag = useRef<(() => void) | null>(null);
  useEffect(() => () => stopDrag.current?.(), []);

  const right = track.items.find((item) => item.id === transition.rightItemId);
  if (!right || !range) return null;

  const durationSeconds = liveSeconds ?? transition.durationSeconds;
  const centerX = right.startSeconds * pixelsPerSecond;
  const halfWidth = (durationSeconds * pixelsPerSecond) / 2;
  const badge = touch ? badgeSize.touch : badgeSize.desktop;
  const hit = touch ? transitionHandleHitWidth.touch : transitionHandleHitWidth.desktop;
  const edgeOffset = Math.max(halfWidth, badge.width / 2 + hit / 2);
  const laneHeight = Math.max(1, rowHeight - timelineClipInset * 2);
  const atMax = liveSeconds !== null && liveSeconds >= range.max - TRANSITION_SECONDS_EPSILON;
  const Icon = transitionKindIcons[transition.kind];
  const name = transitionName({ kind: transition.kind, durationSeconds });
  const handlesShown = selected && !track.locked;
  const edgeCenter = (edge: Edge) => centerX + (edge === "left" ? -edgeOffset : edgeOffset);

  function onBadgeClick(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    store.getState().selectTransition(transition.id);
  }

  function onHandlePointerDown(event: PointerEvent<HTMLDivElement>, edge: Edge) {
    if (event.button !== 0 || !range) return;
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startSeconds = transition.durationSeconds;
    const direction = edge === "right" ? 1 : -1;
    let latest = startSeconds;
    stopDrag.current?.();
    stopDrag.current = followPointer(event.pointerId, {
      move: (point) => {
        const raw = startSeconds + (direction * 2 * (point.clientX - startX)) / pixelsPerSecond;
        latest = clampTransitionSeconds(range, Math.round(raw * 100) / 100);
        setLiveSeconds(latest);
      },
      release: () => {
        stopDrag.current = null;
        if (Math.abs(latest - startSeconds) <= TRANSITION_SECONDS_EPSILON) {
          setLiveSeconds(null);
          return;
        }
        void commands.setDuration(transition.id, latest).finally(() => setLiveSeconds(null));
      },
      cancel: () => {
        stopDrag.current = null;
        setLiveSeconds(null);
      },
    });
  }

  return (
    <>
      <span
        aria-hidden
        data-testid="transition-window"
        className={cn(
          "pointer-events-none absolute border-x bg-[linear-gradient(90deg,hsl(var(--foreground)/0.04),hsl(var(--foreground)/0.22),hsl(var(--foreground)/0.04))]",
          laneLayerClass.transitionOverlay,
          selected ? "border-foreground" : "border-foreground/40",
        )}
        style={{ left: centerX - halfWidth, width: Math.max(1, halfWidth * 2), top: timelineClipInset, height: laneHeight }}
      />
      <button
        type="button"
        aria-label={name}
        aria-pressed={selected}
        data-transition-id={transition.id}
        title={name}
        onPointerDown={(event) => event.stopPropagation()}
        onClick={onBadgeClick}
        style={{ left: centerX - badge.width / 2, top: (rowHeight - badge.height) / 2, width: badge.width, height: badge.height }}
        className={cn(
          laneLayerClass.controls,
          "absolute grid place-items-center rounded-[5px] border shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          selected ? "border-foreground bg-foreground text-background" : "border-line bg-popover text-foreground hover:bg-hover",
        )}
      >
        <Icon className={touch ? "h-4 w-4" : "h-3 w-3"} aria-hidden />
      </button>
      {handlesShown &&
        edges.map((edge) => (
          <span
            key={edge}
            aria-hidden
            data-transition-grip={edge}
            style={{ left: edgeCenter(edge) - gripWidth / 2, width: gripWidth, top: timelineClipInset, height: laneHeight }}
            className={cn("pointer-events-none absolute rounded-full bg-foreground", laneLayerClass.transitionOverlay)}
          />
        ))}
      {handlesShown &&
        edges.map((edge) => (
          <div
            key={edge}
            aria-hidden
            data-transition-handle={edge}
            onPointerDown={(event) => onHandlePointerDown(event, edge)}
            style={{ left: edgeCenter(edge) - hit / 2, width: hit, top: timelineClipInset, height: laneHeight }}
            className={cn("absolute cursor-ew-resize touch-none", laneLayerClass.controls)}
          />
        ))}
      {liveSeconds !== null && (
        <div
          role="status"
          data-testid="transition-duration-tooltip"
          className="tabular-time pointer-events-none absolute z-30 -translate-x-1/2 whitespace-nowrap rounded-[4px] bg-popover px-1.5 text-[11px] leading-5 text-popover-foreground shadow-sm"
          style={{ left: centerX, top: -22 }}
        >
          {`${formatTransitionSeconds(liveSeconds)}s${atMax ? ` · max ${formatTransitionMaxSeconds(range.max)}s` : ""}`}
        </div>
      )}
    </>
  );
}
