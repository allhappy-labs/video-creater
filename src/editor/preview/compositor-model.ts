import type { MediaAsset } from "@/lib/project";
import type { TimelinePreviewIssueState } from "@/lib/preview/canvas-geometry";
import type { Timeline } from "@/lib/timeline";
import type { TimelinePreviewFrame, TimelinePreviewLayer } from "@/lib/timeline-preview";
import type { CompositorCanonicalState } from "./canonical-frames";

export type TimelinePreviewAudioLayer = TimelinePreviewFrame["audioLayers"][number];

export interface CompositorModelInput {
  readonly frame: TimelinePreviewFrame;
  readonly timeline: Timeline;
  readonly media: readonly MediaAsset[];
  readonly playheadSeconds: number;
  readonly mediaPreviewUrls: Readonly<Record<string, string | null | undefined>>;
  readonly failedLayerIds: ReadonlySet<string>;
  readonly canonical: CompositorCanonicalState;
  /** Item ids drawn by canonical prepared frames; once ready the DOM compositor leaves them out. */
  readonly coverageItemIds: ReadonlySet<string>;
  readonly canonicalFrameCount: number;
  /** Audio layers of the ready prepared project with their preview URLs; reversed audio plays these. */
  readonly preparedAudioLayers: readonly CompositorAudioLayer[];
  readonly mediaLoading?: boolean | undefined;
}

interface CompositorAudioLayer {
  readonly layer: TimelinePreviewAudioLayer;
  readonly sourceUrl: string;
}

/** A dip transition's opaque solid, drawn in the compositor just beneath the transition's clips. */
interface CompositorTransitionSolid {
  readonly transitionId: string;
  readonly color: "black" | "white";
  /** The first drawn layer of the pair, or null when neither clip is drawn (the solid goes after the media layers). */
  readonly beforeItemId: string | null;
}

export interface CompositorModel {
  readonly visibleMediaLayers: readonly { readonly layer: TimelinePreviewLayer; readonly sourceUrl: string }[];
  readonly visibleOverlayLayers: TimelinePreviewFrame["overlayLayers"];
  readonly transitionSolids: readonly CompositorTransitionSolid[];
  readonly visibleAudioLayers: readonly CompositorAudioLayer[];
  readonly issues: readonly string[];
  readonly issueTitle: string;
  readonly issueState: TimelinePreviewIssueState;
  /** Retry reloads failed layers. */
  readonly retryReloadsLayers: boolean;
  /** Retry re-runs canonical preparation (missing prepared frames or a failed preparation). */
  readonly retryPreparesCanonical: boolean;
  /** The layer "Open source" opens. */
  readonly problemLayer: { readonly itemId: string; readonly mediaId: string } | null;
  /** Copy for the empty canvas, or null when something is drawn. */
  readonly emptyStateCopy: string | null;
  readonly failedLayerCount: number;
}

const pendingCanonicalIssue = "Preparing canonical Lottie, LUT, reversed-clip, or richer-blend preview frames.";
const pendingReversedAudioIssue = "Reversed audio is preparing.";
const failedCanonicalIssue =
  "Canonical preview preparation failed; affected layers are hidden to avoid an inaccurate approximation.";
/** Retrying cannot fix a project that has no folder to prepare into. */
export const noProjectFolderMessage = "Canonical preview preparation requires a saved project folder.";

function unresolvedMediaItem(input: CompositorModelInput) {
  for (const track of input.timeline.tracks) {
    if (track.enabled === false) continue;
    for (const item of track.items) {
      const { source } = item;
      if (
        source.type === "media" &&
        input.playheadSeconds >= item.startSeconds &&
        input.playheadSeconds < item.startSeconds + item.durationSeconds &&
        !input.media.some((asset) => asset.id === source.mediaId)
      ) {
        return { itemId: item.id, mediaId: source.mediaId };
      }
    }
  }
  return null;
}

