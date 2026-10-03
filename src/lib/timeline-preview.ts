import type { GeneratedAsset, MediaAsset } from "./project";
import type { Timeline, TimelineItem } from "./timeline";
import {
  activeTrackTransitions,
  audioLayerTransition,
  carryTransitionsThroughExpansion,
  isItemActiveWithTransitions,
  itemTransitionAt,
  planTrackTransitions,
  transitionHandleSourceSeconds,
  visualLayerTransition,
  type TimelinePreviewAudioTransition,
  type TimelinePreviewLayerTransition,
  type TimelinePreviewTransition,
} from "./preview/transition-frame";
import { planPreviewTransitionEligibility } from "./preview/transition-eligibility";
import { isReversedItem, sourceSecondsAt } from "./timeline-ops/reverse";

type TimelinePreviewStatus =
  | "ready"
  | "empty"
  | "missing-media"
  | "unsupported-source";
type TimelinePreviewOverlayKind = "caption" | "text" | "template";
type TimelinePreviewCaptionPlacement = "lower" | "center" | "upper";
type TimelinePreviewCaptionStylePreset =
  | "boldReadableLower"
  | "kineticFocus"
  | "centeredMinimal";

interface TimelinePreviewColorGrade {
  exposure: number;
  contrast: number;
  saturation: number;
}

interface TimelinePreviewEffects {
  grain: boolean;
  vignette: boolean;
}

export interface TimelinePreviewLayer {
  itemId: string;
  label: string;
  mediaId: string;
  mediaKind: MediaAsset["kind"];
  relativePath: string;
  timelineStartSeconds: number;
  timelineEndSeconds: number;
  sourceTimeSeconds: number;
  opacity: number;
  blendMode: "over" | "add";
  positionX: number;
  positionY: number;
  scale: number;
  scaleX: number;
  scaleY: number;
  rotationDegrees: number;
  centerX?: number;
  centerY?: number;
  width?: number;
  height?: number;
  flipHorizontal?: boolean;
  flipVertical?: boolean;
  cropTop: number;
  cropRight: number;
  cropBottom: number;
  cropLeft: number;
  colorGrade: TimelinePreviewColorGrade;
  effects: TimelinePreviewEffects;
  /** Drawn only from prepared frames: Lottie, richer blends, effects, colour grades and reversed clips. */
  canonicalPreparationRequired: boolean;
  /** Media element rate: the clip's speed, so playback advances through the source at that speed. */
  playbackRate: number;
  /** Present while the layer is one side of a clip transition; its opacity is already applied. */
  transition?: TimelinePreviewLayerTransition;
}

export interface TimelinePreviewFrame {
  status: TimelinePreviewStatus;
  playheadSeconds: number;
  layers: TimelinePreviewLayer[];
  audioLayers: TimelinePreviewAudioLayer[];
  overlayLayers: TimelinePreviewOverlayLayer[];
  issues: string[];
  /** Clip transitions drawn at the playhead; omitted when there are none. */
  transitions?: TimelinePreviewTransition[];
  /**
   * Visual clips a flattened composite draws at the playhead: every clip on an enabled video track
   * at or below an active group's top track (Rust `collect_dependencies`). Omitted when none.
   */
  flattenedCoverItemIds?: string[];
  /** Authored shader and motion templates resolved by Rust into animated frame sequences. */
  canonicalTemplateItemIds?: string[];
}

interface TimelinePreviewOverlayLayer {
  itemId: string;
  label: string;
  overlayKind: TimelinePreviewOverlayKind;
  text: string | null;
  templateId: string | null;
  timelineStartSeconds: number;
  timelineEndSeconds: number;
  opacity: number;
  positionX: number;
  positionY: number;
  scale: number;
  scaleX: number;
  scaleY: number;
  rotationDegrees: number;
  captionPlacement?: TimelinePreviewCaptionPlacement;
  captionStylePreset?: TimelinePreviewCaptionStylePreset;
  emphasizedWordIndices?: number[];
  activeEmphasizedWordIndices?: number[];
  captionWordStyles?: TimelinePreviewCaptionWordStyle[];
}

interface TimelinePreviewCaptionWordStyle {
  wordIndex: number;
  scale: number;
  opacity: number;
  color: string;
}

interface TimelinePreviewAudioLayer {
  itemId: string;
  label: string;
  mediaId: string;
  relativePath: string;
  timelineStartSeconds: number;
  timelineEndSeconds: number;
  sourceTimeSeconds: number;
  gain: number;
  /** Media element rate: the clip's speed, played with pitch preserved. */
  playbackRate: number;
  /** A reversed clip: media elements can't play backwards, so it plays its prepared reversed audio. */
  canonicalPreparationRequired: boolean;
  /** Present while the layer is one side of an audio transition; its gain is already applied. */
  transition?: TimelinePreviewAudioTransition;
}

export interface TimelinePreviewPoint {
  x: number;
  y: number;
}

export interface TimelinePreviewViewportSize {
  width: number;
  height: number;
}

export interface TimelinePreviewLayerGeometry {
  centerX: number;
  centerY: number;
  width: number;
  height: number;
  rotationDegrees: number;
}

const PREVIEW_OPACITY_SAMPLE_FPS = 120;

interface TimelinePreviewInput {
  timeline: Timeline;
  timelines?: Array<{ id: string; timeline: Timeline }>;
  media: MediaAsset[];
  generatedAssets?: GeneratedAsset[];
  playheadSeconds: number;
  /** The project frame rate, for transition adjacency (24 when omitted or invalid). */
  fps?: number;
}

type PreviewStructuralPlan = ReturnType<typeof createPreviewStructuralPlan>;
interface PreviewStructuralCacheEntry {
  input: TimelinePreviewInput;
  signature: string;
  plan: PreviewStructuralPlan;
}

// The live and prepared timeline may alternate. Strong retention is bounded to two
// modest plans, including their source inputs; a project history cannot grow this cache.
const structuralPlans: PreviewStructuralCacheEntry[] = [];
const MAX_STRUCTURAL_SIGNATURE_CHARACTERS = 2 * 1024 * 1024;
const MAX_STRUCTURAL_PLAN_UNITS = 100_000;

function createPreviewStructuralPlan(input: TimelinePreviewInput, frameSeconds: number) {
  const timelinesById = new Map(
    (input.timelines ?? []).map((entry) => [entry.id, entry.timeline]),
  );
  const timeline = expandTimelineForPreview(input.timeline, timelinesById);
  const mediaById = new Map(input.media.map((asset) => [asset.id, asset]));
  const generatedAssetsById = new Map(
    (input.generatedAssets ?? []).map((asset) => [asset.id, asset]),
  );
  const transitionSources = { media: input.media, generatedAssets: input.generatedAssets ?? [] };
  const { eligibility, flattenedGroups } = planPreviewTransitionEligibility(timeline, transitionSources, frameSeconds);
  const trackTransitions = timeline.tracks.map((track) => planTrackTransitions(track, transitionSources, frameSeconds, eligibility.droppedTransitionIds));
  return { timeline, mediaById, generatedAssetsById, eligibility, flattenedGroups, trackTransitions };
}

function structuralPlanUnits(plan: PreviewStructuralPlan): number {
  let units = plan.mediaById.size + plan.generatedAssetsById.size + plan.flattenedGroups.length;
  for (const track of plan.timeline.tracks) {
    units += track.items.length + (track.transitions?.length ?? 0);
    for (const item of track.items) {
      const keyframes = item.properties.keyframes;
      if (keyframes && typeof keyframes === "object" && !Array.isArray(keyframes)) {
        for (const values of Object.values(keyframes)) if (Array.isArray(values)) units += values.length;
      }
      if (units > MAX_STRUCTURAL_PLAN_UNITS) return units;
    }
  }
  return units;
}

