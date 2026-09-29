/** A rendered track row, in canvas content pixels (y grows downward). */
interface MarqueeRow {
  readonly trackId: string;
  readonly top: number;
  readonly bottom: number;
}

/** A rendered clip's horizontal extent, in canvas content pixels. */
interface MarqueeItemBox {
  readonly itemId: string;
  readonly trackId: string;
  readonly left: number;
  readonly right: number;
}

export interface MarqueeGeometry {
  readonly rows: readonly MarqueeRow[];
  readonly items: readonly MarqueeItemBox[];
  /** Vertical inset between the row edge and the clip body; defaults to 4 px. */
  readonly verticalInset?: number;
}

/** The marquee drag, from the pointer-down point to the current point, in the same space. */
export interface MarqueeRect {
  readonly startX: number;
  readonly startY: number;
  readonly endX: number;
  readonly endY: number;
}

const defaultVerticalInset = 4;
const marqueeClickThresholdPixels = 3;

/**
 * Ids of items whose clip box intersects the marquee, with inclusive edges. A clip's
 * vertical box is `row.top + inset` to `row.top + inset + max(1, rowHeight - 2 * inset)`.
 * Items whose track has no row in the geometry are ignored. Order follows `geometry.items`.
 */
export function itemsInMarquee(geometry: MarqueeGeometry, rect: MarqueeRect): string[] {
  const left = Math.min(rect.startX, rect.endX);
  const right = Math.max(rect.startX, rect.endX);
  const top = Math.min(rect.startY, rect.endY);
  const bottom = Math.max(rect.startY, rect.endY);
  const inset = geometry.verticalInset ?? defaultVerticalInset;
  const rowsByTrackId = new Map(geometry.rows.map((row) => [row.trackId, row]));

  return geometry.items
    .filter((item) => {
      const row = rowsByTrackId.get(item.trackId);
      if (!row) return false;
      const itemTop = row.top + inset;
      const itemBottom = itemTop + Math.max(1, row.bottom - row.top - inset * 2);
      return item.right >= left && item.left <= right && itemBottom >= top && itemTop <= bottom;
    })
    .map((item) => item.itemId);
}

/** Pointer travel under 3 px is a click on empty space, not a marquee selection. */
export function isMarqueeClick(rect: MarqueeRect): boolean {
  return (
    Math.hypot(rect.endX - rect.startX, rect.endY - rect.startY) < marqueeClickThresholdPixels
  );
}
