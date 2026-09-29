import { useId } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import type { ProjectSpeakerIdentity } from "@/lib/project";
import { SpeakerNameList } from "../../properties/speakers-section";

interface SpeakersDialogProps {
  readonly open: boolean;
  readonly speakers: readonly ProjectSpeakerIdentity[];
  onOpenChange(open: boolean): void;
  rename(speakerId: string, name: string): Promise<void>;
}

/** Renames identified speakers; each name commits on blur or Enter, like the Properties Voice tab. */
export function SpeakersDialog({ open, speakers, onOpenChange, rename }: SpeakersDialogProps) {
  const descriptionId = useId();
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        aria-describedby={descriptionId}
        className="left-1/2 top-1/2 w-[min(360px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 rounded-panel border border-line p-4"
      >
        <DialogTitle className="text-[15px] font-semibold">Speakers</DialogTitle>
        <p id={descriptionId} className="mt-1 text-[13px] text-muted-foreground">
          Names appear on speaker marks across the project.
        </p>
        <div className="mt-3">
          {speakers.length === 0 ? (
            <p className="text-[12px] text-dim">No identified speakers yet.</p>
          ) : (
            <SpeakerNameList speakers={speakers} rename={rename} />
          )}
        </div>
        <div className="mt-4 flex justify-end">
          <DialogClose className="h-8 rounded-control px-3 text-[13px] font-medium text-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel">
            Done
          </DialogClose>
        </div>
      </DialogContent>
    </Dialog>
  );
}
