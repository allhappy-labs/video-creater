import { Plus } from "lucide-react";
import { Tooltip } from "@/components/ui/tooltip";

interface TileAddButtonProps {
  readonly label: string;
  /** Why the tile can't be added; the button stays focusable with `aria-disabled` and explains it. */
  readonly reason: string | null;
  onClick(): void;
}

/**
 * The tile's `+`: shown on hover or keyboard focus, and always on touch devices (`hover: none`).
 * A blocked button stays visible while its tile is hovered so the tooltip can explain why.
 */
export function TileAddButton({ label, reason, onClick }: TileAddButtonProps) {
  return (
    <Tooltip content={reason ?? label} side="top">
      <button
        type="button"
        aria-label={label}
        aria-disabled={reason !== null || undefined}
        onClick={() => {
          if (reason === null) onClick();
        }}
        className="absolute right-1.5 top-1.5 grid h-7 w-7 place-items-center rounded-full bg-primary text-primary-foreground opacity-0 shadow-md transition-opacity focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring group-hover:opacity-100 aria-disabled:bg-hover aria-disabled:text-dim motion-reduce:transition-none [@media(hover:none)]:opacity-100"
      >
        <Plus className="h-4 w-4" aria-hidden />
      </button>
    </Tooltip>
  );
}