function previewStructuralPlan(input: TimelinePreviewInput, frameSeconds: number): PreviewStructuralPlan {
  const index = structuralPlans.findIndex((entry) => entry.input.timeline === input.timeline && entry.input.timelines === input.timelines && entry.input.media === input.media && entry.input.generatedAssets === input.generatedAssets);
  const previous = index >= 0 ? structuralPlans.splice(index, 1)[0] : undefined;
  // Simple timelines were already cheap. Avoid signature serialization when there
  // is no nested, transition or preparation work to amortize across frames.
  const needsReuse = input.timeline.tracks.some((track) => track.transitions?.length || track.items.some((item) => item.source.type === "timeline" || item.properties.effects !== undefined || item.properties.colorGrade !== undefined || item.properties.blendMode !== undefined));
  if (!needsReuse) return createPreviewStructuralPlan(input, frameSeconds);
  // Callers normally replace canonical snapshots, but public frame callers also use
  // mutable objects. A content signature detects in-place edits, including nested
  // properties/media/output readiness, rather than silently trusting object identity.
  let signature: string;
  try {
    let ordinal = 0;
    const specialNumbers: string[] = [];
    const serialized = JSON.stringify([input.timeline, input.timelines, input.media, input.generatedAssets, frameSeconds], (_key, value: unknown) => {
      // JSON normalizes -0/nonfinite values. Preserve those distinctions for
      // mutable callers without substituting values the evaluator will read.
      if (typeof value === "number" && (!Number.isFinite(value) || Object.is(value, -0))) specialNumbers.push(`${ordinal}:${Object.is(value, -0) ? "-0" : String(value)}`);
      ordinal += 1;
      return value;
    });
    signature = `${serialized}\0${specialNumbers.join(",")}`;
  } catch {
    // Non-JSON extra properties were harmless to the frame evaluator before
    // reuse was introduced; evaluate them freshly instead of rejecting them.
    return createPreviewStructuralPlan(input, frameSeconds);
  }
  if (previous?.signature === signature) { structuralPlans.push(previous); return previous.plan; }
  const plan = createPreviewStructuralPlan(input, frameSeconds);
  if (signature.length <= MAX_STRUCTURAL_SIGNATURE_CHARACTERS && structuralPlanUnits(plan) <= MAX_STRUCTURAL_PLAN_UNITS) {
    structuralPlans.push({ input: { ...input, playheadSeconds: 0 }, signature, plan });
    if (structuralPlans.length > 2) structuralPlans.shift();
  }
  return plan;
}

export function buildTimelinePreviewFrame(input: TimelinePreviewInput): TimelinePreviewFrame {
  const frameSeconds = input.fps !== undefined && Number.isFinite(input.fps) && input.fps > 0 ? 1 / input.fps : 1 / 24;
  const { timeline, mediaById, generatedAssetsById, eligibility, flattenedGroups: plannedGroups, trackTransitions: plannedTrackTransitions } = previewStructuralPlan(input, frameSeconds);
  const layers: TimelinePreviewLayer[] = [];
  const audioLayers: TimelinePreviewAudioLayer[] = [];
  const overlayLayers: TimelinePreviewOverlayLayer[] = [];
  const issues: string[] = [];
  const transitions: TimelinePreviewTransition[] = [];
  // Skip transitions render preparation removes, and mark the ones flattened composites bake.
  const flattenedGroups = plannedGroups.filter((group) => input.playheadSeconds >= group.start && input.playheadSeconds < group.end);
  const flattenedCoverItemIds: string[] = [];
  const canonicalTemplateItemIds: string[] = [];
  let hasMissingMedia = false;
  let hasUnsupportedSource = false;

  for (const [trackIndex, track] of timeline.tracks.entries()) {
    if (track.enabled === false) {
      continue;
    }
    const coveredByFlattenedGroup = track.kind === "video" && flattenedGroups.some((group) => trackIndex <= group.topTrackIndex);
    const trackTransitions = plannedTrackTransitions[trackIndex]!;
    const activeTransitions = activeTrackTransitions(trackTransitions, input.playheadSeconds, eligibility.flattenedTransitionIds);
    const trackLayerStart = layers.length;

    for (const item of track.items) {
      const itemTransitions = trackTransitions.get(item.id);
      const itemTransition = itemTransitionAt(itemTransitions, input.playheadSeconds);
      if (!isItemActiveWithTransitions(item, itemTransitions, input.playheadSeconds)) {
        const overlayLayer = buildOverlayLayer(item, input.playheadSeconds);
        if (overlayLayer) {
          overlayLayers.push(overlayLayer);
        }
        continue;
      }
      if (item.kind === "audio_clip") {
        const resolvedSource = resolveTimelinePreviewSource(
          item,
          mediaById,
          generatedAssetsById,
        );
        if (resolvedSource.status !== "ready") {
          if (resolvedSource.status === "missing") {
            hasMissingMedia = true;
          } else {
            hasUnsupportedSource = true;
          }
          issues.push(resolvedSource.issue(item.id));
          continue;
        }
        const asset = resolvedSource.asset;
        // Detached and linked audio clips play the sound of their video or generated media.
        if (asset.kind !== "audio" && asset.kind !== "video" && asset.kind !== "generated") {
          hasUnsupportedSource = true;
          issues.push(
            `Timeline audio item ${item.id} references ${asset.kind} media ${asset.id}.`,
          );
          continue;
        }
        if (asset.relativePath.trim().length === 0) {
          hasUnsupportedSource = true;
          issues.push(`Timeline audio item ${item.id} media ${asset.id} has no preview path.`);
          continue;
        }
        const localSeconds = input.playheadSeconds - item.startSeconds;
        const playbackSpeed = previewPlaybackSpeedForItem(item);
        const handleSourceSeconds = transitionHandleSourceSeconds(item, input.playheadSeconds, playbackSpeed, asset.durationSeconds);
        const sourceTime = handleSourceSeconds === null
          ? previewSourceTimeForItem(item, asset.durationSeconds, localSeconds, playbackSpeed)
          : { seconds: roundSeconds(handleSourceSeconds), unclampedSeconds: handleSourceSeconds, wasClamped: false };
        const audioTransition = itemTransition
          ? audioLayerTransition(itemTransition.transition, itemTransition.role, itemTransition.progress)
          : null;
        const gain = previewAudioGainForItem(item, localSeconds);
        if (sourceTime.wasClamped) {
          issues.push(
            `Timeline audio item ${item.id} source time ${roundSeconds(
              sourceTime.unclampedSeconds,
            )}s is outside its preview source range.`,
          );
        }
        audioLayers.push({
          itemId: item.id,
          label: item.label,
          mediaId: asset.id,
          relativePath: asset.relativePath,
          timelineStartSeconds: item.startSeconds,
          timelineEndSeconds: item.startSeconds + item.durationSeconds,
          sourceTimeSeconds: sourceTime.seconds,
          gain: audioTransition ? roundSeconds(gain * audioTransition.gain) : gain,
          playbackRate: playbackSpeed,
          canonicalPreparationRequired: isReversedItem(item),
          ...(audioTransition ? { transition: audioTransition } : {}),
        });
        continue;
      }
      const shaderTemplate = item.kind === "hyperframe_scene" && typeof item.properties.shaderBackgroundTemplateId === "string";
      const motionTemplate = item.kind === "overlay" && typeof item.properties.templateId === "string";
      if (shaderTemplate || motionTemplate) canonicalTemplateItemIds.push(item.id);
      if (shaderTemplate) continue;
      const overlayLayer = buildOverlayLayer(item, input.playheadSeconds);
      if (overlayLayer?.overlayKind === "template") {
        overlayLayers.push(overlayLayer);
        continue;
      }

      if (item.source.type !== "media" && item.source.type !== "generated") {
        if (item.source.type === "timeline") {
          hasUnsupportedSource = true;
          issues.push(
            `Timeline item ${item.id} nests timeline ${item.source.timelineId}; nested timeline preview is not implemented yet.`,
          );
          continue;
        }
        if (overlayLayer) {
          overlayLayers.push(overlayLayer);
        }
        continue;
      }

      const resolvedSource = resolveTimelinePreviewSource(
        item,
        mediaById,
        generatedAssetsById,
      );
      if (resolvedSource.status === "missing") {
        hasMissingMedia = true;
        issues.push(resolvedSource.issue(item.id));
        continue;
      }
      if (resolvedSource.status === "unsupported") {
        hasUnsupportedSource = true;
        issues.push(resolvedSource.issue(item.id));
        continue;
      }
      const asset = resolvedSource.asset;
      if (asset.kind === "audio") {
        hasUnsupportedSource = true;
        issues.push(
          `Timeline item ${item.id} references audio media ${asset.id}, which cannot be shown in the visual timeline preview.`,
        );
        continue;
      }
      if (asset.relativePath.trim().length === 0) {
        hasUnsupportedSource = true;
        issues.push(`Timeline item ${item.id} media ${asset.id} has no preview path.`);
        continue;
      }

      const offset = input.playheadSeconds - item.startSeconds;
      const motion = previewMotionForItem(item, offset);
      const playbackSpeed = previewPlaybackSpeedForItem(item);
      const handleSourceSeconds = transitionHandleSourceSeconds(
        item,
        input.playheadSeconds,
        playbackSpeed,
        asset.durationSeconds,
      );
      const sourceTime = handleSourceSeconds === null
        ? previewSourceTimeForItem(item, asset.durationSeconds, offset, playbackSpeed)
        : { seconds: roundSeconds(handleSourceSeconds), unclampedSeconds: handleSourceSeconds, wasClamped: false };
      const layerTransition = itemTransition
        ? visualLayerTransition(itemTransition.transition, itemTransition.role, itemTransition.progress)
        : null;
      if (sourceTime.wasClamped) {
        issues.push(
          `Timeline item ${item.id} source time ${roundSeconds(
            sourceTime.unclampedSeconds,
          )}s is outside its preview source range.`,
        );
      }
      layers.push({
        itemId: item.id,
        label: item.label,
        mediaId: asset.id,
        mediaKind: asset.kind,
        relativePath: asset.relativePath,
        timelineStartSeconds: item.startSeconds,
        timelineEndSeconds: item.startSeconds + item.durationSeconds,
        sourceTimeSeconds: sourceTime.seconds,
        opacity: layerTransition ? motion.opacity * layerTransition.opacity : motion.opacity,
        blendMode: previewBlendModeForItem(item),
        positionX: motion.positionX,
        positionY: motion.positionY,
        scale: motion.scale,
        scaleX: motion.scaleX,
        scaleY: motion.scaleY,
        rotationDegrees: motion.rotationDegrees,
        ...previewCanvasTransformForItem(item),
        ...previewCropForItem(item),
        colorGrade: previewColorGradeForItem(item),
        effects: previewEffectsForItem(item),
        canonicalPreparationRequired: previewRequiresCanonicalPreparation(item, asset),
        playbackRate: playbackSpeed,
        ...(layerTransition ? { transition: layerTransition } : {}),
      });
      if (coveredByFlattenedGroup) flattenedCoverItemIds.push(item.id);
    }

    if (activeTransitions.length > 0) {
      // Canonical clips never overlap, so only a transition pair shares a frame: outgoing beneath incoming.
      const trackLayers = layers.splice(trackLayerStart);
      layers.push(...trackLayers.sort((left, right) => left.timelineStartSeconds - right.timelineStartSeconds));
      transitions.push(...activeTransitions);
    }
  }

  return {
    status: hasMissingMedia
      ? "missing-media"
      : hasUnsupportedSource
        ? "unsupported-source"
        : layers.length > 0 || audioLayers.length > 0 || overlayLayers.length > 0 || canonicalTemplateItemIds.length > 0
        ? "ready"
        : "empty",
    playheadSeconds: input.playheadSeconds,
    layers,
    audioLayers,
    overlayLayers,
    issues,
    ...(transitions.length > 0 ? { transitions } : {}),
    ...(flattenedCoverItemIds.length > 0 ? { flattenedCoverItemIds } : {}),
    ...(canonicalTemplateItemIds.length > 0 ? { canonicalTemplateItemIds } : {}),
  };
}

