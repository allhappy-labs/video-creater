import { pluralize, formatCredits } from "@/lib/format";
import { selectedGenerationCost } from "@/lib/generation/pricing";
import {
  generationModeFromAsset,
  generationModelOptions,
  generationModelValue,
  generationProviderDisplayName,
} from "@/lib/generation/provider-rules";
import {
  boundedGenerationDurationValue,
  generationDurationOptionsForModel,
  generationDurationValue,
  generationResolutionOptionsForModel,
  generationResolutionValue,
} from "@/lib/generation/settings-options";
import { mediaDisplayName } from "@/lib/media/names";
import type { CodexProposalImpact, ProjectAction, VideoProject } from "@/lib/project";
import {
  isTemplateTimelineItem,
  type TimelineItem,
  type TimelineTrack,
  type TrackKind,
  type TransitionKind,
} from "@/lib/timeline";
import { trackDisplayNames } from "@/lib/timeline-ops/track-bands";

/**
 * Plain-language facts and placement lines for AI result and review cards. Everything here is
 * derived from validated actions and project snapshots, and never renders internal ids.
 */

type ResultFactKind =
  | "duration"
  | "cuts"
  | "captions"
  | "titles"
  | "reverse"
  | "speed"
  | "detachedAudio"
  | "generation"
  | "provider"
  | "cost"
  | "replaces";

export interface ResultFact {
  readonly kind: ResultFactKind;
  readonly label: string;
}

type GeneratedAssetDraft = Extract<ProjectAction, { type: "recordGeneratedAsset" }>["asset"];

/** "0:45", "2:10", or "1:02:05" once past an hour. */
export function clockLabel(seconds: number): string {
  const total = Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds) : 0;
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secondsPart = (total % 60).toString().padStart(2, "0");
  return hours > 0
    ? `${hours.toString()}:${minutes.toString().padStart(2, "0")}:${secondsPart}`
    : `${minutes.toString()}:${secondsPart}`;
}

function secondsLabel(seconds: number): string {
  return seconds >= 60 ? clockLabel(seconds) : `${Math.round(seconds).toString()}s`;
}

function quoted(text: string): string {
  const clean = text.replace(/\s+/g, " ").trim();
  if (clean.length <= 44) return `“${clean}”`;
  const cut = clean.slice(0, 44);
  const lastSpace = cut.lastIndexOf(" ");
  return `“${(lastSpace > 20 ? cut.slice(0, lastSpace) : cut).trimEnd()}…”`;
}

function findItem(project: VideoProject, itemId: string): { item: TimelineItem; track: TimelineTrack } | null {
  for (const track of project.timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return { item, track };
  }
  return null;
}

function isTitle(item: TimelineItem): boolean {
  return item.kind === "overlay" && isTemplateTimelineItem(item);
}

function itemText(item: TimelineItem): string {
  return item.source.type === "text" ? item.source.text.trim() : "";
}

function itemName(item: TimelineItem, project: VideoProject): string | null {
  const { source } = item;
  const media = source.type === "media" ? project.media.find((asset) => asset.id === source.mediaId) : undefined;
  return item.label.trim() || (media ? mediaDisplayName(media) : null) || itemText(item) || null;
}

/** `caption “Hello”`, `title “Opening Hook”`, or `“Opening clip”`; null when nothing human is known. */
function itemPhrase(item: TimelineItem, project: VideoProject): string | null {
  if (item.kind === "caption" || item.kind === "overlay") {
    const text = itemText(item) || item.label.trim();
    const noun = item.kind === "caption" ? "caption" : isTitle(item) ? "title" : "overlay";
    return text ? `${noun} ${quoted(text)}` : noun;
  }
  const name = itemName(item, project);
  return name ? quoted(name) : null;
}

function itemNoun(items: readonly TimelineItem[]): string {
  const nouns = new Set(
    items.map((item) =>
      item.kind === "caption"
        ? "caption"
        : isTitle(item)
          ? "title"
          : item.kind === "overlay"
            ? "overlay"
            : item.kind === "audio_clip"
              ? "audio clip"
              : "clip",
    ),
  );
  const [only] = nouns;
  return nouns.size === 1 && only ? only : "item";
}

function durationFact(impact: CodexProposalImpact, before: VideoProject, after: VideoProject | null): ResultFact {
  const beforeSeconds = Number.isFinite(impact.beforeDurationSeconds)
    ? impact.beforeDurationSeconds
    : before.timeline.durationSeconds;
  const afterSeconds = Number.isFinite(impact.afterDurationSeconds)
    ? impact.afterDurationSeconds
    : (after ?? before).timeline.durationSeconds;
  const beforeLabel = clockLabel(beforeSeconds);
  const afterLabel = clockLabel(afterSeconds);
  return {
    kind: "duration",
    label: beforeLabel === afterLabel ? `${beforeLabel} (unchanged)` : `${beforeLabel} → ${afterLabel}`,
  };
}

