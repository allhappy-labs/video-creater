import { Diamond } from "lucide-react";
import { IconButton } from "@/components/ui/icon-button";
import { cn } from "@/lib/utils";

/** The ◇ state a Properties control shows for its animatable property. */
export interface KeyframeToggle {
  /** A keyframe sits at the playhead; clicking removes it. */
  readonly active: boolean;
  /** The property has a keyframe lane, so value edits upsert at the playhead. */
  readonly keyframed?: boolean;
  readonly disabled?: boolean;
  onToggle(): void;
}

/** ◇ toggle: filled at a keyframe, outlined in the keyframe color when the property is animated. */
export function KeyframeButton({ label, active, keyframed = false, disabled = false, onToggle }: KeyframeToggle & { readonly label: string }) {
  const name = `${active ? "Remove" : "Add"} keyframe for ${label}`;
  return (
    <IconButton
      label={name}
      size="sm"
      aria-pressed={active}
      disabled={disabled}
      onClick={onToggle}
      className={cn((active || keyframed) && "text-keyframe hover:text-keyframe")}
    >
      <Diamond className={cn("h-3.5 w-3.5", active && "fill-current")} aria-hidden />
    </IconButton>
  );
}