function timelinePreviewLayerGeometry(
  layer: TimelinePreviewLayer,
  viewport: TimelinePreviewViewportSize,
): TimelinePreviewLayerGeometry {
  const viewportWidth = Math.max(1, viewport.width);
  const viewportHeight = Math.max(1, viewport.height);
  return {
    centerX: (layer.centerX ?? 0.5) + layer.positionX / viewportWidth,
    centerY: (layer.centerY ?? 0.5) + layer.positionY / viewportHeight,
    width: (layer.width ?? 1) * Math.abs(layer.scaleX),
    height: (layer.height ?? 1) * Math.abs(layer.scaleY),
    rotationDegrees: layer.rotationDegrees,
  };
}

export function timelinePreviewLayerContainsPoint(
  layer: TimelinePreviewLayer,
  point: TimelinePreviewPoint,
  viewport: TimelinePreviewViewportSize,
) {
  if (layer.opacity <= 0.01) return false;
  const geometry = timelinePreviewLayerGeometry(layer, viewport);
  if (geometry.width <= 0 || geometry.height <= 0) return false;

  const radians = (geometry.rotationDegrees * Math.PI) / 180;
  const cosine = Math.cos(radians);
  const sine = Math.sin(radians);
  const deltaX = (point.x - geometry.centerX) * Math.max(1, viewport.width);
  const deltaY = (point.y - geometry.centerY) * Math.max(1, viewport.height);
  const localX = deltaX * cosine + deltaY * sine;
  const localY = -deltaX * sine + deltaY * cosine;
  const width = geometry.width * Math.max(1, viewport.width);
  const height = geometry.height * Math.max(1, viewport.height);
  const left = -width / 2 + layer.cropLeft * width;
  const right = width / 2 - layer.cropRight * width;
  const top = -height / 2 + layer.cropTop * height;
  const bottom = height / 2 - layer.cropBottom * height;

  return localX >= left && localX <= right && localY >= top && localY <= bottom;
}

/**
 * The last-drawn layer under `point` (canvas fractions). `viewport` gives the units of
 * positionX/positionY and the aspect ratio: pass the render size, since the renderer treats
 * position offsets as output pixels and the preview canvas keeps the output aspect ratio.
 */
export function topmostTimelinePreviewLayerAtPoint(
  layers: readonly TimelinePreviewLayer[],
  point: TimelinePreviewPoint,
  viewport: TimelinePreviewViewportSize,
) {
  for (let index = layers.length - 1; index >= 0; index -= 1) {
    const layer = layers[index];
    if (layer && timelinePreviewLayerContainsPoint(layer, point, viewport)) {
      return layer;
    }
  }
  return null;
}

function previewRequiresCanonicalPreparation(
  item: TimelineItem,
  asset: Pick<MediaAsset, "kind">,
): boolean {
  if (asset.kind === "lottie" || isReversedItem(item)) {
    return true;
  }
  const blendMode = item.properties.blendMode;
  if (typeof blendMode === "string" && !["normal", "over"].includes(blendMode)) {
    return true;
  }
  const colorGrade = item.properties.colorGrade;
  if (
    colorGrade !== null &&
    typeof colorGrade === "object" &&
    !Array.isArray(colorGrade) &&
    Object.keys(colorGrade).length > 0
  ) return true;
  const effects = item.properties.effects;
  return Array.isArray(effects) && effects.some((effect) =>
    effect !== null && typeof effect === "object" && !Array.isArray(effect) &&
    (effect as Record<string, unknown>).enabled !== false);
}

function previewBlendModeForItem(item: TimelineItem): "over" | "add" {
  const value = item.properties.blendMode;
  return value === "add" ? value : "over";
}

function previewEffectsForItem(item: TimelineItem): TimelinePreviewEffects {
  const effects = item.properties.effects;
  if (!Array.isArray(effects)) {
    return { grain: false, vignette: false };
  }
  const enabled = (effectType: string) =>
    effects.some(
      (effect) =>
        effect &&
        typeof effect === "object" &&
        !Array.isArray(effect) &&
        (effect as Record<string, unknown>).effectType === effectType &&
        (effect as Record<string, unknown>).enabled !== false,
    );
  return {
    grain: enabled("stylize.grain"),
    vignette: enabled("stylize.vignette"),
  };
}

