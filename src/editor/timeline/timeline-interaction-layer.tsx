import { formatTimecode } from "@/lib/format";
import type { TransitionCut } from "@/lib/timeline-ops/transitions";
import { cn } from "@/lib/utils";
import { useEditorStore } from "../store/editor-store-context";
import type { ClipDragState } from "./use-clip-drag";
import type { ClipTrimState } from "./use-clip-trim";
import type { TimelineDropIndicator } from "./use-timeline-drop";
import { timelineClipInset, timelineRowHeight, type TimelineGeometry } from "./use-timeline-geometry";

interface TimelineInteractionLayerProps {
  readonly geometry: TimelineGeometry;
  readonly drag: ClipDragState | null;
  readonly trim: ClipTrimState | null;
  /** A panel asset dragged over the tracks area. */
  readonly drop?: TimelineDropIndicator | null;
  /** The cut a dragged transition tile would land on. */
  readonly transitionDropCut?: TransitionCut | null;
}

const tooltipHeight = 20;

function timing(startSeconds: number, durationSeconds: number): string {
  return `${formatTimecode(startSeconds)} · ${formatTimecode(durationSeconds)}`;
}

interface Box {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

function TimelineTooltip({ anchor, text, rejected }: { anchor: Box; text: string; rejected: boolean }) {
  return (
    <div
      role="status"
      data-testid="timeline-interaction-tooltip"
      className={cn(
        "tabular-time absolute z-10 whitespace-nowrap rounded-[4px] px-1.5 text-[11px] leading-5 shadow-sm",
        rejected ? "bg-destructive text-destructive-foreground" : "bg-popover text-popover-foreground",
      )}
      style={{ left: Math.max(0, anchor.left), top: anchor.top >= tooltipHeight + 2 ? anchor.top - tooltipHeight - 2 : anchor.top + anchor.height + 2 }}
    >
      {text}
    </div>
  );
}

/**
 * Drag and trim feedback drawn over the lanes (tracks-area pixels): clip ghosts at the target,
 * a red outline when the move is rejected, the new-track insert line, the snap guide, a
 * start · duration tooltip (or the rejection reason) and the marquee rectangle.
 */
export function TimelineInteractionLayer({ geometry, drag, trim, drop = null, transitionDropCut = null }: TimelineInteractionLayerProps) {
  const marquee = useEditorStore((state) => state.marquee);
  const timeline = useEditorStore((state) => state.project.timeline);
  const { pixelsPerSecond, rows, totalHeight } = geometry;
  const guideSeconds = drag?.preview.guideSeconds ?? trim?.guideSeconds ?? null;
  const newTrack = drag?.preview.newTrack ?? null;
  const insertTop = newTrack ? (rows[newTrack.insertIndex]?.top ?? totalHeight) : null;

  function ghostBox(trackId: string, startSeconds: number, durationSeconds: number): Box | null {
    const row = rows.find((candidate) => candidate.track.id === trackId);
    const height = (row?.height ?? timelineRowHeight.other) - timelineClipInset * 2;
    const top = row ? row.top + timelineClipInset : insertTop === null ? null : insertTop - height / 2;
    if (top === null) return null;
    return { left: startSeconds * pixelsPerSecond, top, width: Math.max(2, durationSeconds * pixelsPerSecond), height };
  }

  const rejected = drag?.preview.state === "rejected";
  const ghosts = (drag?.preview.placements ?? []).flatMap((placement) => {
    const box = ghostBox(placement.trackId, placement.startSeconds, placement.durationSeconds);
    const label = timeline.tracks.flatMap((track) => track.items).find((item) => item.id === placement.itemId)?.label ?? "";
    return box ? [{ placement, box, label }] : [];
  });
  const lead = ghosts.find((ghost) => ghost.placement.itemId === drag?.leadItemId);

  const cutRow = transitionDropCut ? rows.find((row) => row.track.id === transitionDropCut.trackId) : undefined;
  const dropRow = drop?.placement.hoveredTrackId ? rows.find((row) => row.track.id === drop.placement.hoveredTrackId) : undefined;
  const trimmed = trim ? trim.placements.get(trim.itemId) : undefined;
  const trimTrackId = trim ? rows.find((row) => row.track.items.some((item) => item.id === trim.itemId))?.track.id : undefined;
  const trimBox = trimmed && trimTrackId ? ghostBox(trimTrackId, trimmed.startSeconds, trimmed.durationSeconds) : null;

  return (
    <div
      aria-hidden={!drag && !trim && !drop && !transitionDropCut}
      data-testid="timeline-interaction-layer"
      className="pointer-events-none absolute top-0 z-10"
      style={{ left: geometry.headerWidth, width: geometry.contentWidth, height: totalHeight }}
    >
      {insertTop !== null && (
        <div data-testid="timeline-insert-line" className="absolute inset-x-0 h-0.5 -translate-y-1/2 bg-accent" style={{ top: insertTop }} />
      )}
      {drop && drop.insertTop !== null && (
        <div data-testid="timeline-drop-line" className="absolute inset-x-0 h-0.5 -translate-y-1/2 bg-accent" style={{ top: drop.insertTop }} />
      )}
      {drop && (
        <div
          data-testid="timeline-drop-marker"
          data-rejected={drop.rejected || undefined}
          className={cn("absolute w-0.5 -translate-x-1/2", drop.rejected ? "bg-destructive" : "bg-accent")}
          style={{
            left: drop.placement.startSeconds * pixelsPerSecond,
            top: dropRow ? dropRow.top : 0,
            height: dropRow ? dropRow.height : totalHeight,
          }}
        />
      )}
      {transitionDropCut && cutRow && (
        <div
          data-testid="transition-drop-target"
          className="absolute w-1 -translate-x-1/2 rounded-full bg-accent shadow-[0_0_0_2px_hsl(var(--accent)/0.35)]"
          style={{ left: transitionDropCut.seconds * pixelsPerSecond, top: cutRow.top + timelineClipInset, height: cutRow.height - timelineClipInset * 2 }}
        />
      )}
      {transitionDropCut && cutRow && (
        <TimelineTooltip
          anchor={{ left: transitionDropCut.seconds * pixelsPerSecond - 40, top: cutRow.top, width: 80, height: cutRow.height }}
          rejected={false}
          text={transitionDropCut.transitionId ? "Change transition" : "Add transition"}
        />
      )}
      {ghosts.map(({ placement, box, label }) => (
        <div
          key={placement.itemId}
          data-testid="clip-drag-ghost"
          data-rejected={rejected || undefined}
          className={cn(
            "absolute flex items-center overflow-hidden rounded-clip border-2 px-1.5 text-[11px] text-foreground",
            rejected ? "border-destructive bg-destructive/20" : "border-foreground bg-foreground/15",
          )}
          style={box}
        >
          <span className="truncate">{drag?.duplicate ? `${label} copy` : label}</span>
        </div>
      ))}
      {guideSeconds !== null && (
        <div data-testid="timeline-snap-guide" className="absolute inset-y-0 w-px bg-accent" style={{ left: guideSeconds * pixelsPerSecond }} />
      )}
      {drag && lead && (
        <TimelineTooltip
          anchor={lead.box}
          rejected={rejected}
          text={rejected ? (drag.preview.reason ?? "Invalid destination") : timing(lead.placement.startSeconds, lead.placement.durationSeconds)}
        />
      )}
      {trim && trimmed && trimBox && (
        <TimelineTooltip
          anchor={trimBox}
          rejected={trim.state === "rejected"}
          text={
            trim.state === "rejected"
              ? (trim.reason ?? "Invalid resize")
              : `${timing(trimmed.startSeconds, trimmed.durationSeconds)}${trim.reason ? ` · ${trim.reason}` : ""}`
          }
        />
      )}
      {marquee && (
        <div
          data-testid="timeline-marquee"
          className="absolute rounded-[2px] border border-accent bg-accent/10"
          style={{
            left: Math.min(marquee.startX, marquee.endX),
            top: Math.min(marquee.startY, marquee.endY),
            width: Math.abs(marquee.endX - marquee.startX),
            height: Math.abs(marquee.endY - marquee.startY),
          }}
        />
      )}
    </div>
  );
}
