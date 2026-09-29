import { useState } from "react";
import { formatTimecode } from "@/lib/format";
import { setItemDuration } from "@/lib/timeline-ops/clip-commands";
import type { EditorState } from "../store/editor-store";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { blockedReason } from "./timeline-availability";
import { useTimelineCommands } from "./timeline-commands";
import { DialogField, TimelineDialog } from "./timeline-dialog";

const invalidDurationCopy = "Enter a duration like 2.5 or 00:00:02.500.";

/** Seconds from "2", "2.5s", "1:30" or "00:00:02.500"; null when the text is not a duration. */
function parseDurationInput(text: string): number | null {
  const trimmed = text.trim().replace(/\s*s$/i, "");
  if (trimmed.length === 0) return null;
  const parts = trimmed.split(":");
  if (parts.length > 3) return null;
  let seconds = 0;
  for (const part of parts) {
    if (!/^\d+(\.\d+)?$|^\.\d+$/.test(part)) return null;
    seconds = seconds * 60 + Number(part);
  }
  return seconds;
}

function itemById(state: EditorState, itemId: string) {
  return state.project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === itemId) ?? null;
}

function SetDurationForm({ itemId, onClose }: { readonly itemId: string; readonly onClose: () => void }) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const label = useEditorStore((state) => itemById(state, itemId)?.label ?? "Clip");
  const [value, setValue] = useState(() => formatTimecode(itemById(store.getState(), itemId)?.durationSeconds ?? 0));
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    const seconds = parseDurationInput(value);
    const reason = seconds === null ? invalidDurationCopy : blockedReason(setItemDuration(store.getState().project, itemId, seconds));
    if (reason !== null || seconds === null) {
      setError(reason);
      return;
    }
    if (await commands.setDuration(itemId, seconds)) onClose();
    else setError(store.getState().lastError ?? "The duration could not be changed.");
  }

  return (
    <TimelineDialog
      open
      onOpenChange={(open) => !open && onClose()}
      title="Set duration"
      description={label}
      submitLabel="Set duration"
      onSubmit={() => void submit()}
    >
      <DialogField
        label="Duration"
        value={value}
        error={error}
        hint="Seconds or hh:mm:ss.mmm. Media clips stop at the end of their source."
        onChange={(next) => {
          setValue(next);
          setError(null);
        }}
      />
    </TimelineDialog>
  );
}

/** Edits one clip's duration from its right edge; mounted while `itemId` is set. */
export function SetDurationDialog({ itemId, onClose }: { readonly itemId: string | null; readonly onClose: () => void }) {
  return itemId ? <SetDurationForm key={itemId} itemId={itemId} onClose={onClose} /> : null;
}
