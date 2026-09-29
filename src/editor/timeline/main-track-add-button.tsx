import { Plus } from "lucide-react";
import { useEditorStore } from "../store/editor-store-context";
import type { TimelineGeometry } from "./use-timeline-geometry";

const buttonSize = 32;
const gapPixels = 8;

/**
 * Mobile `+` after the last clip of the main track, opening the Media sheet. It sits in a strip
 * that ends where the button belongs and sticks to the viewport's right edge, so it stays in
 * reach (over the clips) while the end of the track is scrolled out of view.
 */
export function MainTrackAddButton({ geometry }: { readonly geometry: Pick<TimelineGeometry, "rows" | "pixelsPerSecond"> }) {
  const openSheet = useEditorStore((state) => state.openSheet);
  const row = geometry.rows.find((candidate) => candidate.main);
  if (!row) return null;
  const endSeconds = row.track.items.reduce((end, item) => Math.max(end, item.startSeconds + item.durationSeconds), 0);
  return (
    <div
      className="pointer-events-none absolute left-0 z-20 flex items-center"
      style={{ top: row.top, height: row.height, width: endSeconds * geometry.pixelsPerSecond + gapPixels + buttonSize }}
    >
      <button
        type="button"
        aria-label="Add media"
        onPointerDown={(event) => event.stopPropagation()}
        onClick={() => openSheet("media")}
        className="pointer-events-auto sticky right-2 ml-auto grid shrink-0 place-items-center rounded-control bg-foreground text-background shadow-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-panel"
        style={{ width: buttonSize, height: buttonSize }}
      >
        <Plus className="h-5 w-5" aria-hidden />
      </button>
    </div>
  );
}
