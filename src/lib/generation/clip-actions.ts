import { generatedAssetTitleWithPrompt, generatedReferenceMediaIds, sortGeneratedAssetsByCreatedAtDesc } from "@/lib/generation/assets";
import { formatGenerationCreditEstimate, selectedGenerationCost } from "@/lib/generation/pricing";
import { generationModelOptions, generationModelValue, generationModeFromAsset } from "@/lib/generation/provider-rules";
import { upscaleGenerationRequest, videoAudioGenerationRequest } from "@/lib/generation/requests";
import {
  boundedGenerationDurationValue,
  generationDurationOptionsForModel,
  generationDurationValue,
  generationResolutionOptionsForModel,
  generationResolutionValue,
} from "@/lib/generation/settings-options";
import type {
  GenerationModelCatalog,
  GenerationModelOption,
  MediaGenerationRequest,
  SourceClipUpscaleContext,
  SourceClipVideoAudioContext,
  SourceClipVideoAudioKind,
} from "@/lib/generation/types";
import { defaultVariationDrafts, validVariationDrafts, type GenerationVariationDraft } from "@/lib/generation-variations";
import { mediaContentKind } from "@/lib/media/media-filters";
import type { GeneratedAsset, MediaAsset, VideoProject } from "@/lib/project";
import type { TimelineItemKind } from "@/lib/timeline";

/** A generation request ready to start, or the reason it cannot be built. */
export type RequestPlan = { readonly request: MediaGenerationRequest } | { readonly blocked: string };

/**
 * Pre-cut `queueMediaUpscale` guard (media exists and is not audio). Generated media is upscaled by
 * its content type, so a generated video uses the video upscaler rather than the image one.
 */
export function upscaleRequestPlan(project: VideoProject, mediaId: string, context?: SourceClipUpscaleContext): RequestPlan {
  const media = project.media.find((candidate) => candidate.id === mediaId);
  if (!media) return { blocked: "That media is no longer in the project." };
  const kind = mediaContentKind(media);
  if (kind !== "image" && kind !== "video") return { blocked: "Only images and videos can be upscaled." };
  return { request: upscaleGenerationRequest({ ...media, kind }, context) };
}

/** Pre-cut `queueVideoAudioGeneration` guard: the media must be a video (generated videos included). */
export function videoAudioRequestPlan(
  project: VideoProject,
  mediaId: string,
  kind: SourceClipVideoAudioKind,
  context: SourceClipVideoAudioContext,
): RequestPlan {
  const media = project.media.find((candidate) => candidate.id === mediaId);
  if (!media) return { blocked: "That media is no longer in the project." };
  if (mediaContentKind(media) !== "video") return { blocked: "Music and sound effects need a video clip." };
  return { request: videoAudioGenerationRequest(media, kind, context) };
}

/**
 * The runs "Create variations" queues: one run of the original prompt (pre-cut
 * `queueGeneratedClipVariation`), or the first `count` default drafts as a variation set.
 */
export function variationDraftsForAsset(asset: GeneratedAsset, count: number): GenerationVariationDraft[] {
  if (count <= 1) return asset.prompt.trim() ? [{ name: asset.name?.trim() ?? "", prompt: asset.prompt }] : [];
  return validVariationDrafts(defaultVariationDrafts(asset.prompt).slice(0, count));
}

type CostInput = Pick<MediaGenerationRequest, "kind" | "model" | "prompt" | "settings">;

function pricedModel(input: CostInput, catalog: GenerationModelCatalog | null): GenerationModelOption {
  const mode = generationModeFromAsset(input);
  const value = generationModelValue(input.model);
  const candidates = [...(catalog?.[mode] ?? []), ...(catalog?.upscale ?? []), ...generationModelOptions[mode]];
  return candidates.find((model) => generationModelValue(model) === value) ?? input.model;
}

/** The credit estimate for `count` runs of a request's model and settings, e.g. "Est. 8 credits". */
export function generationCostLabel(input: CostInput, catalog: GenerationModelCatalog | null, count = 1): string {
  const mode = generationModeFromAsset(input);
  const model = pricedModel(input, catalog);
  const { settings } = input;
  const duration =
    generationDurationValue(settings.durationSeconds, generationDurationOptionsForModel(mode, model)) ??
    boundedGenerationDurationValue(settings.durationSeconds) ??
    "";
  const resolution = settings.resolution ?? generationResolutionValue(settings.width, settings.height, generationResolutionOptionsForModel(mode, model)) ?? "";
  const cost = selectedGenerationCost(mode, model, duration, resolution, String(settings.numImages ?? 1), settings.generateAudio ?? true, input.prompt, settings.quality ?? "");
  return formatGenerationCreditEstimate(cost === "varies" ? cost : cost * Math.max(1, count));
}

function assetTitle(project: VideoProject, assetId: string): string {
  const found = project.generatedAssets.find((candidate) => candidate.id === assetId);
  const title = found && (found.name?.trim() || found.prompt.trim()) ? generatedAssetTitleWithPrompt(found) : null;
  return title ? `“${title}”` : "an earlier generation";
}

