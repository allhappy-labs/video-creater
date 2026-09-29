import { Check, Droplets, Focus, Pipette, Sparkles, WandSparkles, type LucideIcon } from "lucide-react";
import type { VisualEffectDescriptor } from "@/lib/project";
import { writeEffectDragData } from "../../timeline/effect-drag-data";
import { TileAddButton } from "./tile-add-button";

const iconsByFamily: Readonly<Record<string, LucideIcon>> = {
  blur: Droplets,
  detail: Focus,
  key: Pipette,
  stylize: Sparkles,
};

function effectIcon(effect: VisualEffectDescriptor): LucideIcon {
  return iconsByFamily[effect.id.split(".")[0] ?? ""] ?? WandSparkles;
}

interface EffectTileProps {
  readonly effect: VisualEffectDescriptor;
  readonly applied: boolean;
  /** Why `+` can't apply to the current selection, or null when it can. */
  readonly blockedReason: string | null;
  onApply(effect: VisualEffectDescriptor): void;
}

/** A catalog effect: preview glyph, name, category and `+`; drag it onto a clip to apply it there. */
export function EffectTile({ effect, applied, blockedReason, onApply }: EffectTileProps) {
  const Icon = effectIcon(effect);
  const reason = applied ? "Already applied to this clip" : blockedReason;
  return (
    <li
      draggable
      onDragStart={(event) => writeEffectDragData(event.dataTransfer, effect)}
      className="group relative flex min-w-0 cursor-grab flex-col gap-1.5 rounded-control bg-raised p-1.5 transition-colors hover:bg-hover active:cursor-grabbing motion-reduce:transition-none"
    >
      <div aria-hidden className="grid aspect-video place-items-center rounded-md bg-panel text-muted-foreground">
        <Icon className="h-5 w-5" />
      </div>
      {applied && (
        <span className="absolute left-2.5 top-2.5 flex items-center gap-1 rounded-full bg-accent-soft px-1.5 py-0.5 text-[10px] font-medium text-foreground">
          <Check className="h-3 w-3" aria-hidden />
          Applied
        </span>
      )}
      <div className="min-w-0 px-0.5">
        <p className="truncate text-[12px] font-medium text-foreground" title={effect.displayName}>
          {effect.displayName}
        </p>
        <p className="truncate text-[11px] text-dim">{effect.category}</p>
      </div>
      <TileAddButton
        label={applied ? `${effect.displayName} applied` : `Apply ${effect.displayName}`}
        reason={reason}
        onClick={() => onApply(effect)}
      />
    </li>
  );
}