function expandTimelineForPreview(
  timeline: Timeline,
  timelinesById: Map<string, Timeline>,
  ancestors: readonly string[] = [],
  offsetSeconds = 0,
  endSeconds = Number.POSITIVE_INFINITY,
  parentOpacityCurves: AbsoluteOpacityCurve[] = [],
  parentMotionWrappers: TimelineItem[] = [],
  parentVolumeDb = 0,
  parentTransform = nestedCanvasTransformIdentity(),
  playbackSpeed = 1,
  namespace = "root",
): Timeline {
  const tracks = timeline.tracks.flatMap((track, trackIndex) => {
    const ownItems: TimelineItem[] = [];
    const nestedTracks: Timeline["tracks"] = [];

    for (const item of track.items) {
      const itemStart = offsetSeconds + item.startSeconds / playbackSpeed;
      const itemEnd = Math.min(itemStart + item.durationSeconds / playbackSpeed, endSeconds);
      if (itemEnd <= itemStart) {
        continue;
      }
      if (item.source.type !== "timeline") {
        const itemSpeed = numberProperty(item, "speed") ?? 1;
        const composedSpeed = itemSpeed * playbackSpeed;
        if (!Number.isFinite(composedSpeed) || composedSpeed < 0.1 || composedSpeed > 8) {
          ownItems.push({ ...item, startSeconds: itemStart, durationSeconds: itemEnd - itemStart });
          continue;
        }
        const properties = { ...item.properties };
        if (composedSpeed === 1) delete properties.speed;
        else properties.speed = composedSpeed;
        const expandedItem = {
          ...item,
          ...(namespace === "root" ? {} : { id: `${namespace}:${item.id}` }),
          startSeconds: itemStart,
          durationSeconds: itemEnd - itemStart,
          properties,
        };
        ownItems.push(
          composeNestedCanvasTransform(
            composeNestedAudioGain(
              composeNestedMotionWrappers(composeNestedOpacityCurves(
                expandedItem,
                parentOpacityCurves,
                itemStart,
                itemEnd,
              ), parentMotionWrappers, itemStart, itemEnd),
              parentVolumeDb,
            ),
            parentTransform,
          ),
        );
        continue;
      }

      const nestedTimeline = timelinesById.get(item.source.timelineId);
      const wrapper = nestedTimelineWrapperProperties(item);
      if (!nestedTimeline || ancestors.includes(item.source.timelineId) || wrapper === null) {
        // Preserve the source item so the normal evaluator reports a useful,
        // actionable unsupported-source issue instead of silently dropping it.
        ownItems.push({
          ...item,
          startSeconds: itemStart,
          durationSeconds: itemEnd - itemStart,
        });
        continue;
      }
      nestedTracks.push(
        ...(() => {
          const timedWrapper = scaleNestedWrapperAnimationTiming(item, playbackSpeed);
          timedWrapper.startSeconds = itemStart;
          const wrapperFadeCurve = fadeOpacityCurveForItem(
            timedWrapper,
            itemStart,
            timedWrapper.durationSeconds,
          );
          return expandTimelineForPreview(
            nestedTimeline,
            timelinesById,
            [...ancestors, item.source.timelineId],
            itemStart,
            itemEnd,
            [
              ...parentOpacityCurves,
              opacityCurveForItem(timedWrapper, itemStart, timedWrapper.durationSeconds),
              ...(wrapperFadeCurve ? [wrapperFadeCurve] : []),
            ],
            [...parentMotionWrappers, timedWrapper],
            parentVolumeDb + wrapper.volumeDb,
            composeNestedCanvasTransforms(parentTransform, wrapper.transform),
            playbackSpeed * wrapper.speed,
            // Rust `expand_timeline_for_render` namespaces by track index; prepared frames and audio match these ids.
            `${namespace}:${trackIndex}:${item.id}`,
          ).tracks;
        })(),
      );
    }

    const { transitions: _transitions, ...trackWithoutTransitions } = track;
    const carriedTransitions = carryTransitionsThroughExpansion(track, ownItems, namespace, playbackSpeed);
    const ownTrack = ownItems.length
      ? [
          {
            ...trackWithoutTransitions,
            id: `${namespace}:${track.id}:${trackIndex}`,
            items: ownItems,
            ...(carriedTransitions.length > 0 ? { transitions: carriedTransitions } : {}),
          },
        ]
      : [];
    return [...ownTrack, ...nestedTracks];
  });

  return {
    durationSeconds: Math.min(timeline.durationSeconds / playbackSpeed + offsetSeconds, endSeconds),
    tracks,
  };
}

interface NestedCanvasTransform {
  centerX: number;
  centerY: number;
  width: number;
  height: number;
  flipHorizontal: boolean;
  flipVertical: boolean;
}

function nestedCanvasTransformIdentity(): NestedCanvasTransform {
  return {
    centerX: 0.5,
    centerY: 0.5,
    width: 1,
    height: 1,
    flipHorizontal: false,
    flipVertical: false,
  };
}

function nestedTimelineWrapperProperties(item: TimelineItem): {
  volumeDb: number;
  transform: NestedCanvasTransform;
  speed: number;
} | null {
  if (
    !Object.keys(item.properties).every(
      (key) =>
        key === "opacity" ||
        key === "keyframes" ||
        key === "volumeDb" ||
        key === "transform" ||
        key === "fadeInSeconds" ||
        key === "fadeOutSeconds" ||
        key === "speed" ||
        ["positionX", "positionY", "scale", "scaleX", "scaleY", "rotationDegrees", "cropTop", "cropRight", "cropBottom", "cropLeft"].includes(key),
    )
  ) {
    return null;
  }
  if (!hasValidNestedOpacityKeyframes(item) || !hasValidNestedFades(item)) return null;
  const volumeDb = numberProperty(item, "volumeDb") ?? 0;
  if (volumeDb < -60 || volumeDb > 24) return null;
  const speed = numberProperty(item, "speed") ?? 1;
  if (speed < 0.1 || speed > 8) return null;
  const transform = nestedCanvasTransformForItem(item);
  return transform ? { volumeDb, transform, speed } : null;
}

function scaleNestedWrapperAnimationTiming(item: TimelineItem, speed: number): TimelineItem {
  if (speed === 1) return item;
  const properties = { ...item.properties };
  for (const key of ["fadeInSeconds", "fadeOutSeconds"] as const) {
    const value = numberProperty(item, key);
    if (value !== null) properties[key] = value / speed;
  }
  const keyframes = properties.keyframes;
  if (keyframes && typeof keyframes === "object" && !Array.isArray(keyframes)) {
    const opacity = (keyframes as Record<string, unknown>).opacity;
    if (Array.isArray(opacity)) {
      properties.keyframes = {
        ...keyframes,
        opacity: opacity.map((keyframe) =>
          keyframe && typeof keyframe === "object" && !Array.isArray(keyframe)
            ? { ...keyframe, atSeconds: Number((keyframe as Record<string, unknown>).atSeconds) / speed }
            : keyframe,
        ),
      };
    }
  }
  return { ...item, durationSeconds: item.durationSeconds / speed, properties };
}

function nestedCanvasTransformForItem(item: TimelineItem): NestedCanvasTransform | null {
  const value = item.properties.transform;
  if (value === undefined) return nestedCanvasTransformIdentity();
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  const transform = value as Record<string, unknown>;
  const numberValue = (key: string, fallback: number, minimum: number) => {
    const candidate = transform[key];
    const parsed = typeof candidate === "number" && Number.isFinite(candidate) ? candidate : fallback;
    return parsed >= minimum && parsed <= 1 ? parsed : null;
  };
  const centerX = numberValue("centerX", 0.5, 0);
  const centerY = numberValue("centerY", 0.5, 0);
  const width = numberValue("width", 1, Number.MIN_VALUE);
  const height = numberValue("height", 1, Number.MIN_VALUE);
  if (centerX === null || centerY === null || width === null || height === null) return null;
  return {
    centerX,
    centerY,
    width,
    height,
    flipHorizontal: transform.flipHorizontal === true,
    flipVertical: transform.flipVertical === true,
  };
}

function composeNestedCanvasTransforms(
  parent: NestedCanvasTransform,
  child: NestedCanvasTransform,
): NestedCanvasTransform {
  const childCenterX = parent.flipHorizontal ? 1 - child.centerX : child.centerX;
  const childCenterY = parent.flipVertical ? 1 - child.centerY : child.centerY;
  return {
    centerX: parent.centerX + (childCenterX - 0.5) * parent.width,
    centerY: parent.centerY + (childCenterY - 0.5) * parent.height,
    width: parent.width * child.width,
    height: parent.height * child.height,
    flipHorizontal: parent.flipHorizontal !== child.flipHorizontal,
    flipVertical: parent.flipVertical !== child.flipVertical,
  };
}

interface AbsoluteOpacityKeyframe {
  atSeconds: number;
  value: number;
}

interface AbsoluteOpacityCurve {
  base: number;
  keyframes: AbsoluteOpacityKeyframe[];
}

