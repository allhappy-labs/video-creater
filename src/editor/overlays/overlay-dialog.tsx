import { X } from "lucide-react";
import { useId, type ReactNode, type RefObject } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

interface OverlayDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly title: string;
  readonly description?: string;
  /** Receives focus on close; the menu item that opened the dialog has already unmounted. */
  readonly returnFocusRef?: RefObject<HTMLElement | null>;
  /** Width cap in px; the dialog always keeps a 16 px margin on narrow screens. */
  readonly size?: "md" | "lg";
  readonly children: ReactNode;
  readonly footer?: ReactNode;
}

/** Centred gear-menu dialog: a title row with Close, an optional description, a scrolling body, and a footer. */
export function OverlayDialog({ open, onOpenChange, title, description, returnFocusRef, size = "md", children, footer }: OverlayDialogProps) {
  const descriptionId = useId();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        aria-describedby={description ? descriptionId : undefined}
        onCloseAutoFocus={(event) => {
          const target = returnFocusRef?.current;
          if (!target) return;
          event.preventDefault();
          target.focus();
        }}
        className={cn(
          "left-1/2 top-1/2 flex max-h-[min(760px,calc(100dvh-32px))] -translate-x-1/2 -translate-y-1/2 flex-col rounded-panel border border-line",
          size === "lg" ? "w-[min(600px,calc(100vw-32px))]" : "w-[min(480px,calc(100vw-32px))]",
        )}
      >
        <div className="flex items-start gap-2 px-4 pt-3.5">
          <div className="min-w-0 flex-1 pt-1">
            <DialogTitle className="text-[15px] font-semibold">{title}</DialogTitle>
            {description && (
              <p id={descriptionId} className="mt-1 text-[13px] text-muted-foreground">
                {description}
              </p>
            )}
          </div>
          <DialogClose
            aria-label={`Close ${title}`}
            className="-mr-1.5 grid h-8 w-8 shrink-0 place-items-center rounded-control text-muted-foreground hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X className="h-4 w-4" aria-hidden />
          </DialogClose>
        </div>
        {children}
        {footer && <div className="flex shrink-0 items-center justify-end gap-2 border-t border-line px-4 py-3">{footer}</div>}
      </DialogContent>
    </Dialog>
  );
}
