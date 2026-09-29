import { useMemo } from "react";
import { roundTimelineSeconds } from "@/lib/format";
import type { ProjectAction, VideoProject } from "@/lib/project";
import {
  decomposeNested,
  defaultSplitItemId,
  deleteGapAt,
  deleteItems,
  duplicateItems,
  linkItems,
  nudgeItems,
  rippleDeleteItems,
  setItemDuration,
  setItemReverse,
  setItemSpeed,
  splitAtPlayhead,
  unlinkItems,
  type CommandResult,
} from "@/lib/timeline-ops/clip-commands";
import { planAssetInsert, playheadAssetPlacement, type AssetPlacement, type AssetRef } from "@/lib/timeline-ops/asset-insert";
import { clipMoveGroup, evaluateClipMove, planClipMoveCommit } from "@/lib/timeline-ops/clip-drag";
import { copyItems, pastePlan } from "@/lib/timeline-ops/clipboard";
import { detachAudioIds, planDetachAudio } from "@/lib/timeline-ops/detach-audio";
import { planAdjacentTrack } from "@/lib/timeline-ops/dynamic-tracks";
import {
  planCreateTimeline,
  planDeleteTimeline,
  planRenameTimeline,
  planSetActiveTimeline,
  projectTimelineEntries,
} from "@/lib/timeline-ops/multi-timeline";
import { orderedTracksByBand } from "@/lib/timeline-ops/track-bands";
import { trimItemsToPlayhead } from "@/lib/timeline-ops/trim-to-playhead";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";

/**
 * Store-bound timeline commands. Each resolves `true` when its edit was applied (or was a
 * no-op) and `false` when it was blocked or failed; the reason is in `project.lastError`.
 * Every applied command is exactly one `applyActions` call, so one undo step.
 */
export interface TimelineCommands {
  splitAtPlayhead(): Promise<boolean>;
  /** Splits one clip at `seconds` (the blade tool) and selects the right half. */
  splitItemAt(itemId: string, seconds: number): Promise<boolean>;
  /**
   * Applies a planned interaction batch (drag, trim, keyframe edit) as one undo step, then
   * keeps the selection or selects the items the batch added.
   */
  applyPlan(result: CommandResult, selectionAfter?: "keep" | "added"): Promise<boolean>;
  deleteSelection(): Promise<boolean>;
  rippleDeleteSelection(): Promise<boolean>;
  /** Closes the gap on `trackId` under `seconds` (defaults to the playhead). */
  deleteGap(trackId: string, seconds?: number): Promise<boolean>;
  nudgeSelection(frames: number): Promise<boolean>;
  setDuration(itemId: string, durationSeconds: number): Promise<boolean>;
  setSpeed(itemId: string, speed: number): Promise<boolean>;
  /** Plays a clip and its linked partners reversed or forwards again. */
  setReverse(itemId: string, reverse: boolean): Promise<boolean>;
  linkSelection(): Promise<boolean>;
  unlinkSelection(): Promise<boolean>;
  /** Moves a video clip's sound onto a new linked audio clip and selects that clip. */
  detachAudio(itemId: string): Promise<boolean>;
  decomposeSelection(): Promise<boolean>;
  duplicateSelection(): Promise<boolean>;
  copySelection(): Promise<boolean>;
  cutSelection(): Promise<boolean>;
  /** Pastes over existing clips at `seconds` (defaults to the playhead). */
  paste(seconds?: number): Promise<boolean>;
  /** Pastes and ripples later clips at `seconds` (defaults to the playhead). */
  pasteInsert(seconds?: number): Promise<boolean>;
  createTimeline(duplicate: boolean): Promise<boolean>;
  renameTimeline(timelineId: string, name: string): Promise<boolean>;
  deleteTimeline(timelineId: string): Promise<boolean>;
  switchTimeline(timelineId: string): Promise<boolean>;
  /** Adds an empty track of the same kind directly above or below `trackId`. */
  addTrack(trackId: string, placement: "above" | "below"): Promise<boolean>;
  /** Places a panel asset (one batch, creating a track when needed) and selects the new clip. */
  insertAsset(asset: AssetRef, placement: AssetPlacement): Promise<boolean>;
  /** Places a panel asset at the playhead on the first track that accepts it (the `+` buttons). */
  insertAssetAtPlayhead(asset: AssetRef): Promise<boolean>;
  /** Trims the start or end of the selected clips under the playhead to the playhead. */
  trimToPlayhead(edge: "start" | "end"): Promise<boolean>;
  /** Moves the selection to the track above (-1) or below (1), keeping its time. */
  moveSelectionToTrack(direction: -1 | 1): Promise<boolean>;
}

type SelectionAfter = "keep" | "clear" | ((actions: readonly ProjectAction[]) => string[]);

const fallbackFps = 30;

