import { X } from "lucide-react";
import { useRef, useState, type PointerEvent, type ReactNode } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { safeAreaBottomClass } from "./use-safe-area";

/** Height of the mobile bottom tool bar that sheets can sit above, before the bottom safe-area inset. */
const mobileToolBarHeight = 78;
/** The tool bar's full height, including the home-indicator inset below it. */
export const mobileToolBarOuterHeight = `calc(${mobileToolBarHeight}px + env(safe-area-inset-bottom))`;
/** Dragging the grab handle down further than this closes the sheet; shorter drags spring back. */
export const sheetSwipeClosePixels = 80;
/** Handle travel that makes the press a drag rather than a tap. */
const dragSlopPixels = 4;

/**
 * Sheet heights: 40% for property tools and 70% for AI, Media and Audio. Tall viewports (iPad
 * portrait, over 1000 px) use 55% for both, so the preview and timeline stay in view.
 */
export const sheetHeightClass = {
  compact: "h-[40dvh] [@media(min-height:1001px)]:h-[55dvh]",
  tall: "h-[70dvh] [@media(min-height:1001px)]:h-[55dvh]",
} as const;

/** How far a sheet follows a handle drag from `startY` to `clientY`: downwards only. */
export function sheetDragOffset(startY: number, clientY: number): number {
  return Math.max(0, clientY - startY);
}

interface HandleDrag {
  readonly pointerId: number;
  readonly startY: number;
  offset: number;
}

/** The grab handle: drag the sheet down past 80 px to close it, or tap (or press Enter) to close. */
function useSheetDrag(onClose: () => void) {
  const [offset, setOffset] = useState(0);
  const drag = useRef<HandleDrag | null>(null);
  const dragged = useRef(false);

  function end(event: PointerEvent<HTMLButtonElement>, commit: boolean) {
    const active = drag.current;
    if (active?.pointerId !== event.pointerId) return;
    drag.current = null;
    if (commit && active.offset > sheetSwipeClosePixels) onClose();
    else setOffset(0);
  }

  return {
    offset,
    handleProps: {
      onPointerDown(event: PointerEvent<HTMLButtonElement>) {
        if (event.pointerType === "mouse" && event.button !== 0) return;
        // Capture keeps the drag on the handle when the finger leaves it (absent in jsdom).
        try {
          event.currentTarget.setPointerCapture?.(event.pointerId);
        } catch {
          // A pointer that is no longer active cannot be captured; the drag still tracks on the handle.
        }
        drag.current = { pointerId: event.pointerId, startY: event.clientY, offset: 0 };
        dragged.current = false;
      },
      onPointerMove(event: PointerEvent<HTMLButtonElement>) {
        const active = drag.current;
        if (active?.pointerId !== event.pointerId) return;
        active.offset = sheetDragOffset(active.startY, event.clientY);
        if (active.offset > dragSlopPixels) dragged.current = true;
        setOffset(active.offset);
      },
      onPointerUp: (event: PointerEvent<HTMLButtonElement>) => end(event, true),
      onPointerCancel: (event: PointerEvent<HTMLButtonElement>) => end(event, false),
      onClick() {
        // A drag that sprang back ends in a click; only a tap or keyboard press closes.
        if (dragged.current) {
          dragged.current = false;
          return;
        }
        onClose();
      },
    },
  };
}

export function BottomSheet({
  title,
  open,
  height,
  modal = true,
  aboveToolBar = false,
  onClose,
  children,
}: {
  title: string;
  open: boolean;
  height: "compact" | "tall";
  /** Non-modal sheets leave the rest of the editor visible and interactive; any outside press closes them. */
  modal?: boolean;
  /** Leaves the bottom tool bar uncovered, as property sheets do for the clip tools. */
  aboveToolBar?: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  const sheetDrag = useSheetDrag(onClose);
  return (
    <Dialog open={open} modal={modal} onOpenChange={(next) => !next && onClose()}>
      <DialogContent
        aria-describedby={undefined}
        data-sheet-height={height}
        className={cn(
          "inset-x-0 bottom-0 flex flex-col rounded-t-[18px]",
          sheetHeightClass[height],
          !aboveToolBar && safeAreaBottomClass,
          sheetDrag.offset === 0 && "transition-transform duration-150 motion-reduce:transition-none",
        )}
        style={{
          ...(aboveToolBar ? { bottom: mobileToolBarOuterHeight } : {}),
          ...(sheetDrag.offset > 0 ? { transform: `translateY(${sheetDrag.offset}px)` } : {}),
        }}
      >
        <button
          type="button"
          aria-label={`Dismiss ${title} sheet`}
          data-testid="sheet-grab-handle"
          {...sheetDrag.handleProps}
          className="group mx-auto grid h-6 w-24 shrink-0 cursor-grab touch-none place-items-center rounded-full focus-visible:outline-none active:cursor-grabbing"
        >
          <span className="h-[5px] w-[38px] rounded-full bg-hover group-focus-visible:ring-2 group-focus-visible:ring-ring" aria-hidden />
        </button>
        <div className="flex items-center px-3.5 pb-2">
          <DialogTitle className="font-semibold">{title}</DialogTitle>
          <div className="flex-1" />
          <DialogClose
            aria-label={`Close ${title}`}
            className="grid h-8 w-8 place-items-center rounded-control text-muted-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X className="h-4 w-4" aria-hidden />
          </DialogClose>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      </DialogContent>
    </Dialog>
  );
}
