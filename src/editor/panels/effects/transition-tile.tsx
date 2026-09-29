import type { TransitionKind } from "@/lib/timeline";
import { transitionKindLabels } from "@/lib/timeline-ops/transition-commands";
import { cn } from "@/lib/utils";
import { transitionKindIcons } from "../../timeline/transition-badge";
import { writeTransitionDragData } from "../../timeline/transition-drag-data";
import { TileAddButton } from "./tile-add-button";

const descriptions: Readonly<Record<TransitionKind, string>> = {
  crossfade: "Blends one clip into the next",
  dipToBlack: "Fades out to black, then in",
  dipToWhite: "Fades out to white, then in",
  wipe: "Reveals the next clip left to right",
};

const shotA = "absolute inset-0 bg-gradient-to-br from-clip-video to-panel";
const shotB = "absolute inset-0 bg-gradient-to-tl from-clip-caption to-panel";

/**
 * A looping preview of shot A giving way to shot B. Under reduced motion the `motion-safe:`
 * animations are dropped and the static styles show the transition's midpoint.
 */
function TransitionPreview({ kind }: { readonly kind: TransitionKind }) {
  const dip = kind === "dipToBlack" || kind === "dipToWhite";
  const Icon = transitionKindIcons[kind];
  return (
    <div aria-hidden data-testid="transition-preview" className="relative aspect-video overflow-hidden rounded-md bg-panel">
      <span className={shotA} />
      {kind === "crossfade" && <span className={cn(shotB, "opacity-50 motion-safe:animate-transition-reveal")} />}
      {kind === "wipe" && <span className={cn(shotB, "[clip-path:inset(0_50%_0_0)] motion-safe:animate-transition-wipe")} />}
      {dip && (
        <>
          <span className={cn(shotB, "opacity-0 motion-safe:animate-transition-dip-cut")} />
          <span
            className={cn(
              "absolute inset-0 opacity-80 motion-safe:animate-transition-dip-color",
              kind === "dipToBlack" ? "bg-background" : "bg-foreground",
            )}
          />
        </>
      )}
      <span className="absolute bottom-1 left-1 grid h-5 w-5 place-items-center rounded-[4px] bg-background/70 text-foreground">
        <Icon className="h-3 w-3" />
      </span>
    </div>
  );
}

interface TransitionTileProps {
  readonly kind: TransitionKind;
  /** Why `+` can't add right now, or null when it can. */
  readonly blockedReason: string | null;
  onAdd(kind: TransitionKind): void;
}

/** A transition: animated preview, name, description and `+`; drag it onto a cut to add it there. */
export function TransitionTile({ kind, blockedReason, onAdd }: TransitionTileProps) {
  const label = transitionKindLabels[kind];
  return (
    <li
      draggable
      onDragStart={(event) => writeTransitionDragData(event.dataTransfer, kind)}
      className="group relative flex min-w-0 cursor-grab flex-col gap-1.5 rounded-control bg-raised p-1.5 transition-colors hover:bg-hover active:cursor-grabbing motion-reduce:transition-none"
    >
      <TransitionPreview kind={kind} />
      <div className="min-w-0 px-0.5">
        <p className="truncate text-[12px] font-medium text-foreground" title={label}>
          {label}
        </p>
        <p className="truncate text-[11px] text-dim" title={descriptions[kind]}>
          {descriptions[kind]}
        </p>
      </div>
      <TileAddButton label={`Add ${label} transition`} reason={blockedReason} onClick={() => onAdd(kind)} />
    </li>
  );
}
