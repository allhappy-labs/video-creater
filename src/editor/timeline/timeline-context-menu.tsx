import { useRef, useState, type MouseEvent, type ReactElement } from "react";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from "@/components/ui/context-menu";
import { Tooltip } from "@/components/ui/tooltip";
import { roundTimelineSeconds } from "@/lib/format";
import type { VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { decomposeNested, deleteGapAt, setItemDuration, setItemReverse, setItemSpeed } from "@/lib/timeline-ops/clip-commands";
import { pastePlan } from "@/lib/timeline-ops/clipboard";
import { detachAudioIds, planDetachAudio } from "@/lib/timeline-ops/detach-audio";
import { numberProperty } from "@/lib/timeline-ops/item-properties";
import { isReversedItem, isReversibleItem } from "@/lib/timeline-ops/reverse";
import { trackEnabled } from "@/lib/timeline-ops/navigation";
import { saveRangeTarget } from "@/lib/timeline-ops/save-range";
import { cutAdjacentToItem, locateTransition, planRemoveTransition } from "@/lib/timeline-ops/transition-commands";
import { cutsOnTrack, transitionFrameSeconds } from "@/lib/timeline-ops/transitions";
import { cn } from "@/lib/utils";
import { useExportService } from "../services/export-service";
import { shortcutHint, useShortcutPlatform } from "../shell/use-shortcut-platform";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { SetDurationDialog } from "./set-duration-dialog";
import { SpeedDialog } from "./speed-dialog";
import { blockedReason, usePlayheadAvailability, useSelectionAvailability } from "./timeline-availability";
import { useTimelineCommands } from "./timeline-commands";
import { useTransitionCommands } from "./transition-commands";
import { TransitionSubmenu } from "./transition-menu";

type MenuTarget =
  | { readonly kind: "clip"; readonly itemId: string; readonly seconds: number }
  | { readonly kind: "lane"; readonly trackId: string; readonly seconds: number }
  | { readonly kind: "transition"; readonly transitionId: string };

interface MenuActionProps {
  readonly label: string;
  readonly onSelect?: () => void;
  readonly shortcutId?: string;
  /** Planner copy; the item is disabled and shows it in a tooltip. */
  readonly reason?: string | null;
  readonly destructive?: boolean;
}

function MenuAction({ label, onSelect, shortcutId, reason = null, destructive = false }: MenuActionProps) {
  const platform = useShortcutPlatform();
  const item = (
    <ContextMenuItem disabled={reason !== null} onSelect={() => onSelect?.()} className={cn(destructive && "text-destructive data-[disabled]:text-dim")}>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {shortcutId && <span className="tabular-time shrink-0 pl-4 text-[11px] text-dim">{shortcutHint(shortcutId, platform)}</span>}
    </ContextMenuItem>
  );
  return reason === null ? (
    item
  ) : (
    <Tooltip content={reason} side="right">
      {item}
    </Tooltip>
  );
}

function locate(project: VideoProject, itemId: string): { item: TimelineItem; track: TimelineTrack } | null {
  for (const track of project.timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return { item, track };
  }
  return null;
}

const replaceableKinds: ReadonlySet<TimelineItem["kind"]> = new Set(["video_clip", "image_clip", "lottie_clip", "generated_clip", "audio_clip"]);

/** A selected clip saves its span; otherwise the I/O range when both marks are set. */
function SaveRangeAction() {
  const exportService = useExportService();
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const rangeIn = useEditorStore((state) => state.rangeIn);
  const rangeOut = useEditorStore((state) => state.rangeOut);
  const target = saveRangeTarget(project, { selectedItemIds, rangeIn, rangeOut });
  return (
    <MenuAction
      label="Save range as media"
      reason={"blocked" in target ? target.blocked : null}
      onSelect={() => {
        if ("range" in target) void exportService.saveRangeAsMedia(target.range);
      }}
    />
  );
}

function TrackActions({ track }: { readonly track: TimelineTrack }) {
  const applyActions = useEditorStore((state) => state.applyActions);
  const enabled = trackEnabled(track);
  const audio = track.kind === "audio";
  return (
    <>
      <MenuAction
        label={track.locked ? "Unlock track" : "Lock track"}
        onSelect={() => void applyActions([{ type: "setTrackLocked", trackId: track.id, locked: !track.locked }])}
      />
      <MenuAction
        label={audio ? (enabled ? "Mute track" : "Unmute track") : enabled ? "Hide track" : "Show track"}
        onSelect={() => void applyActions([{ type: "setTrackEnabled", trackId: track.id, enabled: !enabled }])}
      />
    </>
  );
}

interface ClipMenuProps {
  readonly itemId: string;
  /** Where the clip was right-clicked: Add transition picks the nearer cut at the clip's edges. */
  readonly seconds: number;
  readonly onSetDuration: (itemId: string) => void;
  readonly onSetSpeed: (itemId: string) => void;
  /** Ask AI moved focus to the AI composer, so closing the menu must not return focus to the clip. */
  readonly onHandoff: () => void;
}

function ClipMenuItems({ itemId, seconds, onSetDuration, onSetSpeed, onHandoff }: ClipMenuProps) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const selection = useSelectionAvailability();
  const playhead = usePlayheadAvailability();
  const location = locate(project, itemId);
  if (!location) return null;
  const { item, track } = location;
  const nested = item.source.type === "timeline";
  const mediaId = item.source.type === "media" ? item.source.mediaId : null;
  const unlink = selection.linkMode === "unlink";
  const reversed = isReversedItem(item);
  const replaceReason = !replaceableKinds.has(item.kind)
    ? "Only video, image and audio clips can be replaced."
    : track.locked
      ? "Unlock the track to replace this clip."
      : null;

  function openTab(tab: "media") {
    store.getState().setActiveTab(tab);
  }

  return (
    <>
      <MenuAction label="Cut" shortcutId="timeline.cut" reason={selection.cut} onSelect={() => void commands.cutSelection()} />
      <MenuAction label="Copy" shortcutId="timeline.copy" reason={selection.copy} onSelect={() => void commands.copySelection()} />
      <MenuAction label="Paste" shortcutId="timeline.paste" reason={playhead.paste} onSelect={() => void commands.paste()} />
      <MenuAction label="Paste insert" shortcutId="timeline.pasteInsert" reason={playhead.pasteInsert} onSelect={() => void commands.pasteInsert()} />
      <MenuAction label="Duplicate" shortcutId="timeline.duplicate" reason={selection.duplicate} onSelect={() => void commands.duplicateSelection()} />
      <ContextMenuSeparator />
      <MenuAction label="Delete" shortcutId="timeline.delete" destructive reason={selection.delete} onSelect={() => void commands.deleteSelection()} />
      <MenuAction
        label="Ripple delete"
        shortcutId="timeline.rippleDelete"
        destructive
        reason={selection.rippleDelete}
        onSelect={() => void commands.rippleDeleteSelection()}
      />
      <ContextMenuSeparator />
      <MenuAction label="Split at playhead" shortcutId="timeline.split" reason={playhead.split} onSelect={() => void commands.splitAtPlayhead()} />
      <MenuAction
        label="Set duration…"
        reason={blockedReason(setItemDuration(project, item.id, item.durationSeconds))}
        onSelect={() => onSetDuration(item.id)}
      />
      <MenuAction
        label="Speed…"
        reason={blockedReason(setItemSpeed(project, item.id, numberProperty(item, "speed") ?? 1))}
        onSelect={() => onSetSpeed(item.id)}
      />
      {isReversibleItem(project, item) && (
        <MenuAction
          label={reversed ? "Play forward" : "Reverse"}
          reason={blockedReason(setItemReverse(project, item.id, !reversed))}
          onSelect={() => void commands.setReverse(item.id, !reversed)}
        />
      )}
      <MenuAction
        label={unlink ? "Unlink" : "Link"}
        reason={selection.link}
        onSelect={() => void (unlink ? commands.unlinkSelection() : commands.linkSelection())}
      />
      {item.kind === "video_clip" && (
        <MenuAction
          label="Detach audio"
          reason={blockedReason(planDetachAudio(project, item.id, detachAudioIds(project, item.id, "link-detach")))}
          onSelect={() => void commands.detachAudio(item.id)}
        />
      )}
      {nested && (
        <MenuAction
          label="Decompose nested sequence"
          reason={selectedItemIds.length === 1 ? blockedReason(decomposeNested(project, item.id)) : "Select one nested timeline sequence to decompose."}
          onSelect={() => void commands.decomposeSelection()}
        />
      )}
      <ContextMenuSeparator />
      <MenuAction
        label="Replace with media…"
        reason={replaceReason}
        onSelect={() => {
          store.getState().setReplaceTargetItemId(item.id);
          openTab("media");
        }}
      />
      <MenuAction
        label="Reveal in Media"
        reason={mediaId && project.media.some((media) => media.id === mediaId) ? null : "This clip has no media in the project."}
        onSelect={() => {
          store.getState().setRevealMediaId(mediaId);
          openTab("media");
        }}
      />
      <SaveRangeAction />
      <ContextMenuSeparator />
      <TransitionSubmenu cut={cutAdjacentToItem(project, item.id, seconds)} />
      <ContextMenuSeparator />
      <MenuAction
        label="Ask AI about this clip"
        onSelect={() => {
          onHandoff();
          store.getState().requestAgent({ itemIds: selectedItemIds.includes(item.id) ? selectedItemIds : [item.id] });
        }}
      />
      <ContextMenuSeparator />
      <TrackActions track={track} />
    </>
  );
}