function hasValidNestedOpacityKeyframes(item: TimelineItem) {
  const opacity = numberProperty(item, "opacity");
  if (item.properties.opacity !== undefined && (opacity === null || opacity < 0 || opacity > 1)) {
    return false;
  }
  const keyframes = item.properties.keyframes;
  if (keyframes === undefined) return true;
  if (!keyframes || typeof keyframes !== "object" || Array.isArray(keyframes)) return false;
  if (!Object.keys(keyframes).every((key) => key === "opacity" || ["positionX", "positionY", "scale", "scaleX", "scaleY", "rotationDegrees", "cropTop", "cropRight", "cropBottom", "cropLeft"].includes(key))) return false;
  for (const [property, values] of Object.entries(keyframes)) {
    if (!Array.isArray(values)) return false;
    let previousAtSeconds = -1;
    for (const keyframe of values) {
      if (!keyframe || typeof keyframe !== "object" || Array.isArray(keyframe)) return false;
      const record = keyframe as Record<string, unknown>;
      const atSeconds = record.atSeconds;
      const value = record.value;
      const easing = record.easing ?? "linear";
      if (
        typeof atSeconds !== "number" || !Number.isFinite(atSeconds) || atSeconds < 0 ||
        atSeconds > item.durationSeconds || atSeconds <= previousAtSeconds ||
        typeof value !== "number" || !Number.isFinite(value) ||
        !["linear", "hold", "easeIn", "easeOut", "easeInOut", "smooth"].includes(String(easing)) ||
        (property === "opacity" && (value < 0 || value > 1)) ||
        (property.startsWith("crop") && (value < 0 || value > 1)) ||
        ((property === "scale" || property === "scaleX" || property === "scaleY") && value <= 0)
      ) return false;
      previousAtSeconds = atSeconds;
    }
  }
  return true;
}

function hasValidNestedFades(item: TimelineItem) {
  const configuredFadeInSeconds = numberProperty(item, "fadeInSeconds");
  const configuredFadeOutSeconds = numberProperty(item, "fadeOutSeconds");
  if (
    (item.properties.fadeInSeconds !== undefined && configuredFadeInSeconds === null) ||
    (item.properties.fadeOutSeconds !== undefined && configuredFadeOutSeconds === null)
  ) {
    return false;
  }
  const fadeInSeconds = configuredFadeInSeconds ?? 0;
  const fadeOutSeconds = configuredFadeOutSeconds ?? 0;
  return (
    fadeInSeconds >= 0 &&
    fadeOutSeconds >= 0 &&
    fadeInSeconds + fadeOutSeconds <= item.durationSeconds
  );
}

function opacityCurveForItem(
  item: TimelineItem,
  absoluteStartSeconds: number,
  sourceDurationSeconds: number,
): AbsoluteOpacityCurve {
  const base = numberProperty(item, "opacity") ?? 1;
  const candidateKeyframes = item.properties.keyframes;
  const opacityKeyframes =
    candidateKeyframes && typeof candidateKeyframes === "object" && !Array.isArray(candidateKeyframes)
      ? (candidateKeyframes as Record<string, unknown>).opacity
      : undefined;
  if (!Array.isArray(opacityKeyframes)) return { base, keyframes: [] };
  let previousAtSeconds = -1;
  const keyframes: AbsoluteOpacityKeyframe[] = [];
  for (const keyframe of opacityKeyframes) {
    if (!keyframe || typeof keyframe !== "object" || Array.isArray(keyframe)) {
      return { base, keyframes: [] };
    }
    const atSeconds = (keyframe as Record<string, unknown>).atSeconds;
    const value = (keyframe as Record<string, unknown>).value;
    if (
      typeof atSeconds !== "number" ||
      !Number.isFinite(atSeconds) ||
      atSeconds < 0 ||
      atSeconds > sourceDurationSeconds ||
      atSeconds <= previousAtSeconds ||
      typeof value !== "number" ||
      !Number.isFinite(value) ||
      value < 0 ||
      value > 1
    ) {
      return { base, keyframes: [] };
    }
    previousAtSeconds = atSeconds;
    keyframes.push({ atSeconds: absoluteStartSeconds + atSeconds, value });
  }
  return { base, keyframes };
}

function fadeOpacityCurveForItem(
  item: TimelineItem,
  absoluteStartSeconds: number,
  durationSeconds: number,
): AbsoluteOpacityCurve | null {
  const fadeInSeconds = numberProperty(item, "fadeInSeconds") ?? 0;
  const fadeOutSeconds = numberProperty(item, "fadeOutSeconds") ?? 0;
  if (fadeInSeconds === 0 && fadeOutSeconds === 0) return null;
  const keyframes: AbsoluteOpacityKeyframe[] = [];
  if (fadeInSeconds > 0) {
    keyframes.push({ atSeconds: absoluteStartSeconds, value: 0 });
    keyframes.push({ atSeconds: absoluteStartSeconds + fadeInSeconds, value: 1 });
  }
  if (fadeOutSeconds > 0) {
    const fadeOutStart = absoluteStartSeconds + durationSeconds - fadeOutSeconds;
    if (keyframes.at(-1)?.atSeconds !== fadeOutStart) {
      keyframes.push({ atSeconds: fadeOutStart, value: 1 });
    }
    keyframes.push({ atSeconds: absoluteStartSeconds + durationSeconds, value: 0 });
  }
  return { base: 1, keyframes };
}

function opacityValueAt(curve: AbsoluteOpacityCurve, seconds: number) {
  const first = curve.keyframes[0];
  if (!first || seconds < first.atSeconds) return curve.base;
  for (let index = 0; index < curve.keyframes.length - 1; index += 1) {
    const current = curve.keyframes[index];
    const next = curve.keyframes[index + 1];
    if (!current || !next) continue;
    if (seconds <= next.atSeconds) {
      const progress = (seconds - current.atSeconds) / (next.atSeconds - current.atSeconds);
      return current.value + (next.value - current.value) * progress;
    }
  }
  return curve.keyframes.at(-1)?.value ?? curve.base;
}

function composeNestedOpacityCurves(
  item: TimelineItem,
  parentCurves: AbsoluteOpacityCurve[],
  absoluteStartSeconds: number,
  absoluteEndSeconds: number,
): TimelineItem {
  if (parentCurves.length === 0 || item.kind === "audio_clip") return item;
  const curves = [
    ...parentCurves,
    opacityCurveForItem(item, absoluteStartSeconds, item.durationSeconds),
  ];
  const opacityAt = (seconds: number) =>
    Math.max(0, Math.min(1, curves.reduce((value, curve) => value * opacityValueAt(curve, seconds), 1)));
  const hasKeyframes = curves.some((curve) => curve.keyframes.length > 0);
  const properties: Record<string, unknown> = {
    ...item.properties,
    opacity: opacityAt(absoluteStartSeconds),
  };
  if (!hasKeyframes) return { ...item, properties };

  const times = new Set<number>([absoluteStartSeconds, absoluteEndSeconds]);
  for (const curve of curves) {
    for (const keyframe of curve.keyframes) {
      if (keyframe.atSeconds >= absoluteStartSeconds && keyframe.atSeconds <= absoluteEndSeconds) {
        times.add(keyframe.atSeconds);
      }
    }
  }
  const startFrame = Math.ceil(absoluteStartSeconds * PREVIEW_OPACITY_SAMPLE_FPS);
  const endFrame = Math.floor(absoluteEndSeconds * PREVIEW_OPACITY_SAMPLE_FPS);
  for (let frame = startFrame; frame <= endFrame; frame += 1) {
    times.add(frame / PREVIEW_OPACITY_SAMPLE_FPS);
  }
  const keyframes = {
    ...(properties.keyframes && typeof properties.keyframes === "object" && !Array.isArray(properties.keyframes)
      ? properties.keyframes
      : {}),
    opacity: [...times]
      .sort((left, right) => left - right)
      .map((atSeconds) => ({
        atSeconds: atSeconds - absoluteStartSeconds,
        value: opacityAt(atSeconds),
      })),
  };
  return { ...item, properties: { ...properties, keyframes } };
}

function composeNestedAudioGain(item: TimelineItem, parentVolumeDb: number): TimelineItem {
  if (parentVolumeDb === 0 || item.kind !== "audio_clip") return item;
  const volumeDb = (numberProperty(item, "volumeDb") ?? 0) + parentVolumeDb;
  if (volumeDb < -60 || volumeDb > 24) return item;
  return {
    ...item,
    properties: { ...item.properties, volumeDb },
  };
}