function activeTimelineId(project: VideoProject): string {
  return project.activeTimelineId ?? projectTimelineEntries(project)[0]?.id ?? "main";
}

/** The playhead on the nearest frame boundary at the project frame rate (3 dp). */
export function frameSnappedSeconds(project: VideoProject, seconds: number): number {
  const fps = project.renderSettings.fps > 0 ? project.renderSettings.fps : fallbackFps;
  return roundTimelineSeconds(Math.round(seconds * fps) / fps);
}

function splitRightHalfIds(actions: readonly ProjectAction[]): string[] {
  return actions.flatMap((action) =>
    action.type === "splitItems" ? action.splits.map((split) => split.newItemId) : [],
  );
}

function addedItemIds(actions: readonly ProjectAction[]): string[] {
  return actions.flatMap((action) =>
    action.type === "addItems" || action.type === "insertItems" ? action.items.map((item) => item.id) : [],
  );
}

function newLinkGroupId(): string {
  return `link-${globalThis.crypto.randomUUID()}`;
}

function createTimelineCommands(store: EditorStore): TimelineCommands {
  const state = () => store.getState();

  function block(reason: string): false {
    state().setLastError(reason);
    return false;
  }

  /** Applies a planned batch as one undo step, then updates the selection. */
  async function run(result: CommandResult, selectionAfter: SelectionAfter): Promise<boolean> {
    if ("blocked" in result) return block(result.blocked);
    if (result.actions.length === 0) {
      state().setLastError(null);
      return true;
    }
    const applied = await state().applyActions(result.actions);
    if (!applied) return false;
    if (selectionAfter === "clear") state().clearSelection();
    else if (selectionAfter !== "keep") state().selectItems(selectionAfter(result.actions));
    return true;
  }

  /** Legacy timeline change: remember the playhead, stop playback, clear timeline context. */
  function leaveActiveTimeline() {
    const { project, playheadSeconds } = state();
    state().rememberTimelinePlayhead(activeTimelineId(project), playheadSeconds);
    state().setPlaying(false);
    state().clearTimelineContext();
  }

  /** Seeks to the stored playhead of the now active timeline (or `pending`), clamped. */
  function enterActiveTimeline(pending?: number) {
    const { project, timelinePlayheads } = state();
    state().seek(pending ?? timelinePlayheads[activeTimelineId(project)] ?? 0);
  }

  function selection() {
    const { project, selectedItemIds } = state();
    return { project, itemIds: selectedItemIds };
  }

  async function pasteAt(seconds: number | undefined, mode: "paste" | "insert") {
    const { project, clipboard, playheadSeconds } = state();
    if (!clipboard) return block("Copy a clip first.");
    return run(pastePlan(project, clipboard, seconds ?? playheadSeconds, mode), addedItemIds);
  }

  /**
   * Places a panel asset and selects the new clip. Placing media is a timeline interaction, so an
   * applied insert (`+`, drag or drop) ends asset preview: the viewer returns to the timeline at the
   * unchanged playhead. A blocked insert keeps the previewed asset.
   */
  async function insertPlanned(result: CommandResult): Promise<boolean> {
    if (!(await run(result, addedItemIds))) return false;
    if (state().previewSource.kind === "asset") state().previewTimeline();
    return true;
  }

  return {
    splitItemAt(itemId, seconds) {
      const { project } = state();
      const result = splitAtPlayhead(project, [itemId], seconds, (id) => defaultSplitItemId(id, seconds));
      return run(result, splitRightHalfIds);
    },
    applyPlan(result, selectionAfter = "keep") {
      return run(result, selectionAfter === "added" ? addedItemIds : "keep");
    },
    splitAtPlayhead() {
      const { project, itemIds } = selection();
      const splitSeconds = frameSnappedSeconds(project, state().playheadSeconds);
      const result = splitAtPlayhead(project, itemIds, splitSeconds, (itemId) =>
        defaultSplitItemId(itemId, splitSeconds),
      );
      return run(result, splitRightHalfIds);
    },
    deleteSelection() {
      const { project, itemIds } = selection();
      return run(deleteItems(project, itemIds), "clear");
    },
    rippleDeleteSelection() {
      const { project, itemIds } = selection();
      return run(rippleDeleteItems(project, itemIds), "clear");
    },
    deleteGap(trackId, seconds) {
      const { project, playheadSeconds } = state();
      return run(deleteGapAt(project, trackId, seconds ?? playheadSeconds), "keep");
    },
    nudgeSelection(frames) {
      const { project, itemIds } = selection();
      return run(nudgeItems(project, itemIds, frames), "keep");
    },
    setDuration(itemId, durationSeconds) {
      return run(setItemDuration(state().project, itemId, durationSeconds), "keep");
    },
    setSpeed(itemId, speed) {
      return run(setItemSpeed(state().project, itemId, speed), "keep");
    },
    setReverse(itemId, reverse) {
      return run(setItemReverse(state().project, itemId, reverse), "keep");
    },
    linkSelection() {
      const { project, itemIds } = selection();
      return run(linkItems(project, itemIds, newLinkGroupId()), "keep");
    },
    unlinkSelection() {
      const { project, itemIds } = selection();
      return run(unlinkItems(project, itemIds), "keep");
    },
    detachAudio(itemId) {
      const { project } = state();
      const ids = detachAudioIds(project, itemId, newLinkGroupId());
      return run(planDetachAudio(project, itemId, ids), () => [ids.audioItemId]);
    },
    async decomposeSelection() {
      const { project, itemIds } = selection();
      const [itemId] = itemIds;
      if (itemIds.length !== 1 || itemId === undefined) {
        return block("Select one nested timeline sequence to decompose.");
      }
      return run(decomposeNested(project, itemId), "clear");
    },
    duplicateSelection() {
      const { project, itemIds } = selection();
      return run(duplicateItems(project, itemIds), addedItemIds);
    },
    async copySelection() {
      const { project, itemIds } = selection();
      const clipboard = copyItems(project.timeline, itemIds);
      if (!clipboard) return block("Select a clip to copy.");
      state().setClipboard(clipboard);
      state().setLastError(null);
      return true;
    },
    async cutSelection() {
      const { project, itemIds } = selection();
      const clipboard = copyItems(project.timeline, itemIds);
      if (!clipboard) return block("Select a clip to cut.");
      const removal = deleteItems(project, itemIds);
      if ("blocked" in removal) return block(removal.blocked);
      state().setClipboard(clipboard);
      return run(removal, "clear");
    },
    paste(seconds) {
      return pasteAt(seconds, "paste");
    },
    pasteInsert(seconds) {
      return pasteAt(seconds, "insert");
    },
    async createTimeline(duplicate) {
      const result = planCreateTimeline(state().project, { duplicate });
      if ("blocked" in result) return block(result.blocked);
      leaveActiveTimeline();
      if (!(await run(result, "keep"))) return false;
      enterActiveTimeline(0);
      return true;
    },
    renameTimeline(timelineId, name) {
      return run(planRenameTimeline(state().project, timelineId, name), "keep");
    },
    async deleteTimeline(timelineId) {
      const result = planDeleteTimeline(state().project, timelineId);
      if ("blocked" in result) return block(result.blocked);
      const previousActiveId = activeTimelineId(state().project);
      leaveActiveTimeline();
      if (!(await run(result, "keep"))) return false;
      state().forgetTimelinePlayhead(timelineId);
      if (activeTimelineId(state().project) !== previousActiveId) enterActiveTimeline();
      return true;
    },
    async switchTimeline(timelineId) {
      const result = planSetActiveTimeline(state().project, timelineId);
      if ("blocked" in result) return block(result.blocked);
      if (result.actions.length === 0) {
        state().setLastError(null);
        return true;
      }
      leaveActiveTimeline();
      if (!(await run(result, "keep"))) return false;
      enterActiveTimeline();
      return true;
    },
    addTrack(trackId, placement) {
      return run(planAdjacentTrack(state().project.timeline, trackId, placement), "keep");
    },
    insertAsset(asset, placement) {
      return insertPlanned(planAssetInsert(state().project, asset, placement, Date.now().toString(36)));
    },
    insertAssetAtPlayhead(asset) {
      const { project, playheadSeconds } = state();
      return insertPlanned(planAssetInsert(project, asset, playheadAssetPlacement(project, asset, playheadSeconds), Date.now().toString(36)));
    },
    trimToPlayhead(edge) {
      const { project, itemIds } = selection();
      return run(trimItemsToPlayhead(project, itemIds, state().playheadSeconds, edge), "keep");
    },
    async moveSelectionToTrack(direction) {
      const { project, itemIds } = selection();
      const { timeline } = project;
      const leadItemId = itemIds[0];
      const rows = orderedTracksByBand(timeline);
      const index = rows.findIndex((track) => track.items.some((item) => item.id === leadItemId));
      if (leadItemId === undefined || index < 0) return block("Select a clip to move.");
      const target = rows[index + direction];
      if (!target) return block(direction < 0 ? "There is no track above." : "There is no track below.");
      const group = clipMoveGroup(timeline, itemIds, leadItemId);
      if (!group) return block("Unlock the selected tracks to move these clips.");
      const preview = evaluateClipMove({
        timeline,
        leadItemId,
        itemIds: group,
        deltaSeconds: 0,
        target: { trackId: target.id, insertIndex: null },
        duplicate: false,
        snap: null,
      });
      return run(planClipMoveCommit(project, preview, false), "keep");
    },
  };
}

/** Timeline commands bound to the editor store; stable for the lifetime of the store. */
export function useTimelineCommands(): TimelineCommands {
  const store = useEditorStoreApi();
  return useMemo(() => createTimelineCommands(store), [store]);
}