function addedItems(actions: readonly ProjectAction[]): TimelineItem[] {
  return actions.flatMap((action) =>
    action.type === "addItems" || action.type === "insertItems" ? [...action.items] : [],
  );
}

/** Chips for playback edits: "1 reversed clip", "1 clip played forward", "2 speed changes", "1 detached audio clip". */
function playbackFacts(actions: readonly ProjectAction[]): ResultFact[] {
  const count = (matches: (action: ProjectAction) => boolean) => actions.filter(matches).length;
  const reversed = count((action) => action.type === "updateClipReverse" && action.reverse);
  const forward = count((action) => action.type === "updateClipReverse" && !action.reverse);
  const speed = count((action) => action.type === "updateVisualClipSpeed" || action.type === "updateAudioClipSpeed");
  const detached = count((action) => action.type === "detachAudio");
  return [
    ...(reversed > 0 ? [{ kind: "reverse" as const, label: pluralize(reversed, "reversed clip") }] : []),
    ...(forward > 0 ? [{ kind: "reverse" as const, label: `${pluralize(forward, "clip")} played forward` }] : []),
    ...(speed > 0 ? [{ kind: "speed" as const, label: pluralize(speed, "speed change") }] : []),
    ...(detached > 0 ? [{ kind: "detachedAudio" as const, label: pluralize(detached, "detached audio clip") }] : []),
  ];
}

function generationCost(asset: GeneratedAssetDraft): number | "varies" {
  const mode = generationModeFromAsset(asset);
  const value = generationModelValue(asset.model);
  const model = generationModelOptions[mode].find((option) => generationModelValue(option) === value) ?? asset.model;
  const { settings } = asset;
  const duration =
    generationDurationValue(settings.durationSeconds, generationDurationOptionsForModel(mode, model)) ??
    boundedGenerationDurationValue(settings.durationSeconds) ??
    "";
  const resolution =
    settings.resolution ??
    generationResolutionValue(settings.width, settings.height, generationResolutionOptionsForModel(mode, model)) ??
    "";
  return selectedGenerationCost(
    mode,
    model,
    duration,
    resolution,
    String(settings.numImages ?? 1),
    settings.generateAudio ?? true,
    asset.prompt,
    settings.quality ?? "",
  );
}

function generationFacts(assets: readonly GeneratedAssetDraft[]): ResultFact[] {
  if (assets.length === 0) return [];
  const groups = new Map<string, number>();
  const providers: string[] = [];
  let credits = 0;
  let varies = false;
  for (const asset of assets) {
    const seconds = asset.settings.durationSeconds;
    const duration = seconds !== null && Number.isFinite(seconds) && seconds > 0 ? ` · ${secondsLabel(seconds)}` : "";
    const key = `${generationModeFromAsset(asset)}${duration}`;
    groups.set(key, (groups.get(key) ?? 0) + 1);
    const provider = generationProviderDisplayName(asset.model.provider);
    if (provider && !providers.includes(provider)) providers.push(provider);
    const cost = generationCost(asset);
    if (cost === "varies") varies = true;
    else credits += cost;
  }
  return [
    ...Array.from(groups, ([key, count]): ResultFact => ({ kind: "generation", label: `${count.toString()} × ${key}` })),
    ...(providers.length > 0 ? [{ kind: "provider" as const, label: providers.join(", ") }] : []),
    {
      kind: "cost",
      label: varies ? "Cost varies" : `≈ ${formatCredits(credits)} ${credits === 1 ? "credit" : "credits"}`,
    },
  ];
}

function replacedItemIds(actions: readonly ProjectAction[]): string[] {
  const ids: string[] = [];
  const add = (id: string | null | undefined) => {
    if (id && !ids.includes(id)) ids.push(id);
  };
  for (const action of actions) {
    if (action.type === "removeItems") action.itemIds.forEach(add);
    else if (action.type === "replaceTimelineItemWithGeneratedOutput") add(action.replacement.itemId);
    else if (action.type === "completeGeneratedAsset") add(action.replacement?.itemId);
    else if (action.type === "recordGeneratedAsset" && action.asset.placementIntent?.startsWith("replace:")) {
      add(action.asset.placementIntent.slice("replace:".length));
    }
  }
  return ids;
}