function TransitionMenuItems({ transitionId }: { readonly transitionId: string }) {
  const commands = useTransitionCommands();
  const project = useEditorStore((state) => state.project);
  const located = locateTransition(project.timeline, transitionId);
  if (!located) return null;
  const cut = cutsOnTrack(located.track, transitionFrameSeconds(project)).find((candidate) => candidate.transitionId === transitionId) ?? null;
  return (
    <>
      <TransitionSubmenu cut={cut} />
      <MenuAction
        label="Delete transition"
        shortcutId="timeline.delete"
        destructive
        reason={blockedReason(planRemoveTransition(project, transitionId))}
        onSelect={() => void commands.remove(transitionId)}
      />
      <ContextMenuSeparator />
      <TrackActions track={located.track} />
    </>
  );
}

function LaneMenuItems({ trackId, seconds }: { readonly trackId: string; readonly seconds: number }) {
  const commands = useTimelineCommands();
  const project = useEditorStore((state) => state.project);
  const clipboard = useEditorStore((state) => state.clipboard);
  const track = project.timeline.tracks.find((candidate) => candidate.id === trackId);
  if (!track) return null;
  return (
    <>
      <MenuAction
        label="Paste"
        shortcutId="timeline.paste"
        reason={clipboard ? blockedReason(pastePlan(project, clipboard, seconds, "paste")) : "Copy a clip first."}
        onSelect={() => void commands.paste(seconds)}
      />
      <MenuAction label="Delete gap" reason={blockedReason(deleteGapAt(project, trackId, seconds))} onSelect={() => void commands.deleteGap(trackId, seconds)} />
      <ContextMenuSeparator />
      <SaveRangeAction />
      <ContextMenuSeparator />
      <MenuAction label="Add track above" onSelect={() => void commands.addTrack(trackId, "above")} />
      <MenuAction label="Add track below" onSelect={() => void commands.addTrack(trackId, "below")} />
    </>
  );
}

