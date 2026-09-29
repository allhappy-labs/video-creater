import { useCallback, type KeyboardEvent } from "react";
import { roundTimelineSeconds } from "@/lib/format";
import { matchShortcut, type ShortcutPlatform } from "@/lib/keymap";
import {
  isEditableKeyboardTarget,
  nextTimelineEditPoint,
  previousTimelineEditPoint,
  timelineEditPointSeconds,
  timelineSnapSeconds,
} from "@/lib/timeline-ops/navigation";
import { planRemoveTransition } from "@/lib/timeline-ops/transition-commands";
import type { EditorStore } from "../store/editor-store";
import { useEditorStoreApi } from "../store/editor-store-context";
import { useTimelineCommands, type TimelineCommands } from "./timeline-commands";

async function removeSelectedTransition(store: EditorStore, transitionId: string) {
  const state = store.getState();
  const result = planRemoveTransition(state.project, transitionId);
  if ("blocked" in result) {
    state.setLastError(result.blocked);
    return;
  }
  if (await state.applyActions(result.actions)) store.getState().selectTransition(null);
}

/** I/O marks closer than this leave only the newest mark (legacy minimum range). */
const minimumRangeSeconds = timelineSnapSeconds;

function markRange(store: EditorStore, edge: "in" | "out") {
  const state = store.getState();
  const seconds = state.playheadSeconds;
  if (edge === "in") {
    state.setRangeIn(seconds);
    if (state.rangeOut !== null && state.rangeOut - seconds < minimumRangeSeconds) state.setRangeOut(null);
  } else {
    state.setRangeOut(seconds);
    if (state.rangeIn !== null && seconds - state.rangeIn < minimumRangeSeconds) state.setRangeIn(null);
  }
}

/**
 * Delete and ripple delete remove a selected transition (never rippling), else close a selected gap,
 * else act on the selected clips.
 */
export async function deleteGapOrSelection(store: EditorStore, commands: TimelineCommands, ripple: boolean) {
  const { selectedGap, selectedTransitionId } = store.getState();
  if (selectedTransitionId !== null) {
    await removeSelectedTransition(store, selectedTransitionId);
    return;
  }
  if (!selectedGap) {
    await (ripple ? commands.rippleDeleteSelection() : commands.deleteSelection());
    return;
  }
  if (await commands.deleteGap(selectedGap.trackId, (selectedGap.startSeconds + selectedGap.endSeconds) / 2)) {
    store.getState().selectGap(null);
  }
}

/** Selects the clips starting at or after the lead selected clip, on its track or on all tracks. */
export function selectForward(store: EditorStore, allTracks: boolean) {
  const { project, selectedItemIds, selectItems, setLastError } = store.getState();
  const tracks = project.timeline.tracks;
  const leadTrack = tracks.find((track) => track.items.some((item) => item.id === selectedItemIds[0]));
  const lead = leadTrack?.items.find((item) => item.id === selectedItemIds[0]);
  if (!leadTrack || !lead) {
    setLastError("Select a clip to select forward from.");
    return;
  }
  const scope = allTracks ? tracks : [leadTrack];
  selectItems(scope.flatMap((track) => track.items.filter((item) => item.startSeconds >= lead.startSeconds).map((item) => item.id)));
}

function seekToEditPoint(store: EditorStore, direction: "previous" | "next") {
  const { project, playheadSeconds, seek } = store.getState();
  const points = timelineEditPointSeconds(project.timeline);
  const point = direction === "previous" ? previousTimelineEditPoint(points, playheadSeconds) : nextTimelineEditPoint(points, playheadSeconds);
  if (point !== null) seek(point);
}

type ShortcutRunner = (store: EditorStore, commands: TimelineCommands) => void;

const runners: Readonly<Record<string, ShortcutRunner>> = {
  "timeline.selectTool": (store) => store.getState().setTool("select"),
  "timeline.bladeTool": (store) => store.getState().setTool("blade"),
  "timeline.split": (_, commands) => void commands.splitAtPlayhead(),
  "timeline.markIn": (store) => markRange(store, "in"),
  "timeline.markOut": (store) => markRange(store, "out"),
  "timeline.delete": (store, commands) => void deleteGapOrSelection(store, commands, false),
  "timeline.rippleDelete": (store, commands) => void deleteGapOrSelection(store, commands, true),
  "timeline.duplicate": (_, commands) => void commands.duplicateSelection(),
  "timeline.copy": (_, commands) => void commands.copySelection(),
  "timeline.cut": (_, commands) => void commands.cutSelection(),
  "timeline.paste": (_, commands) => void commands.paste(),
  "timeline.pasteInsert": (_, commands) => void commands.pasteInsert(),
  "timeline.trimStart": (_, commands) => void commands.trimToPlayhead("start"),
  "timeline.trimEnd": (_, commands) => void commands.trimToPlayhead("end"),
  "timeline.nudgeLeft": (_, commands) => void commands.nudgeSelection(-1),
  "timeline.nudgeRight": (_, commands) => void commands.nudgeSelection(1),
  "timeline.moveTrackUp": (_, commands) => void commands.moveSelectionToTrack(-1),
  "timeline.moveTrackDown": (_, commands) => void commands.moveSelectionToTrack(1),
  "timeline.selectForwardTrack": (store) => selectForward(store, false),
  "timeline.selectForwardAll": (store) => selectForward(store, true),
  "navigation.stepBack": (store) => {
    const state = store.getState();
    state.seek(roundTimelineSeconds(state.playheadSeconds - timelineSnapSeconds));
  },
  "navigation.stepForward": (store) => {
    const state = store.getState();
    state.seek(roundTimelineSeconds(state.playheadSeconds + timelineSnapSeconds));
  },
  "navigation.previousEdit": (store) => seekToEditPoint(store, "previous"),
  "navigation.nextEdit": (store) => seekToEditPoint(store, "next"),
  "navigation.start": (store) => store.getState().seek(0),
  "navigation.end": (store) => {
    const state = store.getState();
    state.seek(state.project.timeline.durationSeconds);
  },
  "playback.toggle": (store) => store.getState().togglePlaying(),
};

/** Escape inside the timeline: select tool, no range marks, marquee or gap. Selection is global. */
function resetTimelineTools(store: EditorStore) {
  const state = store.getState();
  state.setTool("select");
  state.clearRange();
  state.setMarquee(null);
  state.selectGap(null);
}

/**
 * Timeline-scope keymap handling for the "Timeline canvas" region's `onKeyDown`. Keys typed in
 * editable fields, handled by a focused control (clips, trim handles, diamonds), or fired inside
 * portalled menus and dialogs are ignored. Escape resets the tool, range and marquee and is left
 * for the global handler, which clears the selection.
 */
export function useTimelineShortcuts(platform: ShortcutPlatform): (event: KeyboardEvent<HTMLElement>) => void {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  return useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      const { target } = event;
      if (event.defaultPrevented || isEditableKeyboardTarget(target)) return;
      if (!(target instanceof Node) || !event.currentTarget.contains(target)) return;
      const shortcut = matchShortcut(event.nativeEvent, "timeline", platform);
      if (!shortcut) {
        if (matchShortcut(event.nativeEvent, "global", platform)?.id === "editor.clearSelection") resetTimelineTools(store);
        return;
      }
      // Space on a focused button presses it rather than toggling playback.
      if (shortcut.id === "playback.toggle" && target instanceof Element && target.closest("button")) return;
      const run = runners[shortcut.id];
      if (!run) return;
      event.preventDefault();
      run(store, commands);
    },
    [commands, platform, store],
  );
}
