import { Check } from "lucide-react";
import { ContextMenuItem, ContextMenuSub, ContextMenuSubContent, ContextMenuSubTrigger } from "@/components/ui/context-menu";
import { Tooltip } from "@/components/ui/tooltip";
import type { TransitionKind } from "@/lib/timeline";
import { locateTransition, noCutReason, transitionKindLabels, transitionKinds } from "@/lib/timeline-ops/transition-commands";
import type { TransitionCut } from "@/lib/timeline-ops/transitions";
import { useEditorStore } from "../store/editor-store-context";
import { transitionKindIcons } from "./transition-badge";
import { useTransitionCommands } from "./transition-commands";

/**
 * "Add transition ▸ Crossfade / Dip to black / Dip to white / Wipe" for `cut`. A cut that already
 * has a transition reads "Change transition" and checks its type. Without a cut the entry is
 * disabled and explains why.
 */
export function TransitionSubmenu({ cut }: { readonly cut: TransitionCut | null }) {
  const commands = useTransitionCommands();
  const currentKind = useEditorStore((state) =>
    cut?.transitionId ? (locateTransition(state.project.timeline, cut.transitionId)?.transition.kind ?? null) : null,
  );
  const label = cut?.transitionId ? "Change transition" : "Add transition";
  if (!cut) {
    return (
      <Tooltip content={noCutReason} side="right">
        <ContextMenuItem disabled>
          <span className="min-w-0 flex-1 truncate">{label}</span>
        </ContextMenuItem>
      </Tooltip>
    );
  }
  return (
    <ContextMenuSub>
      <ContextMenuSubTrigger>
        <span className="min-w-0 flex-1 truncate">{label}</span>
      </ContextMenuSubTrigger>
      <ContextMenuSubContent>
        {transitionKinds.map((kind: TransitionKind) => {
          const Icon = transitionKindIcons[kind];
          const current = kind === currentKind;
          return (
            <ContextMenuItem key={kind} onSelect={() => void commands.addAtCut(cut, kind)}>
              <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
              <span className="min-w-0 flex-1 truncate">{transitionKindLabels[kind]}</span>
              {current && (
                <>
                  <Check className="h-3.5 w-3.5 shrink-0" aria-hidden />
                  <span className="sr-only">(current)</span>
                </>
              )}
            </ContextMenuItem>
          );
        })}
      </ContextMenuSubContent>
    </ContextMenuSub>
  );
}