function emptyStateCopy(missingCount: number, awaitingCount: number, canonical: CompositorCanonicalState): string {
  if (missingCount > 0) return "Canonical preview unavailable";
  if (awaitingCount > 0) return canonical?.status === "pending" ? "Preparing canonical preview" : "Canonical preview unavailable";
  return "No timeline media at playhead";
}

/** Which layers the DOM compositor draws, and the issue, retry and empty-state copy for the frame. */
export function buildCompositorModel(input: CompositorModelInput): CompositorModel {
  const { frame, mediaPreviewUrls, failedLayerIds, canonical, coverageItemIds } = input;
  const unavailableLayers = input.mediaLoading ? [] : frame.layers.filter((layer) => !mediaPreviewUrls[layer.mediaId]);
  const canonicalReady = canonical?.status === "ready";
  const templateIds = new Set(frame.canonicalTemplateItemIds ?? []);
  const visibleOverlayLayers = frame.overlayLayers.filter((layer) =>
    !(canonical && templateIds.has(layer.itemId)) && !(canonicalReady && coverageItemIds.has(layer.itemId)));
  const awaitingTemplateIds = canonical && !canonicalReady ? [...templateIds] : [];
  const missingTemplateIds = canonicalReady && !input.mediaLoading ? [...templateIds].filter((id) => !coverageItemIds.has(id)) : [];
  // Media elements can't play backwards: reversed audio plays its prepared intermediate, else stays silent.
  const preparedAudioFor = (layer: TimelinePreviewAudioLayer) =>
    canonicalReady ? input.preparedAudioLayers.find((prepared) => prepared.layer.itemId === layer.itemId) : undefined;
  const reversedAudioLayers = frame.audioLayers.filter((layer) => layer.canonicalPreparationRequired);
  const awaitingReversedAudio = canonicalReady ? [] : reversedAudioLayers;
  const missingReversedAudio = canonicalReady && !input.mediaLoading ? reversedAudioLayers.filter((layer) => !preparedAudioFor(layer)) : [];
  const unavailableAudioLayers = input.mediaLoading ? [] : frame.audioLayers.filter((layer) => !layer.canonicalPreparationRequired && !mediaPreviewUrls[layer.mediaId]);
  const visibleMediaLayers = frame.layers.flatMap((layer) => {
    // Any canonical state hides layers that need preparation instead of approximating them, and
    // ready prepared frames stand in for the layers they cover (such as a flattened transition's clips).
    if (canonical && layer.canonicalPreparationRequired) return [];
    if (canonicalReady && coverageItemIds.has(layer.itemId)) return [];
    const sourceUrl = mediaPreviewUrls[layer.mediaId];
    return sourceUrl && !failedLayerIds.has(layer.itemId) ? [{ layer, sourceUrl }] : [];
  });
  const transitionSolids: CompositorTransitionSolid[] = (frame.transitions ?? []).flatMap(({ transitionId, solidColor, leftItemId, rightItemId, flattened }) => {
    if (solidColor === null) return [];
    // A flattened composite bakes the solid with both clips.
    if (canonicalReady && flattened && coverageItemIds.has(leftItemId) && coverageItemIds.has(rightItemId)) return [];
    const first = visibleMediaLayers.find(({ layer }) => layer.itemId === leftItemId || layer.itemId === rightItemId);
    return [{ transitionId, color: solidColor, beforeItemId: first?.layer.itemId ?? null }];
  });

  const visibleAudioLayers = frame.audioLayers.flatMap((layer) => {
    if (failedLayerIds.has(layer.itemId)) return [];
    if (layer.canonicalPreparationRequired) {
      const prepared = preparedAudioFor(layer);
      return prepared ? [prepared] : [];
    }
    const sourceUrl = mediaPreviewUrls[layer.mediaId];
    return sourceUrl ? [{ layer, sourceUrl }] : [];
  });
  const failedLayers = frame.layers.filter((layer) => failedLayerIds.has(layer.itemId));
  const failedAudioLayers = frame.audioLayers.filter((layer) => failedLayerIds.has(layer.itemId));
  const awaitingCanonicalLayers =
    canonical && canonical.status !== "ready" ? frame.layers.filter((layer) => layer.canonicalPreparationRequired) : [];
  const missingCanonicalLayers =
    canonical?.status === "ready" && !input.mediaLoading
      ? frame.layers.filter((layer) => layer.canonicalPreparationRequired && !coverageItemIds.has(layer.itemId))
      : [];
  const failedLayerCount = failedLayers.length + failedAudioLayers.length;
  const awaitingPreparation = awaitingCanonicalLayers.length > 0 || awaitingReversedAudio.length > 0 || awaitingTemplateIds.length > 0;
  const failedPreparationIssue = canonical?.status === "failed" ? canonical.message || failedCanonicalIssue : null;
  const preparationIssues = failedPreparationIssue
    ? awaitingPreparation ? [failedPreparationIssue] : []
    : [...(awaitingCanonicalLayers.length > 0 ? [pendingCanonicalIssue] : []),
      ...(awaitingTemplateIds.length > 0 ? ["Preparing animated graphics preview frames."] : []),
      ...(awaitingReversedAudio.length > 0 ? [pendingReversedAudioIssue] : [])];

  const issues = [
    ...frame.issues,
    ...preparationIssues,
    ...missingCanonicalLayers.map((layer) => `Prepared frame missing for timeline item ${layer.itemId}. Rebuild the canonical preview.`),
    ...missingTemplateIds.map((id) => `Prepared frame missing for timeline item ${id}. Rebuild the canonical preview.`),
    ...missingReversedAudio.map((layer) => `Prepared audio missing for timeline audio item ${layer.itemId}. Rebuild the canonical preview.`),
    ...unavailableLayers.map((layer) => `Timeline item ${layer.itemId} has no local preview URL.`),
    ...unavailableAudioLayers.map((layer) => `Timeline audio item ${layer.itemId} has no local preview URL.`),
    ...failedLayers.map((layer) => `Timeline item ${layer.itemId} failed to load.`),
    ...failedAudioLayers.map((layer) => `Timeline audio item ${layer.itemId} failed to load.`),
  ];
  const issueTitle =
    failedLayerCount > 0
      ? "Preview failed"
      : awaitingPreparation && canonical?.status === "pending"
        ? "Preparing preview"
        : missingCanonicalLayers.length > 0 || missingTemplateIds.length > 0
          ? "Prepared frame missing"
          : "Preview unavailable";
  const preparationFailed = awaitingPreparation && canonical?.status === "failed" && canonical.message !== noProjectFolderMessage;
  const retryReloadsLayers = failedLayerCount > 0;
  const retryPreparesCanonical = missingCanonicalLayers.length > 0 || missingTemplateIds.length > 0 || missingReversedAudio.length > 0 || preparationFailed;
  const retryVisible = retryReloadsLayers || retryPreparesCanonical;

  const firstProblem = failedLayers[0] ?? failedAudioLayers[0] ?? missingCanonicalLayers[0] ?? missingReversedAudio[0] ?? unavailableLayers[0] ?? unavailableAudioLayers[0];
  const hasVisiblePreview =
    visibleMediaLayers.length > 0 || transitionSolids.length > 0 || visibleOverlayLayers.length > 0 || input.canonicalFrameCount > 0;

  return {
    visibleMediaLayers,
    visibleOverlayLayers,
    transitionSolids,
    visibleAudioLayers,
    issues,
    issueTitle,
    issueState: issues.length === 0 ? "clear" : retryVisible ? "retry" : "notice",
    retryReloadsLayers,
    retryPreparesCanonical,
    problemLayer: firstProblem ? { itemId: firstProblem.itemId, mediaId: firstProblem.mediaId } : unresolvedMediaItem(input),
    emptyStateCopy: hasVisiblePreview ? null : canonicalReady && input.mediaLoading ? "Connecting prepared preview frames" : emptyStateCopy(missingCanonicalLayers.length + missingTemplateIds.length, awaitingCanonicalLayers.length + awaitingTemplateIds.length, canonical),
    failedLayerCount,
  };
}
