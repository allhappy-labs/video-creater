import { useState, type ReactNode } from "react";
import type { CaptionStylePreset } from "@/lib/captions/caption-items";
import { pluralize } from "@/lib/format";
import { cn } from "@/lib/utils";
import { PresetGrid } from "../../properties/controls/preset-grid";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { allCaptionsPresetActions, sharedCaptionPreset } from "./captions-model";

function PresetSample({ label, className }: { readonly label: string; readonly className: string }) {
  return (
    <span className="flex min-w-0 flex-col items-center gap-1">
      <span aria-hidden className={cn("rounded px-1.5 py-0.5 text-[15px] leading-5", className)}>
        Aa
      </span>
      <span className="max-w-full truncate text-[11px] text-muted-foreground">{label}</span>
    </span>
  );
}

const presetOptions: readonly { value: CaptionStylePreset; label: string; preview: ReactNode }[] = [
  { value: "boldReadableLower", label: "Bold lower", preview: <PresetSample label="Bold lower" className="bg-background font-bold text-foreground" /> },
  { value: "kineticFocus", label: "Kinetic focus", preview: <PresetSample label="Kinetic focus" className="font-black text-warning" /> },
  { value: "centeredMinimal", label: "Centered minimal", preview: <PresetSample label="Centered minimal" className="rounded-md bg-panel font-medium text-foreground" /> },
];

/** Styles view: a caption preset applied to every unlocked caption as one undo step. */
export function CaptionStylesView() {
  const store = useEditorStoreApi();
  const project = useEditorStore((state) => state.project);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const captionCount = project.timeline.tracks.filter((track) => !track.locked).reduce((count, track) => count + track.items.filter((item) => item.kind === "caption").length, 0);

  const apply = async (preset: CaptionStylePreset) => {
    const state = store.getState();
    const result = allCaptionsPresetActions(state.project, preset);
    if ("blocked" in result) {
      setError(result.blocked);
      return;
    }
    setError(null);
    setBusy(true);
    const applied = await state.applyActions(result.actions);
    setBusy(false);
    if (!applied) setError(store.getState().lastError ?? "The caption style could not be applied.");
  };

  return (
    <div className="flex flex-col gap-2 p-3">
      <PresetGrid label="Caption style preset" value={sharedCaptionPreset(project)} options={presetOptions} disabled={captionCount === 0 || busy} onChange={(preset) => void apply(preset)} />
      <p className="text-[11px] text-dim">
        {captionCount === 0 ? "Generate captions to style them." : `Applies to all ${pluralize(captionCount, "caption")}.`}
      </p>
      {error && (
        <p role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