function replacesFact(actions: readonly ProjectAction[], project: VideoProject): ResultFact {
  const ids = replacedItemIds(actions);
  const found = ids.flatMap((id) => findItem(project, id)?.item ?? []);
  if (ids.length === 0) return { kind: "replaces", label: "Replaces nothing" };
  const [single] = found;
  const singleName = ids.length === 1 && single ? itemName(single, project) : null;
  if (singleName) return { kind: "replaces", label: `Replaces ${quoted(singleName)}` };
  const noun = found.length === ids.length ? itemNoun(found) : "item";
  return { kind: "replaces", label: `Replaces ${pluralize(ids.length, noun)}` };
}

/**
 * Fact chips for a prepared or applied bundle, e.g. "2:10 → 0:45", "14 cuts", "38 captions",
 * "1 title", "3 × video · 6s", "Replicate", "≈ 24 credits", "Replaces nothing".
 * `afterProject` is null while the bundle still waits for review.
 */
export function resultFacts(
  impact: CodexProposalImpact,
  actions: readonly ProjectAction[],
  beforeProject: VideoProject,
  afterProject: VideoProject | null,
): ResultFact[] {
  const cuts = actions.reduce(
    (total, action) =>
      total +
      (action.type === "splitItems" ? action.splits.length : action.type === "rippleDeleteRanges" ? action.ranges.length : 0),
    0,
  );
  const added = addedItems(actions);
  const captions = added.filter((item) => item.kind === "caption").length;
  const titles = added.filter(isTitle).length;
  const assets = actions.flatMap((action) => (action.type === "recordGeneratedAsset" ? [action.asset] : []));

  return [
    durationFact(impact, beforeProject, afterProject),
    ...(cuts > 0 ? [{ kind: "cuts" as const, label: pluralize(cuts, "cut") }] : []),
    ...(captions > 0 ? [{ kind: "captions" as const, label: pluralize(captions, "caption") }] : []),
    ...(titles > 0 ? [{ kind: "titles" as const, label: pluralize(titles, "title") }] : []),
    ...playbackFacts(actions),
    ...generationFacts(assets),
    replacesFact(actions, beforeProject),
  ];
}

const trackKindLabels: Record<TrackKind, string> = {
  video: "a video",
  audio: "an audio",
  caption: "a caption",
  overlay: "a text",
  hyperframe_scene: "a graphics",
};

interface PlacementContext {
  readonly project: VideoProject;
  readonly actions: readonly ProjectAction[];
  /** Timeline header names ("Video 1", "Captions") for existing tracks and the tracks the bundle creates. */
  readonly trackNames: ReadonlyMap<string, string>;
}

/**
 * Header names from `trackDisplayNames`. Created tracks go after the existing ones, so they number
 * on from their kind ("Video 2") without renaming the tracks the timeline shows now.
 */
function placementTrackNames(actions: readonly ProjectAction[], project: VideoProject): Map<string, string> {
  const created = actions.flatMap((action) => (action.type === "createTrack" ? [action.track] : []));
  const { timeline } = project;
  return trackDisplayNames({ ...timeline, tracks: [...timeline.tracks, ...created] });
}

function trackName(context: PlacementContext, trackId: string): string | null {
  return context.trackNames.get(trackId) ?? null;
}

function onTrack(context: PlacementContext, trackId: string): string {
  const name = trackName(context, trackId);
  return name ? ` on ${name}` : " on a new track";
}

function generatedMediaName(context: PlacementContext, mediaId: string): string | null {
  const assets = [
    ...context.project.generatedAssets,
    ...context.actions.flatMap((action) => (action.type === "recordGeneratedAsset" ? [action.asset] : [])),
  ];
  const asset = assets.find((candidate) => candidate.outputs.some((output) => output.mediaId === mediaId));
  const media = context.project.media.find((candidate) => candidate.id === mediaId);
  return asset?.name?.trim() || (media ? mediaDisplayName(media) : null);
}

function itemAt(context: PlacementContext, itemId: string): string | null {
  const found = findItem(context.project, itemId);
  const phrase = found ? itemPhrase(found.item, context.project) : null;
  return found && phrase ? `${phrase} at ${clockLabel(found.item.startSeconds)}` : null;
}

function addLines(
  context: PlacementContext,
  verb: "Add" | "Insert",
  trackId: string,
  items: readonly TimelineItem[],
  insertSeconds: number | null,
): string[] {
  const where = onTrack(context, trackId);
  if (items.length === 0) return [];
  if (items.length > 3) {
    const start = insertSeconds ?? Math.min(...items.map((item) => item.startSeconds));
    const end = Math.max(...items.map((item) => item.startSeconds + item.durationSeconds));
    const range = insertSeconds === null ? ` from ${clockLabel(start)} to ${clockLabel(end)}` : ` at ${clockLabel(start)}`;
    return [`${verb} ${pluralize(items.length, itemNoun(items))}${where}${range}`];
  }
  return items.map((item) => {
    const phrase = itemPhrase(item, context.project) ?? `a ${itemNoun([item])}`;
    return `${verb} ${phrase}${where} at ${clockLabel(insertSeconds ?? item.startSeconds)}`;
  });
}