interface TimelineContextMenuProps {
  readonly pixelsPerSecond: number;
  /** The tracks area; right-clicks on clips open the clip menu and on empty lane space the lane menu. */
  readonly children: ReactElement;
}

/** Clip, transition and lane context menus for the tracks area, plus the dialogs they open. */
export function TimelineContextMenu({ pixelsPerSecond, children }: TimelineContextMenuProps) {
  const store = useEditorStoreApi();
  const [target, setTarget] = useState<MenuTarget | null>(null);
  const [durationItemId, setDurationItemId] = useState<string | null>(null);
  const [speedItemId, setSpeedItemId] = useState<string | null>(null);
  const handedOff = useRef(false);

  function onContextMenu(event: MouseEvent<HTMLElement>) {
    const element = event.target instanceof Element ? event.target : null;
    const option = element?.closest<HTMLElement>('[role="option"][data-item-id]');
    const badge = element?.closest<HTMLElement>("[data-transition-id]");
    const lane = element?.closest<HTMLElement>("[data-track-id]");
    const itemId = option?.dataset.itemId;
    const transitionId = badge?.dataset.transitionId;
    const trackId = lane?.dataset.trackId;
    const seconds = lane ? Math.max(0, roundTimelineSeconds((event.clientX - lane.getBoundingClientRect().left) / pixelsPerSecond)) : 0;
    if (transitionId) {
      store.getState().selectTransition(transitionId);
      setTarget({ kind: "transition", transitionId });
    } else if (itemId) {
      const state = store.getState();
      if (!state.selectedItemIds.includes(itemId)) state.selectItems([itemId]);
      setTarget({ kind: "clip", itemId, seconds });
    } else if (lane && trackId) {
      setTarget({ kind: "lane", trackId, seconds });
    } else {
      // Headers and the keyframe lane have no menu; preventing default also stops Radix opening.
      event.preventDefault();
    }
  }

  return (
    <>
      <ContextMenu onOpenChange={(open) => !open && setTarget(null)}>
        <ContextMenuTrigger asChild onContextMenu={onContextMenu}>
          {children}
        </ContextMenuTrigger>
        <ContextMenuContent
          aria-label={target?.kind === "lane" ? "Track actions" : target?.kind === "transition" ? "Transition actions" : "Clip actions"}
          onCloseAutoFocus={(event) => {
            if (!handedOff.current) return;
            handedOff.current = false;
            event.preventDefault();
          }}
        >
          {target?.kind === "clip" && (
            <ClipMenuItems
              itemId={target.itemId}
              seconds={target.seconds}
              onSetDuration={setDurationItemId}
              onSetSpeed={setSpeedItemId}
              onHandoff={() => {
                handedOff.current = true;
              }}
            />
          )}
          {target?.kind === "lane" && <LaneMenuItems trackId={target.trackId} seconds={target.seconds} />}
          {target?.kind === "transition" && <TransitionMenuItems transitionId={target.transitionId} />}
        </ContextMenuContent>
      </ContextMenu>
      <SetDurationDialog itemId={durationItemId} onClose={() => setDurationItemId(null)} />
      <SpeedDialog itemId={speedItemId} onClose={() => setSpeedItemId(null)} />
    </>
  );
}