function composeNestedMotionWrappers(
  item: TimelineItem,
  wrappers: readonly TimelineItem[],
  absoluteStart: number,
  absoluteEnd: number,
): TimelineItem {
  if (wrappers.length === 0 || item.kind === "audio_clip") return item;
  const properties = { ...item.properties };
  const keyframes = properties.keyframes && typeof properties.keyframes === "object" && !Array.isArray(properties.keyframes)
    ? { ...properties.keyframes as Record<string, unknown> } : {};
  const motionProperties = ["positionX", "positionY", "rotationDegrees", "scaleX", "scaleY", "cropTop", "cropRight", "cropBottom", "cropLeft"] as const;
  const firstFrame = Math.ceil(absoluteStart * PREVIEW_OPACITY_SAMPLE_FPS);
  const lastFrame = Math.floor(absoluteEnd * PREVIEW_OPACITY_SAMPLE_FPS);
  let animatedTimes: number[] | undefined;
  for (const property of motionProperties) {
    const childFallback = property === "scaleX" || property === "scaleY"
      ? numberProperty(item, property) ?? numberProperty(item, "scale") ?? 1
      : numberProperty(item, property) ?? 0;
    const hasAnimation = [item, ...wrappers].some((candidate) => {
      const values = candidate.properties.keyframes;
      return values && typeof values === "object" && !Array.isArray(values) && Array.isArray((values as Record<string, unknown>)[property]) && ((values as Record<string, unknown>)[property] as unknown[]).length > 0;
    });
    // Static channels previously repeated the same rounded value 120 times/second.
    // One identical sample preserves clamping/rounding without those allocations.
    if (hasAnimation && !animatedTimes) {
      animatedTimes = [];
      for (let frame = firstFrame; frame <= lastFrame; frame += 1) animatedTimes.push(frame / PREVIEW_OPACITY_SAMPLE_FPS);
    }
    const times = hasAnimation ? animatedTimes! : firstFrame <= lastFrame ? [firstFrame / PREVIEW_OPACITY_SAMPLE_FPS] : [];
    keyframes[property] = times.map((absolute) => {
      const local = absolute - absoluteStart;
      let value = keyframedNumberProperty(item, property, local, childFallback);
      for (const wrapper of wrappers) {
        const fallback = property === "scaleX" || property === "scaleY"
          ? numberProperty(wrapper, property) ?? numberProperty(wrapper, "scale") ?? 1
          : numberProperty(wrapper, property) ?? 0;
        const parent = keyframedNumberProperty(wrapper, property, absolute - wrapper.startSeconds, fallback);
        value = property === "scaleX" || property === "scaleY" ? value * parent
          : property.startsWith("crop") ? Math.max(0, Math.min(1, parent + value * (1 - parent)))
          : value + parent;
      }
      return { atSeconds: local, value: roundSeconds(value), easing: "linear" };
    });
  }
  delete properties.scale;
  properties.keyframes = keyframes;
  return { ...item, properties };
}

function composeNestedCanvasTransform(
  item: TimelineItem,
  parentTransform: NestedCanvasTransform,
): TimelineItem {
  if (
    item.kind === "audio_clip" ||
    (parentTransform.centerX === 0.5 &&
      parentTransform.centerY === 0.5 &&
      parentTransform.width === 1 &&
      parentTransform.height === 1 &&
      !parentTransform.flipHorizontal &&
      !parentTransform.flipVertical)
  ) {
    return item;
  }
  const childTransform = nestedCanvasTransformForItem(item);
  if (!childTransform) return item;
  const transform = composeNestedCanvasTransforms(parentTransform, childTransform);
  return { ...item, properties: { ...item.properties, transform } };
}

function resolveTimelinePreviewSource(
  item: TimelineItem,
  mediaById: Map<string, MediaAsset>,
  generatedAssetsById: Map<string, GeneratedAsset>,
): ResolvedPreviewSource {
  if (item.source.type === "media") {
    return resolveMediaPreviewSource(item.source.mediaId, mediaById);
  }
  if (item.source.type === "generated") {
    return resolveGeneratedPreviewSource(item.source.artifactId, generatedAssetsById, mediaById);
  }
  return {
    status: "unsupported",
    issue: (itemId) => `Timeline item ${itemId} has no media preview source.`,
  };
}

type ResolvedPreviewSource =
  | {
      status: "ready";
      asset: Pick<MediaAsset, "id" | "kind" | "relativePath" | "durationSeconds">;
    }
  | {
      status: "missing";
      issue: (itemId: string) => string;
    }
  | {
      status: "unsupported";
      issue: (itemId: string) => string;
    };

function resolveMediaPreviewSource(
  mediaId: string,
  mediaById: Map<string, MediaAsset>,
): ResolvedPreviewSource {
  const asset = mediaById.get(mediaId);
  if (!asset) {
    return {
      status: "missing",
      issue: (itemId) => `Timeline item ${itemId} references missing media ${mediaId}.`,
    };
  }

  return { status: "ready", asset };
}

function resolveGeneratedPreviewSource(
  artifactId: string,
  generatedAssetsById: Map<string, GeneratedAsset>,
  mediaById: Map<string, MediaAsset>,
): ResolvedPreviewSource {
  const generatedAsset = generatedAssetsById.get(artifactId);
  if (!generatedAsset) {
    return {
      status: "missing",
      issue: (itemId) =>
        `Timeline item ${itemId} references missing generated asset ${artifactId}.`,
    };
  }
  if (generatedAsset.status !== "completed") {
    return {
      status: "unsupported",
      issue: (itemId) =>
        `Timeline item ${itemId} generated asset ${artifactId} is not completed.`,
    };
  }

  const output = generatedAsset.outputs[0];
  if (!output) {
    return {
      status: "unsupported",
      issue: (itemId) =>
        `Timeline item ${itemId} generated asset ${artifactId} has no preview output.`,
    };
  }

  const mediaAsset = mediaById.get(output.mediaId);
  return {
    status: "ready",
    asset: {
      id: output.mediaId,
      kind: mediaAsset?.kind ?? generatedAsset.kind,
      relativePath: mediaAsset?.relativePath ?? output.relativePath,
      durationSeconds: mediaAsset?.durationSeconds ?? output.durationSeconds,
    },
  };
}

function isTimelinePreviewItemActiveAt(
  item: TimelineItem,
  playheadSeconds: number,
) {
  return isActiveAt(item, playheadSeconds);
}

function isActiveAt(item: TimelineItem, playheadSeconds: number) {
  return (
    playheadSeconds >= item.startSeconds &&
    playheadSeconds < item.startSeconds + item.durationSeconds
  );
}

