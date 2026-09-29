import { useMemo } from "react";
import { formatTimecode } from "@/lib/format";
import type { TransitionKind } from "@/lib/timeline";
import { noCutReason, transitionAddTarget, transitionKinds } from "@/lib/timeline-ops/transition-commands";
import { trackDisplayNames } from "@/lib/timeline-ops/track-bands";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { useTransitionCommands } from "../../timeline/transition-commands";
import { TransitionTile } from "./transition-tile";

/**
 * Transition tiles. `+` adds to the selected transition's cut, or the cut nearest the playhead on
 * the selected clip's track (any track without a selection); dragging a tile onto a cut adds it there.
 */
export function TransitionsView() {
  const store = useEditorStoreApi();
  const commands = useTransitionCommands();
  const project = useEditorStore((state) => state.project);
  const selectedTransitionId = useEditorStore((state) => state.selectedTransitionId);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const selectedTrackId = useEditorStore((state) => state.selectedTrackId);
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const target = useMemo(
    () => transitionAddTarget(project, { selectedTransitionId, selectedItemIds, selectedTrackId, playheadSeconds }),
    [project, selectedTransitionId, selectedItemIds, selectedTrackId, playheadSeconds],
  );
  const trackName = target ? (trackDisplayNames(project.timeline).get(target.trackId) ?? "") : "";

  async function add(kind: TransitionKind) {
    if (!(await commands.addAtTarget(kind))) return;
    // On phones the sheet covers the timeline: close it so the new badge and its tools show.
    if (store.getState().openSheetId === "effects") store.getState().closeSheet();
  }

  return (
    <div className="flex flex-col gap-2.5">
      <p className="text-[12px] text-muted-foreground">
        {target
          ? `${target.transitionId ? "+ changes the transition" : "+ adds at the cut"} at ${formatTimecode(target.seconds)} on ${trackName}. Or drag a tile onto a cut.`
          : `${noCutReason}, then drop a tile on the cut.`}
      </p>
      <ul aria-label="Transitions" className="grid grid-cols-2 gap-2">
        {transitionKinds.map((kind) => (
          <TransitionTile key={kind} kind={kind} blockedReason={target ? null : noCutReason} onAdd={(picked) => void add(picked)} />
        ))}
      </ul>
    </div>
  );
}
