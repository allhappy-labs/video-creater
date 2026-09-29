import { useState } from "react";
import { setItemSpeed } from "@/lib/timeline-ops/clip-commands";
import { numberProperty } from "@/lib/timeline-ops/item-properties";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { blockedReason } from "./timeline-availability";
import { useTimelineCommands } from "./timeline-commands";
import { DialogField, TimelineDialog } from "./timeline-dialog";

function SpeedForm({ itemId, onClose }: { readonly itemId: string; readonly onClose: () => void }) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const item = useEditorStore((state) => state.project.timeline.tracks.flatMap((track) => track.items).find((entry) => entry.id === itemId) ?? null);
  const [value, setValue] = useState(() => {
    const speed = item ? numberProperty(item, "speed") : null;
    return (speed !== null && speed > 0 ? speed : 1).toString();
  });
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    const trimmed = value.trim().replace(/x$/i, "");
    const speed = trimmed.length > 0 ? Number(trimmed) : Number.NaN;
    const reason = blockedReason(setItemSpeed(store.getState().project, itemId, speed));
    if (reason !== null) {
      setError(reason);
      return;
    }
    if (await commands.setSpeed(itemId, speed)) onClose();
    else setError(store.getState().lastError ?? "The speed could not be changed.");
  }

  return (
    <TimelineDialog
      open
      onOpenChange={(open) => !open && onClose()}
      title="Speed"
      description={item?.label ?? "Clip"}
      submitLabel="Set speed"
      onSubmit={() => void submit()}
    >
      <DialogField
        label="Playback speed"
        value={value}
        error={error}
        hint="0.1x to 8x. The clip length changes to keep the same source range."
        suffix="x"
        inputMode="decimal"
        onChange={(next) => {
          setValue(next);
          setError(null);
        }}
      />
    </TimelineDialog>
  );
}

/** Edits one visual or audio clip's playback speed (and its linked partners'); mounted while `itemId` is set. */
export function SpeedDialog({ itemId, onClose }: { readonly itemId: string | null; readonly onClose: () => void }) {
  return itemId ? <SpeedForm key={itemId} itemId={itemId} onClose={onClose} /> : null;
}