function numberProperty(item: TimelineItem, key: string) {
  const value = item.properties[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function previewCanvasTransformForItem(item: TimelineItem) {
  const transform = item.properties.transform;
  if (!transform || typeof transform !== "object" || Array.isArray(transform)) {
    return {};
  }
  const values = transform as Record<string, unknown>;
  const numberValue = (key: string, fallback: number) => {
    const value = values[key];
    return typeof value === "number" && Number.isFinite(value) ? value : fallback;
  };
  return {
    centerX: numberValue("centerX", 0.5),
    centerY: numberValue("centerY", 0.5),
    width: numberValue("width", 1),
    height: numberValue("height", 1),
    flipHorizontal: values.flipHorizontal === true,
    flipVertical: values.flipVertical === true,
  };
}

function previewCropForItem(item: TimelineItem) {
  const cropValue = (key: string) => {
    const value = numberProperty(item, key);
    return value !== null && value >= 0 && value < 1 ? value : 0;
  };
  return {
    cropTop: cropValue("cropTop"),
    cropRight: cropValue("cropRight"),
    cropBottom: cropValue("cropBottom"),
    cropLeft: cropValue("cropLeft"),
  };
}

function previewSourceTimeForItem(
  item: TimelineItem,
  mediaDurationSeconds: number,
  localSeconds: number,
  playbackSpeed: number,
) {
  const sourceIn = numberProperty(item, "sourceIn") ?? 0;
  const sourceOut = numberProperty(item, "sourceOut");
  const maxSourceTime =
    sourceOut !== null
      ? sourceOut
      : mediaDurationSeconds > 0
        ? mediaDurationSeconds
        : Number.POSITIVE_INFINITY;
  const unclampedSeconds = sourceSecondsAt(
    {
      sourceIn,
      sourceOut: sourceOut ?? sourceIn + item.durationSeconds * playbackSpeed,
      speed: playbackSpeed,
      reverse: isReversedItem(item),
    },
    localSeconds,
  );
  const seconds = Math.max(0, Math.min(unclampedSeconds, maxSourceTime));
  return {
    seconds: roundSeconds(seconds),
    unclampedSeconds,
    wasClamped: seconds !== unclampedSeconds,
  };
}

function previewPlaybackSpeedForItem(item: TimelineItem) {
  const speed = numberProperty(item, "speed");
  return speed !== null && speed > 0 ? speed : 1;
}

function previewAudioGainForItem(item: TimelineItem, localSeconds: number) {
  const volumeDb = keyframedNumberProperty(
    item,
    "volumeDb",
    localSeconds,
    numberProperty(item, "volumeDb") ?? 0,
  );
  const fadeInSeconds = Math.max(0, numberProperty(item, "fadeInSeconds") ?? 0);
  const fadeOutSeconds = Math.max(0, numberProperty(item, "fadeOutSeconds") ?? 0);
  const fadeInProgress =
    fadeInSeconds > 0 ? Math.max(0, Math.min(1, localSeconds / fadeInSeconds)) : 1;
  const fadeOutProgress =
    fadeOutSeconds > 0
      ? Math.max(0, Math.min(1, (item.durationSeconds - localSeconds) / fadeOutSeconds))
      : 1;
  return roundSeconds(10 ** (volumeDb / 20) * fadeInProgress * fadeOutProgress);
}

function stringProperty(item: TimelineItem, key: string) {
  const value = item.properties[key];
  return typeof value === "string" && value.trim().length > 0 ? value : null;
}

/**
 * Motion at `localSeconds`. Like the renderer's frame program, keyframes win, then the static item
 * property, then the identity default; per-axis scale falls back to the uniform scale.
 */
function previewMotionForItem(item: TimelineItem, localSeconds: number) {
  const motionValue = (property: string, fallback: number) =>
    keyframedNumberProperty(item, property, localSeconds, numberProperty(item, property) ?? fallback);
  const scale = motionValue("scale", 1);
  const fadeInSeconds = Math.max(0, numberProperty(item, "fadeInSeconds") ?? 0);
  const fadeOutSeconds = Math.max(0, numberProperty(item, "fadeOutSeconds") ?? 0);
  // Clamped below too: a transition's incoming clip is sampled before its canonical start.
  const fadeInProgress = fadeInSeconds > 0 ? Math.max(0, Math.min(1, localSeconds / fadeInSeconds)) : 1;
  const fadeOutProgress =
    fadeOutSeconds > 0
      ? Math.max(0, Math.min(1, (item.durationSeconds - localSeconds) / fadeOutSeconds))
      : 1;
  return {
    opacity:
      keyframedNumberProperty(
        item,
        "opacity",
        localSeconds,
        numberProperty(item, "opacity") ?? 1,
      ) * fadeInProgress * fadeOutProgress,
    positionX: motionValue("positionX", 0),
    positionY: motionValue("positionY", 0),
    scale,
    scaleX: motionValue("scaleX", scale),
    scaleY: motionValue("scaleY", scale),
    rotationDegrees: motionValue("rotationDegrees", 0),
  };
}

function previewColorGradeForItem(item: TimelineItem): TimelinePreviewColorGrade {
  const grade = item.properties.colorGrade;
  const value = (key: string, fallback: number, minimum: number, maximum: number) => {
    if (!grade || typeof grade !== "object" || Array.isArray(grade)) {
      return fallback;
    }
    const candidate = (grade as Record<string, unknown>)[key];
    return typeof candidate === "number" && Number.isFinite(candidate)
      ? Math.max(minimum, Math.min(maximum, candidate))
      : fallback;
  };
  return {
    exposure: value("exposure", 0, -3, 3),
    contrast: value("contrast", 1, 0.5, 1.5),
    saturation: value("saturation", 1, 0, 2),
  };
}

function keyframedNumberProperty(
  item: TimelineItem,
  property: string,
  localSeconds: number,
  fallback: number,
) {
  const keyframes = item.properties.keyframes;
  if (!keyframes || typeof keyframes !== "object" || Array.isArray(keyframes)) {
    return fallback;
  }
  const propertyKeyframes = (keyframes as Record<string, unknown>)[property];
  if (!Array.isArray(propertyKeyframes) || propertyKeyframes.length === 0) {
    return fallback;
  }

  const sorted = propertyKeyframes
    .map((keyframe) => {
      if (!keyframe || typeof keyframe !== "object" || Array.isArray(keyframe)) {
        return null;
      }
      const atSeconds = (keyframe as Record<string, unknown>).atSeconds;
      const value = (keyframe as Record<string, unknown>).value;
      const easing = (keyframe as Record<string, unknown>).easing;
      return typeof atSeconds === "number" &&
        Number.isFinite(atSeconds) &&
        typeof value === "number" &&
        Number.isFinite(value)
        ? { atSeconds, value, easing: typeof easing === "string" ? easing : "linear" }
        : null;
    })
    .filter((keyframe): keyframe is { atSeconds: number; value: number; easing: string } => keyframe !== null)
    .sort((left, right) => left.atSeconds - right.atSeconds);

  if (sorted.length === 0) {
    return fallback;
  }
  const first = sorted[0];
  if (!first) return fallback;
  if (localSeconds <= first.atSeconds) {
    return roundSeconds(first.value);
  }
  const last = sorted.at(-1);
  if (!last || localSeconds >= last.atSeconds) {
    return roundSeconds(last?.value ?? fallback);
  }

  for (let index = 0; index < sorted.length - 1; index += 1) {
    const current = sorted[index];
    const next = sorted[index + 1];
    if (!current || !next) continue;
    if (localSeconds >= current.atSeconds && localSeconds <= next.atSeconds) {
      const span = next.atSeconds - current.atSeconds;
      if (span <= 0) {
        return roundSeconds(current.value);
      }
      const progress = (localSeconds - current.atSeconds) / span;
      const eased = current.easing === "hold" ? 0
        : current.easing === "easeIn" ? progress * progress
        : current.easing === "easeOut" ? 1 - (1 - progress) ** 2
        : current.easing === "easeInOut" || current.easing === "smooth" ? progress * progress * (3 - 2 * progress)
        : progress;
      return roundSeconds(current.value + (next.value - current.value) * eased);
    }
  }

  return fallback;
}

function buildOverlayLayer(
  item: TimelineItem,
  playheadSeconds: number,
): TimelinePreviewOverlayLayer | null {
  if (!isActiveAt(item, playheadSeconds)) {
    return null;
  }

  const templateId = previewTemplateIdForItem(item);
  if (templateId) {
    const motion = previewMotionForItem(item, playheadSeconds - item.startSeconds);
    return {
      itemId: item.id,
      label: item.label,
      overlayKind: "template",
      text: null,
      templateId,
      timelineStartSeconds: item.startSeconds,
      timelineEndSeconds: item.startSeconds + item.durationSeconds,
      opacity: motion.opacity,
      positionX: motion.positionX,
      positionY: motion.positionY,
      scale: motion.scale,
      scaleX: motion.scaleX,
      scaleY: motion.scaleY,
      rotationDegrees: motion.rotationDegrees,
    };
  }

  if (item.kind !== "caption" && item.kind !== "overlay") {
    return null;
  }

  const motion = previewMotionForItem(item, playheadSeconds - item.startSeconds);
  return {
    itemId: item.id,
    label: item.label,
    overlayKind: item.kind === "caption" ? "caption" : "text",
    text: item.source.type === "text" ? item.source.text : item.label,
    templateId: null,
    timelineStartSeconds: item.startSeconds,
    timelineEndSeconds: item.startSeconds + item.durationSeconds,
    opacity: motion.opacity,
    positionX: motion.positionX,
    positionY: motion.positionY,
    scale: motion.scale,
    scaleX: motion.scaleX,
    scaleY: motion.scaleY,
    rotationDegrees: motion.rotationDegrees,
    ...(item.kind === "caption"
      ? {
          captionPlacement: previewCaptionPlacement(item),
          captionStylePreset: previewCaptionStylePreset(item),
          emphasizedWordIndices: previewCaptionEmphasizedWordIndices(item),
          activeEmphasizedWordIndices: previewActiveCaptionEmphasizedWordIndices(
            item,
            playheadSeconds - item.startSeconds,
          ),
          captionWordStyles: previewCaptionWordStyles(item, playheadSeconds - item.startSeconds),
        }
      : {}),
  };
}

function previewCaptionWordStyles(item: TimelineItem, elapsedSeconds: number): TimelinePreviewCaptionWordStyle[] {
  const values = item.properties.captionWordAnimations;
  if (!Array.isArray(values)) return [];
  return values.flatMap((value): TimelinePreviewCaptionWordStyle[] => {
    if (!value || typeof value !== "object" || Array.isArray(value)) return [];
    const record = value as Record<string, unknown>;
    const wordIndex = record.wordIndex;
    const enterStart = record.enterStartSeconds;
    const enterEnd = record.enterEndSeconds;
    const holdEnd = record.holdEndSeconds;
    const exitEnd = record.exitEndSeconds;
    const targetScale = record.emphasisScale;
    const targetOpacity = record.emphasisOpacity;
    const color = record.emphasisColor;
    if (typeof wordIndex !== "number" || !Number.isInteger(wordIndex) ||
      typeof enterStart !== "number" || typeof enterEnd !== "number" || typeof holdEnd !== "number" || typeof exitEnd !== "number" ||
      typeof targetScale !== "number" || typeof targetOpacity !== "number" || typeof color !== "string" ||
      !(0 <= enterStart && enterStart <= enterEnd && enterEnd <= holdEnd && holdEnd <= exitEnd && exitEnd <= item.durationSeconds)) return [];
    let progress = 0;
    if (elapsedSeconds >= enterStart && elapsedSeconds <= exitEnd) {
      if (elapsedSeconds < enterEnd) progress = (elapsedSeconds - enterStart) / Math.max(0.001, enterEnd - enterStart);
      else if (elapsedSeconds <= holdEnd) progress = 1;
      else progress = 1 - (elapsedSeconds - holdEnd) / Math.max(0.001, exitEnd - holdEnd);
    }
    if (progress <= 0) return [];
    const eased = record.easing === "linear" ? progress : record.easing === "outBack"
      ? 1 + 2.70158 * Math.pow(progress - 1, 3) + 1.70158 * Math.pow(progress - 1, 2)
      : 1 - Math.pow(1 - progress, 2);
    return [{ wordIndex, scale: 1 + (targetScale - 1) * eased, opacity: targetOpacity * Math.max(0, progress), color }];
  });
}

function previewCaptionPlacement(item: TimelineItem): TimelinePreviewCaptionPlacement {
  const placement = stringProperty(item, "captionPlacement");
  return placement === "center" || placement === "upper" ? placement : "lower";
}

function previewCaptionStylePreset(item: TimelineItem): TimelinePreviewCaptionStylePreset {
  const stylePreset = stringProperty(item, "stylePreset");
  return stylePreset === "kineticFocus" || stylePreset === "centeredMinimal"
    ? stylePreset
    : "boldReadableLower";
}

function previewCaptionEmphasizedWordIndices(item: TimelineItem): number[] {
  const text = item.source.type === "text" ? item.source.text : item.label;
  const wordCount = text.match(/\S+/g)?.length ?? 0;
  const values = item.properties.emphasizedWordIndices;
  if (!Array.isArray(values)) return [];
  return [...new Set(values)].filter(
    (value): value is number =>
      typeof value === "number" && Number.isInteger(value) && value >= 0 && value < wordCount,
  );
}

function previewActiveCaptionEmphasizedWordIndices(
  item: TimelineItem,
  elapsedSeconds: number,
): number[] {
  const emphasizedWordIndices = previewCaptionEmphasizedWordIndices(item);
  const timings = item.properties.captionWordTimings;
  if (!Array.isArray(timings)) return emphasizedWordIndices;
  const active = new Set<number>();
  for (const timing of timings) {
    if (
      timing &&
      typeof timing === "object" &&
      "wordIndex" in timing &&
      "startSeconds" in timing &&
      "endSeconds" in timing &&
      typeof timing.wordIndex === "number" &&
      typeof timing.startSeconds === "number" &&
      typeof timing.endSeconds === "number" &&
      emphasizedWordIndices.includes(timing.wordIndex) &&
      elapsedSeconds >= timing.startSeconds &&
      elapsedSeconds <= timing.endSeconds
    ) {
      active.add(timing.wordIndex);
    }
  }
  return [...active];
}

function previewTemplateIdForItem(item: TimelineItem) {
  const explicitTemplateId = stringProperty(item, "templateId");
  if (explicitTemplateId) {
    return explicitTemplateId;
  }
  if (item.kind !== "hyperframe_scene") {
    return null;
  }

  switch (stringProperty(item, "kind")) {
    case "title_card":
      return "chapter-card-v1";
    case "lower_third":
      return "kinetic-lower-third-v1";
    case "diagram":
      return "metric-callout-v1";
    case "transition":
      return "gradient-background-loop-v1";
    case "immersive_scene":
      return immersiveSceneTemplateId(item);
    default:
      return null;
  }
}

function immersiveSceneTemplateId(item: TimelineItem) {
  const cues = sceneCueText([
    item.label,
    stringProperty(item, "sourceBeat"),
    stringProperty(item, "role"),
    stringProperty(item, "visualTreatment"),
    stringProperty(item, "motion"),
    stringProperty(item, "safeZone"),
    ...stringFieldValues(item, "fields"),
  ]);

  if (sceneCuesContainAny(cues, ["logo", "brand", "wordmark", "identity"])) {
    return "holographic-logo-cutout-v1";
  }
  if (
    sceneCuesContainAny(cues, [
      "metric",
      "data",
      "kpi",
      "retention",
      "conversion",
      "revenue",
      "percent",
      "percentage",
      "growth",
      "pricing",
      "price",
      "cost",
      "savings",
      "roi",
      "margin",
      "spend",
      "budget",
      "comparison",
      "compare",
      "versus",
      "before vs after",
      "split comparison",
    ])
  ) {
    return "metric-callout-v1";
  }
  if (
    sceneCuesContainAny(
      cues,
      ["risk", "warning", "alert", "security", "compliance", "failure", "blocked", "urgent"],
    )
  ) {
    return "punchy-caption-v1";
  }
  if (
    sceneCuesContainAny(cues, [
      "process",
      "roadmap",
      "workflow",
      "steps",
      "step",
      "sequence",
      "milestone",
      "rollout",
      "journey",
      "timeline",
      "schedule",
      "deadline",
      "calendar",
      "due date",
    ])
  ) {
    return "chapter-card-v1";
  }
  if (
    sceneCuesContainAny(cues, [
      "quote",
      "quoted",
      "emphasis",
      "emphasize",
      "punch",
      "punchy",
      "caption",
      "key line",
      "hook line",
      "testimonial",
      "interview",
      "speaker",
      "reaction",
      "emotional",
      "emotion",
      "surprised",
      "surprise",
      "wow",
    ])
  ) {
    return "punchy-caption-v1";
  }
  if (
    sceneCuesContainAny(cues, [
      "chapter",
      "story",
      "story beat",
      "reset",
      "section",
      "act break",
      "setup",
      "payoff",
      "proof segment",
      "launch",
      "announcement",
      "announce",
      "release",
    ])
  ) {
    return "chapter-card-v1";
  }
  if (
    sceneCuesContainAny(cues, [
      "callout",
      "detail",
      "tracking",
      "highlight",
      "pointer",
      "look here",
      "watch this",
      "product",
      "feature",
      "spec",
      "leader line",
      "lens",
      "map",
      "route",
      "location",
      "place marker",
      "map pin",
      "arrival",
      "venue",
      "city map",
    ])
  ) {
    return "tracking-highlight-v1";
  }
  return "gradient-background-loop-v1";
}

function sceneCueText(values: Array<string | null | undefined>) {
  const normalized = normalizeSceneCueText(
    values.filter((value): value is string => Boolean(value)).join(" "),
  );
  return {
    normalized,
    tokens: new Set(normalized.split(/\s+/).filter(Boolean)),
  };
}

function sceneCuesContainAny(cues: ReturnType<typeof sceneCueText>, keywords: string[]) {
  return keywords.some((keyword) => {
    const normalizedKeyword = normalizeSceneCueText(keyword);
    if (normalizedKeyword.includes(" ")) {
      return cues.normalized.includes(normalizedKeyword);
    }
    return cues.tokens.has(normalizedKeyword);
  });
}

function normalizeSceneCueText(value: string) {
  return value
    .toLowerCase()
    .split(/[^a-z0-9]+/)
    .filter(Boolean)
    .join(" ");
}

function stringFieldValues(item: TimelineItem, key: string) {
  const value = item.properties[key];
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return [];
  }
  return Object.values(value).filter(
    (fieldValue): fieldValue is string =>
      typeof fieldValue === "string" && fieldValue.trim().length > 0,
  );
}

function roundSeconds(value: number) {
  return Math.round(value * 1000) / 1000;
}
