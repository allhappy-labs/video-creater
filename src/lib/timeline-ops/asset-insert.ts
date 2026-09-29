import { roundTimelineSeconds } from "@/lib/format";
import { mediaTimelineItem } from "@/lib/generation/timeline-placement";
import { createTemplateOverlayItem } from "@/lib/motion-templates";
import type { ProjectAction, VideoProject } from "@/lib/project";
import { createShaderBackgroundTemplateItem } from "@/lib/shader-background-templates";
import { itemAllowedOnTrack, type TimelineItem, type TimelineItemKind } from "@/lib/timeline";
import { withEmptyTrackRemovals, type CommandResult } from "@/lib/timeline-ops/clip-commands";
import { newTrackId, planDropTarget, type AssetKind } from "@/lib/timeline-ops/dynamic-tracks";
import { orderedTracksByBand } from "@/lib/timeline-ops/track-bands";

/**
 * A panel asset that can be placed on the timeline: media, a motion template, a shader background
 * or the default text overlay ("Add text"; its `id` is unused).
 */
export interface AssetRef {
  readonly kind: "media" | "template" | "background" | "text";
  readonly id: string;
}

/** Where an asset lands: the hovered track, or the band-ordered insert index between tracks. */
export interface AssetPlacement {
  readonly hoveredTrackId: string | null;
  readonly insertIndex: number | null;
  readonly startSeconds: number;
}

/** Legacy add-text defaults (3 s, editable overlay guidance) with the redesign's "Text" / "Your text" copy. */
const defaultTextOverlay = { durationSeconds: 3, label: "Text", text: "Your text" } as const;
const defaultTextOverlayGuidance = {
  visualTreatment: "clean editable text overlay with high-contrast type and transparent backing",
  motion: "quick fade in, hold, and soft fade out",
  safeZone: "keep text inside 10% title-safe margins",
  avoid: "opaque slabs, default-font template look, and covering faces or key action",
} as const;

function defaultTextOverlayItem(itemId: string, startSeconds: number): TimelineItem {
  const { durationSeconds, label, text } = defaultTextOverlay;
  return {
    id: itemId,
    kind: "overlay",
    startSeconds,
    durationSeconds,
    source: { type: "text", text },
    label,
    properties: { text, ...defaultTextOverlayGuidance },
  };
}

interface CreatedItem {
  readonly item: TimelineItem;
  readonly assetKind: AssetKind;
}

function createItem(project: VideoProject, asset: AssetRef, startSeconds: number, idSuffix: string): CreatedItem | { blocked: string } {
  switch (asset.kind) {
    case "media": {
      const item = mediaTimelineItem(project, asset.id, startSeconds);
      if (!item) return { blocked: "That media is no longer in the project." };
      return { item, assetKind: item.kind === "audio_clip" ? "audio" : "video" };
    }
    case "template":
      try {
        return {
          item: createTemplateOverlayItem({ templateId: asset.id, itemId: `template-${asset.id}-${idSuffix}`, startSeconds }),
          assetKind: "template",
        };
      } catch {
        return { blocked: "This template can't be placed on the timeline." };
      }
    case "background":
      try {
        return {
          item: createShaderBackgroundTemplateItem({ templateId: asset.id, itemId: `shader-background-${asset.id}-${idSuffix}`, startSeconds }),
          assetKind: "background",
        };
      } catch {
        return { blocked: "This background can't be placed on the timeline." };
      }
    case "text":
      return { item: defaultTextOverlayItem(`text-overlay-${idSuffix}`, startSeconds), assetKind: "text" };
  }
}

/**
 * One batch that places an asset: `createTrack` and its `reorderTrack` when the target is a new
 * track, then `addItems`, then removal of emptied tracks. Drops onto a locked track, a colliding
 * clip on an incompatible band, or a missing asset are blocked with user-facing copy.
 * `idSuffix` keeps template and text item ids unique (the caller passes a timestamp).
 */
export function planAssetInsert(project: VideoProject, asset: AssetRef, placement: AssetPlacement, idSuffix: string): CommandResult {
  const startSeconds = roundTimelineSeconds(Number.isFinite(placement.startSeconds) ? Math.max(0, placement.startSeconds) : 0);
  const created = createItem(project, asset, startSeconds, idSuffix);
  if ("blocked" in created) return created;
  const { item, assetKind } = created;
  const { timeline } = project;
  const plan = planDropTarget({
    timeline,
    assetKind,
    hoveredTrackId: placement.hoveredTrackId,
    insertIndex: placement.insertIndex,
    startSeconds: item.startSeconds,
    durationSeconds: item.durationSeconds,
    newTrackId: newTrackId(timeline, assetKind),
  });
  if (plan.kind === "invalid") return { blocked: plan.reason };

  const actions: ProjectAction[] = [];
  if (plan.kind === "create") {
    actions.push(plan.createTrack);
    if (plan.reorderTrack) actions.push(plan.reorderTrack);
  }
  actions.push({ type: "addItems", targetTrackId: plan.trackId, items: [item] });
  return withEmptyTrackRemovals(project, actions);
}

function itemKindForAsset(project: VideoProject, asset: AssetRef): TimelineItemKind {
  if (asset.kind === "template" || asset.kind === "text") return "overlay";
  if (asset.kind === "background") return "hyperframe_scene";
  return project.media.find((media) => media.id === asset.id)?.kind === "audio" ? "audio_clip" : "video_clip";
}

/**
 * The placement for "add at playhead" buttons: the first unlocked track (top to bottom) that
 * accepts the asset, where a collision makes `planAssetInsert` create a track next to it; with
 * no such track, a new track at the end of the asset's band.
 */
export function playheadAssetPlacement(project: VideoProject, asset: AssetRef, playheadSeconds: number): AssetPlacement {
  const itemKind = itemKindForAsset(project, asset);
  const track = orderedTracksByBand(project.timeline).find((candidate) => !candidate.locked && itemAllowedOnTrack(itemKind, candidate.kind));
  return { hoveredTrackId: track?.id ?? null, insertIndex: null, startSeconds: playheadSeconds };
}