function removeLines(context: PlacementContext, itemIds: readonly string[]): string[] {
  const found = itemIds.flatMap((id) => findItem(context.project, id) ?? []);
  const unknown = itemIds.length - found.length;
  const lines =
    found.length > 3
      ? [`Remove ${pluralize(found.length, itemNoun(found.map(({ item }) => item)))}`]
      : found.map(({ item, track }) => {
          const phrase = itemPhrase(item, context.project) ?? `a ${itemNoun([item])}`;
          return `Remove ${phrase} from ${trackName(context, track.id) ?? track.name} at ${clockLabel(item.startSeconds)}`;
        });
  return unknown > 0 ? [...lines, `Remove ${pluralize(unknown, "item")}`] : lines;
}

function rippleLines(
  context: PlacementContext,
  ranges: Extract<ProjectAction, { type: "rippleDeleteRanges" }>["ranges"],
): string[] {
  if (ranges.length > 3) {
    const total = ranges.reduce((sum, range) => sum + Math.max(0, range.endSeconds - range.startSeconds), 0);
    return [`Cut ${ranges.length.toString()} ranges (${secondsLabel(total)} total) and close the gaps`];
  }
  return ranges.map((range) => {
    const names = range.trackIds.flatMap((id) => trackName(context, id) ?? []);
    const where = names.length > 0 && names.length === range.trackIds.length ? ` on ${names.join(", ")}` : "";
    return `Cut ${clockLabel(range.startSeconds)}–${clockLabel(range.endSeconds)}${where} and close the gap`;
  });
}

function generationLine(context: PlacementContext, asset: GeneratedAssetDraft): string {
  const name = asset.name?.trim() || asset.prompt.trim();
  const subject = `Generate ${generationModeFromAsset(asset)}${name ? ` ${quoted(name)}` : ""}`;
  const intent = asset.placementIntent ?? "library";
  if (intent === "timeline") {
    const start = asset.settings.timelineStartSeconds;
    return `${subject} and place it on the timeline${typeof start === "number" ? ` at ${clockLabel(start)}` : ""}`;
  }
  if (intent.startsWith("replace:")) {
    const target = itemAt(context, intent.slice("replace:".length));
    return `${subject} to replace ${target ?? "a clip"}`;
  }
  const folder = context.project.mediaFolders?.find((candidate) => candidate.id === asset.targetFolderId);
  return `${subject} into the media library${folder ? ` (${folder.name})` : ""}`;
}

function replaceLine(context: PlacementContext, itemId: string, mediaId: string): string {
  const media = generatedMediaName(context, mediaId);
  return `Replace ${itemAt(context, itemId) ?? "a clip"} with ${media ? quoted(media) : "the generated clip"}`;
}

const transitionKindLabels: Record<TransitionKind, string> = {
  crossfade: "crossfade",
  dipToBlack: "dip to black",
  dipToWhite: "dip to white",
  wipe: "wipe",
};

function clipPairLabel(context: PlacementContext, leftItemId: string, rightItemId: string): string {
  const phrase = (itemId: string) => {
    const found = findItem(context.project, itemId);
    return found ? itemPhrase(found.item, context.project) : null;
  };
  const left = phrase(leftItemId);
  const right = phrase(rightItemId);
  return left && right ? `between ${left} and ${right}` : "between two clips";
}

function transitionLine(
  context: PlacementContext,
  action: Extract<ProjectAction, { type: "addTransition" | "updateTransition" | "removeTransition" }>,
): string {
  if (action.type === "addTransition") {
    const { transition } = action;
    return `Add a ${transitionKindLabels[transition.kind]} ${clipPairLabel(context, transition.leftItemId, transition.rightItemId)}`;
  }
  const existing = context.project.timeline.tracks
    .find((track) => track.id === action.trackId)
    ?.transitions?.find((transition) => transition.id === action.transitionId);
  const between = existing ? ` ${clipPairLabel(context, existing.leftItemId, existing.rightItemId)}` : "";
  if (action.type === "removeTransition") {
    return `Remove the ${existing ? transitionKindLabels[existing.kind] : "transition"}${between}`;
  }
  const duration =
    action.durationSeconds !== undefined ? `${Number(action.durationSeconds.toFixed(2)).toString()}s` : null;
  if (action.kind === undefined) return `Set the transition${between} to ${duration ?? "its current length"}`;
  return `Change the transition${between} to a ${duration ? `${duration} ` : ""}${transitionKindLabels[action.kind]}`;
}

