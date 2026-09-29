import { useEditorStore } from "../store/editor-store-context";

interface PlayheadProps {
  readonly pixelsPerSecond: number;
  /** Horizontal scroll of the lanes, in content pixels. */
  readonly scrollLeft: number;
  /** Width of the header column the lanes start after. */
  readonly offsetLeft: number;
  /** Lanes viewport width; the playhead hides outside it. */
  readonly viewportWidth: number;
  /** Mobile: fixed at the viewport centre while the content scrolls under it. */
  readonly centered?: boolean;
}

/**
 * Playhead line with a cap, drawn over the ruler and lanes. It is positioned in viewport
 * coordinates (outside the scroll content) so it spans every row at any vertical scroll.
 */
export function Playhead({ pixelsPerSecond, scrollLeft, offsetLeft, viewportWidth, centered = false }: PlayheadProps) {
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const x = centered ? viewportWidth / 2 : playheadSeconds * pixelsPerSecond - scrollLeft;
  if (x < 0 || x > viewportWidth) return null;
  return (
    <div
      data-testid="playhead"
      data-seconds={playheadSeconds.toFixed(3)}
      aria-hidden
      className="pointer-events-none absolute inset-y-0 z-30 w-0.5 -translate-x-1/2 bg-foreground"
      style={{ left: offsetLeft + x }}
    >
      <span className="absolute left-1/2 top-0 h-3 w-3 -translate-x-1/2 bg-foreground [clip-path:polygon(0_0,100%_0,100%_55%,50%_100%,0_55%)]" />
    </div>
  );
}
