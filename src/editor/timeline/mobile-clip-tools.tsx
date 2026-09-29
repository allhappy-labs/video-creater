import { AudioLines, ChevronLeft, SquareSplitHorizontal, Trash2, type LucideIcon } from "lucide-react";
import { Tooltip } from "@/components/ui/tooltip";
import type { VideoProject } from "@/lib/project";
import { detachAudioIds, planDetachAudio } from "@/lib/timeline-ops/detach-audio";
import { mobilePropertyTools, propertySheetId, usePropertiesSelection } from "../properties/mobile-property-sheets";
import { useEditorStore } from "../store/editor-store-context";
import { blockedReason, usePlayheadAvailability, useSelectionAvailability } from "./timeline-availability";
import { useTimelineCommands } from "./timeline-commands";
import { useTransitionCommands } from "./transition-commands";

interface ClipToolProps {
  readonly label: string;
  readonly icon: LucideIcon;
  /** Why the tool cannot run; the button stays focusable with `aria-disabled` and shows it. */
  readonly reason: string | null;
  readonly onClick: () => void;
}

function ClipTool({ label, icon: Icon, reason, onClick }: ClipToolProps) {
  return (
    <Tooltip content={reason ?? label} side="top">
      <button
        type="button"
        aria-disabled={reason !== null || undefined}
        onClick={onClick}
        className="flex min-w-[60px] flex-1 flex-col items-center gap-1 rounded-control py-1.5 text-[11px] text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:text-dim"
      >
        <Icon className="h-[21px] w-[21px]" aria-hidden />
        {label}
      </button>
    </Tooltip>
  );
}

/** Detach audio for exactly one selected video clip, with the reason it can't run. */
function detachAudioTool(project: VideoProject, selectedItemIds: readonly string[]) {
  const [itemId] = selectedItemIds;
  if (selectedItemIds.length !== 1 || itemId === undefined) return null;
  const isVideo = project.timeline.tracks.some((track) => track.items.some((item) => item.id === itemId && item.kind === "video_clip"));
  if (!isVideo) return null;
  return { itemId, reason: blockedReason(planDetachAudio(project, itemId, detachAudioIds(project, itemId, "link-detach"))) };
}

/**
 * Mobile bottom bar while clips are selected: back, Split, the selection's Properties sheet tools
 * (Speed, Volume, Animation, Effects and Adjust on visual clips), Detach audio for one video clip,
 * Delete, then AI. A selected
 * transition gets back, Type, Duration and Delete.
 */
export function MobileClipTools({ onBack }: { readonly onBack: () => void }) {
  const commands = useTimelineCommands();
  const setLastError = useEditorStore((state) => state.setLastError);
  const openSheet = useEditorStore((state) => state.openSheet);
  const selection = useSelectionAvailability();
  const playhead = usePlayheadAvailability();
  const { selection: propertiesSelection } = usePropertiesSelection();
  const tools = mobilePropertyTools(propertiesSelection);
  const transitionId = useEditorStore((state) => (propertiesSelection?.kind === "transition" ? state.selectedTransitionId : null));
  const transitionCommands = useTransitionCommands();
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const detachReason = detachAudioTool(project, selectedItemIds);

  /** Touch has no hover tooltips, so a blocked tap reports its reason in the timeline feedback line. */
  function run(reason: string | null, action: () => void) {
    if (reason === null) action();
    else setLastError(reason);
  }

  return (
    <div role="toolbar" aria-label="Clip tools" className="flex h-full">
      <button
        type="button"
        aria-label="Back to editor tools"
        onClick={onBack}
        className="flex w-11 shrink-0 flex-col items-center justify-center gap-1 border-r border-line text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <ChevronLeft className="h-5 w-5" aria-hidden />
        <span className="text-[10px] leading-none" aria-hidden>Tools</span>
      </button>
      <div className="flex min-w-0 flex-1 overflow-x-auto [scrollbar-width:none]">
        {transitionId === null && (
          <ClipTool label="Split" icon={SquareSplitHorizontal} reason={playhead.split} onClick={() => run(playhead.split, () => void commands.splitAtPlayhead())} />
        )}
        {tools
          .filter((tool) => tool.id !== "ai")
          .map((tool) => (
            <ClipTool key={tool.id} label={tool.label} icon={tool.icon} reason={null} onClick={() => openSheet(propertySheetId(tool.id))} />
          ))}
        {transitionId === null && detachReason !== null && (
          <ClipTool
            label="Detach audio"
            icon={AudioLines}
            reason={detachReason.reason}
            onClick={() => run(detachReason.reason, () => void commands.detachAudio(detachReason.itemId))}
          />
        )}
        {transitionId === null ? (
          <ClipTool label="Delete" icon={Trash2} reason={selection.delete} onClick={() => run(selection.delete, () => void commands.deleteSelection())} />
        ) : (
          <ClipTool label="Delete" icon={Trash2} reason={null} onClick={() => void transitionCommands.remove(transitionId)} />
        )}
        {tools
          .filter((tool) => tool.id === "ai")
          .map((tool) => (
            <ClipTool key={tool.id} label={tool.label} icon={tool.icon} reason={null} onClick={() => openSheet(propertySheetId(tool.id))} />
          ))}
      </div>
    </div>
  );
}