function clipPhrase(context: PlacementContext, itemId: string): string {
  const found = findItem(context.project, itemId);
  return (found && itemPhrase(found.item, context.project)) ?? "a clip";
}

/** "Detach audio from “Clip”", then the audio clip it adds, named the way `detachAudio` labels it. */
function detachAudioLines(context: PlacementContext, action: Extract<ProjectAction, { type: "detachAudio" }>): string[] {
  const found = findItem(context.project, action.itemId);
  if (!found) return ["Detach audio from a clip"];
  const { item } = found;
  const label = item.label.trim() ? `${item.label} audio` : "";
  const audio: TimelineItem = { ...item, id: action.audioItemId, kind: "audio_clip", label };
  return [`Detach audio from ${clipPhrase(context, action.itemId)}`, ...addLines(context, "Add", action.targetTrackId, [audio], null)];
}

function placementLines(context: PlacementContext, action: ProjectAction): string[] {
  const { project } = context;
  switch (action.type) {
    case "createTrack":
      return [`Add ${trackKindLabels[action.track.kind]} track ${quoted(trackName(context, action.track.id) ?? action.track.name)}`];
    case "addItems":
      return addLines(context, "Add", action.targetTrackId, action.items, null);
    case "insertItems":
      return addLines(context, "Insert", action.targetTrackId, action.items, action.insertSeconds);
    case "moveItems":
      return action.moves.flatMap((move) => {
        const found = findItem(project, move.itemId);
        const phrase = found ? itemPhrase(found.item, project) : null;
        return `Move ${phrase ?? "a clip"}${onTrack(context, move.targetTrackId).replace(" on ", " to ")} at ${clockLabel(move.startSeconds)}`;
      });
    case "splitItems":
      return action.splits.map((split) => {
        const found = findItem(project, split.itemId);
        return `Split ${(found && itemPhrase(found.item, project)) ?? "a clip"} at ${clockLabel(split.splitSeconds)}`;
      });
    case "rippleDeleteRanges":
      return rippleLines(context, action.ranges);
    case "removeItems":
      return removeLines(context, action.itemIds);
    case "recordGeneratedAsset":
      return [generationLine(context, action.asset)];
    case "replaceTimelineItemWithGeneratedOutput":
      return [replaceLine(context, action.replacement.itemId, action.replacement.mediaId)];
    case "completeGeneratedAsset":
      return action.replacement ? [replaceLine(context, action.replacement.itemId, action.replacement.mediaId)] : [];
    case "deleteMedia": {
      const names = action.mediaIds.flatMap((id) => {
        const media = project.media.find((candidate) => candidate.id === id);
        return media ? [mediaDisplayName(media)] : [];
      });
      const [only] = names;
      return action.mediaIds.length === 1 && only
        ? [`Delete ${quoted(only)} and its clips from the project`]
        : [`Delete ${pluralize(action.mediaIds.length, "media file")} and their clips from the project`];
    }
    case "removeTracks":
      return action.trackIds.map((id) => {
        const name = trackName(context, id);
        return name ? `Remove the ${name} track` : "Remove a track";
      });
    case "createTimeline":
      return [`Create timeline ${quoted(action.name)}`];
    case "deleteTimeline": {
      const timeline = project.timelines?.find((candidate) => candidate.id === action.timelineId);
      return [timeline ? `Delete timeline ${quoted(timeline.name)}` : "Delete a timeline"];
    }
    case "addTransition":
    case "updateTransition":
    case "removeTransition":
      return [transitionLine(context, action)];
    case "updateClipReverse":
      return [action.reverse ? `Reverse ${clipPhrase(context, action.itemId)}` : `Play ${clipPhrase(context, action.itemId)} forward`];
    case "updateVisualClipSpeed":
    case "updateAudioClipSpeed":
      return [`Set ${clipPhrase(context, action.itemId)} to ${Number(action.speed.toFixed(2)).toString()}× speed`];
    case "detachAudio":
      return detachAudioLines(context, action);
    default:
      return [];
  }
}

/** Plain-language placement lines for a review card, with human names and timecodes. */
export function reviewPlacements(actions: readonly ProjectAction[], project: VideoProject): string[] {
  const context: PlacementContext = { project, actions, trackNames: placementTrackNames(actions, project) };
  return actions.flatMap((action) => placementLines(context, action));
}
