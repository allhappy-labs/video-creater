import { useId } from "react";
import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";

export interface RemovalConfirmation {
  /** Names what is removed, for example "Remove Parakeet TDT 0.6B v3?". */
  readonly title: string;
  /** States what stays unaffected. */
  readonly description: string;
  readonly onConfirm: () => void;
}

interface ConfirmRemovalDialogProps {
  readonly confirmation: RemovalConfirmation | null;
  readonly onClose: () => void;
}

/** In-app confirmation before a destructive settings removal; Cancel is focused first. */
export function ConfirmRemovalDialog({ confirmation, onClose }: ConfirmRemovalDialogProps) {
  const descriptionId = useId();

  return (
    <Dialog open={confirmation !== null} onOpenChange={(open) => !open && onClose()}>
      {confirmation ? (
        <DialogContent
          role="alertdialog"
          aria-describedby={descriptionId}
          className="left-1/2 top-1/2 grid w-[min(400px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 gap-3 rounded-md border bg-background p-4 text-xs shadow-xl"
        >
          <div>
            <DialogTitle className="text-sm font-semibold">{confirmation.title}</DialogTitle>
            <p id={descriptionId} className="mt-1 text-muted-foreground">
              {confirmation.description}
            </p>
          </div>
          <div className="flex justify-end gap-2">
            <DialogClose asChild>
              <Button type="button" variant="ghost" size="sm">
                Cancel
              </Button>
            </DialogClose>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="text-red-700 hover:text-red-800"
              onClick={() => {
                onClose();
                confirmation.onConfirm();
              }}
            >
              Remove
            </Button>
          </div>
        </DialogContent>
      ) : null}
    </Dialog>
  );
}