/** Human lineage ("Variation of “Hero shot”"); null for an original generation. */
export function generatedLineageText(project: VideoProject, asset: GeneratedAsset): string | null {
  const parts: string[] = [];
  if (asset.parentAssetId) parts.push(`Variation of ${assetTitle(project, asset.parentAssetId)}`);
  if (asset.retryOfAssetId && asset.retryOfAssetId !== asset.parentAssetId) {
    parts.push(`${parts.length > 0 ? "based" : "Based"} on ${assetTitle(project, asset.retryOfAssetId)}`);
  }
  return parts.length > 0 ? parts.join(" · ") : null;
}

export interface GenerationReferenceTile {
  readonly role: "First frame" | "Last frame" | "Source video" | "Reference";
  readonly mediaId: string;
  /** Null when the media left the project. */
  readonly media: MediaAsset | null;
}

/** The media a generation used: frames, source video, then the remaining references, each once. */
export function generationReferenceTiles(project: VideoProject, asset: GeneratedAsset): GenerationReferenceTile[] {
  const { references } = asset;
  const entries: [GenerationReferenceTile["role"], string | null | undefined][] = [
    ["First frame", references.firstFrameMediaId],
    ["Last frame", references.lastFrameMediaId],
    ["Source video", references.sourceVideoMediaRef],
  ];
  const tiles: GenerationReferenceTile[] = [];
  const add = (role: GenerationReferenceTile["role"], mediaId: string) =>
    tiles.push({ role, mediaId, media: project.media.find((media) => media.id === mediaId) ?? null });
  for (const [role, mediaId] of entries) if (mediaId) add(role, mediaId);
  const shown = new Set(tiles.map((tile) => tile.mediaId));
  for (const mediaId of generatedReferenceMediaIds(references)) if (!shown.has(mediaId)) add("Reference", mediaId);
  return tiles;
}

export interface VariationChoice {
  readonly assetId: string;
  /** The output to swap in; null while the variation has no output. */
  readonly mediaId: string | null;
  readonly label: string;
  readonly status: GeneratedAsset["status"];
}

/**
 * Every output of the generation's variation family (the original and everything derived from it),
 * oldest first. Variations named like the original are numbered so rows stay distinguishable.
 */
export function variationChoices(project: VideoProject, asset: GeneratedAsset): VariationChoice[] {
  const rootId = asset.parentAssetId ?? asset.id;
  const family = sortGeneratedAssetsByCreatedAtDesc(project.generatedAssets.filter((member) => member.id === rootId || member.parentAssetId === rootId)).reverse();
  const rootName = family.find((member) => member.id === rootId)?.name?.trim() ?? "";
  let derived = 0;
  return family.flatMap((member): VariationChoice[] => {
    const name = member.name?.trim() ?? "";
    let label: string;
    if (member.id === rootId) {
      label = name || "Original";
    } else {
      derived += 1;
      label = name && name !== rootName ? name : `Variation ${derived.toString()}`;
    }
    if (member.outputs.length === 0) return [{ assetId: member.id, mediaId: null, label, status: member.status }];
    return member.outputs.map((output, index) => ({
      assetId: member.id,
      mediaId: output.mediaId,
      label: member.outputs.length > 1 ? `${label} · ${(index + 1).toString()}` : label,
      status: member.status,
    }));
  });
}

/** Clip kinds Rust `replace_timeline_item_with_generated_output` accepts (video-track clips). */
const replaceableKinds: ReadonlySet<TimelineItemKind> = new Set<TimelineItemKind>(["video_clip", "image_clip", "lottie_clip", "generated_clip"]);

/**
 * Why `replaceTimelineItemWithGeneratedOutput` cannot swap `mediaId` into the clip, or null. The
 * action only runs in the backend (a split project folder) on unlocked video-track clips, with a
 * generated output of a completed generation.
 */
export function generatedOutputReplacementBlocker(project: VideoProject, projectDir: string, itemId: string, mediaId: string): string | null {
  const track = project.timeline.tracks.find((candidate) => candidate.items.some((item) => item.id === itemId));
  const item = track?.items.find((candidate) => candidate.id === itemId);
  if (!track || !item) return "That clip is no longer on the timeline.";
  if (project.schemaVersion < 2 || projectDir.trim().length === 0) return "Save the project to a folder to swap generated outputs.";
  if (!replaceableKinds.has(item.kind)) return item.kind === "audio_clip" ? "Audio clips can't swap generated outputs yet." : "This clip can't swap generated outputs.";
  if (track.locked) return "Unlock the track to replace this clip.";
  const media = project.media.find((candidate) => candidate.id === mediaId);
  if (!media) return "That output is no longer in the project.";
  if (media.kind !== "generated" || mediaContentKind(media) === "audio") return "Only generated outputs can replace this clip.";
  const completed = project.generatedAssets.some((asset) => asset.status === "completed" && asset.outputs.some((output) => output.mediaId === mediaId));
  return completed ? null : "That output isn't ready yet.";
}
