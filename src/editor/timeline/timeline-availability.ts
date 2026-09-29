import { useMemo } from "react";
import type { VideoProject } from "@/lib/project";
import {
  defaultSplitItemId,
  deleteItems,
  duplicateItems,
  linkItems,
  nudgeItems,
  rippleDeleteItems,
  splitAtPlayhead,
  unlinkItems,
  type CommandResult,
} from "@/lib/timeline-ops/clip-commands";
import { pastePlan, type TimelineClipboard } from "@/lib/timeline-ops/clipboard";
import { stringProperty } from "@/lib/timeline-ops/item-properties";
import { useEditorStore } from "../store/editor-store-context";
import { frameSnappedSeconds } from "./timeline-commands";

/** Why a selection command is unavailable (user-facing planner copy), or null when it can run. */
export interface SelectionAvailability {
  readonly delete: string | null;
  readonly rippleDelete: string | null;
  readonly duplicate: string | null;
  readonly copy: string | null;
  readonly cut: string | null;
  /** Link while no selected clip is linked, otherwise Unlink. */
  readonly linkMode: "link" | "unlink";
  readonly link: string | null;
  readonly nudgeLeft: string | null;
  readonly nudgeRight: string | null;
}

export interface PlayheadAvailability {
  readonly split: string | null;
  readonly paste: string | null;
  readonly pasteInsert: string | null;
}

/** The blocked reason of a planner result. */
export function blockedReason(result: CommandResult): string | null {
  return "blocked" in result ? result.blocked : null;
}

/** Link group ids only matter for availability, so a placeholder id is enough to plan. */
const probeLinkGroupId = "link-availability";

function selectionAvailability(project: VideoProject, itemIds: readonly string[]): SelectionAvailability {
  const ids = new Set(itemIds);
  const linked = project.timeline.tracks.some((track) =>
    track.items.some((item) => ids.has(item.id) && stringProperty(item, "linkGroupId")),
  );
  const deleteReason = blockedReason(deleteItems(project, itemIds));
  return {
    delete: deleteReason,
    rippleDelete: blockedReason(rippleDeleteItems(project, itemIds)),
    duplicate: blockedReason(duplicateItems(project, itemIds)),
    copy: ids.size > 0 ? null : "Select a clip to copy.",
    cut: ids.size > 0 ? deleteReason : "Select a clip to cut.",
    linkMode: linked ? "unlink" : "link",
    link: blockedReason(linked ? unlinkItems(project, itemIds) : linkItems(project, itemIds, probeLinkGroupId)),
    nudgeLeft: blockedReason(nudgeItems(project, itemIds, -1)),
    nudgeRight: blockedReason(nudgeItems(project, itemIds, 1)),
  };
}

function playheadAvailability(
  project: VideoProject,
  itemIds: readonly string[],
  playheadSeconds: number,
  clipboard: TimelineClipboard | null,
): PlayheadAvailability {
  const splitSeconds = frameSnappedSeconds(project, playheadSeconds);
  const pasteReason = (mode: "paste" | "insert") =>
    clipboard ? blockedReason(pastePlan(project, clipboard, playheadSeconds, mode)) : "Copy a clip first.";
  return {
    split: blockedReason(splitAtPlayhead(project, itemIds, splitSeconds, (itemId) => defaultSplitItemId(itemId, splitSeconds))),
    paste: pasteReason("paste"),
    pasteInsert: pasteReason("insert"),
  };
}

/** Availability of the selection commands, recomputed only when the project or selection changes. */
export function useSelectionAvailability(): SelectionAvailability {
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  return useMemo(() => selectionAvailability(project, selectedItemIds), [project, selectedItemIds]);
}

/** Availability of the commands that depend on the playhead. */
export function usePlayheadAvailability(): PlayheadAvailability {
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const clipboard = useEditorStore((state) => state.clipboard);
  return useMemo(
    () => playheadAvailability(project, selectedItemIds, playheadSeconds, clipboard),
    [project, selectedItemIds, playheadSeconds, clipboard],
  );
}
