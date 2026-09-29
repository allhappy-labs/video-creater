import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem, TimelineTrack } from "@/lib/timeline";
import { timelineItemHasAudio } from "@/lib/timeline-ops/automation";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { newTrackId, planDropTarget } from "@/lib/timeline-ops/dynamic-tracks";
import { stringProperty } from "@/lib/timeline-ops/item-properties";

type DetachAudioAction = Extract<ProjectAction, { type: "detachAudio" }>;

/** Fresh ids the caller supplies: the audio clip, its link group, and a track created if needed. */
export interface DetachAudioIds {
  readonly audioItemId: string;
  readonly linkGroupId: string;
  readonly trackId: string;
}

/**
 * Ids for detaching `itemId`'s audio: `<itemId>-audio` (suffixed `-2`, `-3`, ... when taken, like
 * the agent tool), the caller's fresh `linkGroupId`, and an unused audio track id for a new track.
 */
export function detachAudioIds(project: VideoProject, itemId: string, linkGroupId: string): DetachAudioIds {
  const taken = new Set(project.timeline.tracks.flatMap((track) => track.items.map((item) => item.id)));
  const base = `${itemId}-audio`;
  let audioItemId = base;
  for (let suffix = 2; taken.has(audioItemId); suffix += 1) audioItemId = `${base}-${suffix.toString()}`;
  return { audioItemId, linkGroupId, trackId: newTrackId(project.timeline, "audio") };
}

const overlapEpsilonSeconds = 1e-6;
/** Clip-level audio properties that move from the video clip to its detached audio clip. */
const movedAudioProperties = ["volumeDb"] as const;
const copiedRangeProperties = ["sourceIn", "sourceOut", "speed", "reverse"] as const;

function locate(project: VideoProject, itemId: string): { item: TimelineItem; track: TimelineTrack } | null {
  for (const track of project.timeline.tracks) {
    const item = track.items.find((candidate) => candidate.id === itemId);
    if (item) return { item, track };
  }
  return null;
}

function linkedAudioClip(project: VideoProject, item: TimelineItem): TimelineItem | null {
  const groupId = stringProperty(item, "linkGroupId");
  if (groupId === null) return null;
  return (
    project.timeline.tracks
      .flatMap((track) => track.items)
      .find((candidate) => candidate.kind === "audio_clip" && stringProperty(candidate, "linkGroupId") === groupId) ?? null
  );
}

function detachableMedia(project: VideoProject, item: TimelineItem) {
  if (item.kind !== "video_clip" || item.source.type !== "media") return false;
  const { mediaId } = item.source;
  const kind = project.media.find((media) => media.id === mediaId)?.kind;
  return kind === "video" || kind === "generated";
}

function spanIsFree(track: TimelineTrack, startSeconds: number, endSeconds: number) {
  return track.items.every(
    (candidate) =>
      candidate.startSeconds + candidate.durationSeconds <= startSeconds + overlapEpsilonSeconds ||
      candidate.startSeconds >= endSeconds - overlapEpsilonSeconds,
  );
}

/**
 * Plans Detach audio for a video clip: one `detachAudio` onto the first unlocked audio track with
 * room for the clip's span, else a new audio track (`createTrack`, plus `reorderTrack` when the
 * track must sit before an existing one) followed by `detachAudio`. The clip keeps an existing
 * link group; otherwise `ids.linkGroupId` links the pair.
 */
export function planDetachAudio(project: VideoProject, itemId: string, ids: DetachAudioIds): CommandResult {
  const location = locate(project, itemId);
  if (!location || location.item.kind !== "video_clip") return { blocked: "Detach audio is available for video clips." };
  const { item, track } = location;
  if (!timelineItemHasAudio(project, item) || !detachableMedia(project, item)) return { blocked: "This clip has no sound to detach." };
  if (linkedAudioClip(project, item)) return { blocked: "This clip's sound is already on a linked audio clip." };
  if (track.locked) return { blocked: "Unlock the track to detach this clip's audio." };

  const endSeconds = item.startSeconds + item.durationSeconds;
  const audioTracks = project.timeline.tracks.filter((candidate) => candidate.kind === "audio" && !candidate.locked);
  const detach = (targetTrackId: string): DetachAudioAction => ({
    type: "detachAudio",
    itemId,
    audioItemId: ids.audioItemId,
    targetTrackId,
    linkGroupId: stringProperty(item, "linkGroupId") ?? ids.linkGroupId,
  });
  const free = audioTracks.find((candidate) => spanIsFree(candidate, item.startSeconds, endSeconds));
  if (free) return { actions: [detach(free.id)] };

  const plan = planDropTarget({
    timeline: project.timeline,
    assetKind: "audio",
    hoveredTrackId: audioTracks[0]?.id ?? null,
    insertIndex: null,
    startSeconds: item.startSeconds,
    durationSeconds: item.durationSeconds,
    newTrackId: ids.trackId,
  });
  if (plan.kind === "invalid") return { blocked: plan.reason };
  if (plan.kind === "existing") return { actions: [detach(plan.trackId)] };
  return { actions: [plan.createTrack, ...(plan.reorderTrack ? [plan.reorderTrack] : []), detach(plan.trackId)] };
}

function withoutVolumeKeyframes(properties: TimelineItem["properties"]) {
  const keyframes = properties.keyframes;
  if (keyframes === null || typeof keyframes !== "object" || Array.isArray(keyframes) || !("volumeDb" in keyframes)) {
    return { rest: properties, volumeLane: undefined };
  }
  const { volumeDb: volumeLane, ...otherLanes } = keyframes as Record<string, unknown>;
  const rest = { ...properties };
  if (Object.keys(otherLanes).length > 0) rest.keyframes = otherLanes;
  else delete rest.keyframes;
  return { rest, volumeLane };
}

function isAudioEffect(effect: unknown) {
  if (effect === null || typeof effect !== "object") return false;
  const effectType = (effect as Record<string, unknown>).effectType;
  return typeof effectType === "string" && effectType.startsWith("audio.");
}

/**
 * The local mirror of Rust `detach_audio`: adds a linked `audio_clip` over the video clip's span
 * that plays the same media range, moves the clip-level audio properties (`volumeDb`, its keyframe
 * lane and `audio.*` effects) onto it, and marks the video `audioDetached`. Visual fades stay on the
 * video, where they fade opacity. Returns the new tracks; errors use the Rust validation messages.
 */
export function applyDetachAudio(project: VideoProject, action: DetachAudioAction): TimelineTrack[] {
  const location = locate(project, action.itemId);
  if (!location) throw new Error(`timeline item was not found: ${action.itemId}`);
  const { item, track } = location;
  if (!detachableMedia(project, item)) throw new Error(`timeline item has no detachable audio: ${action.itemId}`);
  if (linkedAudioClip(project, item)) throw new Error(`timeline item audio is already on a linked audio clip: ${action.itemId}`);
  const target = project.timeline.tracks.find((candidate) => candidate.id === action.targetTrackId);
  if (!target) throw new Error(`timeline track was not found: ${action.targetTrackId}`);
  if (target.kind !== "audio") throw new Error(`item kind AudioClip is incompatible with track kind ${trackKindName(target.kind)}`);
  if (track.locked) throw new Error(`track is locked: ${track.id}`);
  if (target.locked) throw new Error(`track is locked: ${target.id}`);
  const existingGroup = stringProperty(item, "linkGroupId");
  const groupLength = [...action.linkGroupId].length;
  if (!action.linkGroupId.trim() || groupLength > 128 || (existingGroup !== null && existingGroup !== action.linkGroupId)) {
    throw new Error(
      `effect parameter is invalid: link group id must be 1 to 128 characters and match the clip's existing link group: ${action.linkGroupId}`,
    );
  }
  if (project.timeline.tracks.some((candidate) => candidate.items.some((entry) => entry.id === action.audioItemId))) {
    throw new Error(`timeline item id already exists: ${action.audioItemId}`);
  }
  const endSeconds = item.startSeconds + item.durationSeconds;
  const blocking = target.items.find((candidate) => !spanIsFree({ ...target, items: [candidate] }, item.startSeconds, endSeconds));
  if (blocking) {
    throw new Error(`timeline items overlap on track ${target.id}: ${action.audioItemId} overlaps ${blocking.id}`);
  }

  const { rest, volumeLane } = withoutVolumeKeyframes(item.properties);
  const effects = Array.isArray(item.properties.effects) ? item.properties.effects : [];
  const audioProperties: TimelineItem["properties"] = { linkGroupId: action.linkGroupId, sourceClipType: "audio" };
  for (const key of [...copiedRangeProperties, ...movedAudioProperties]) {
    if (item.properties[key] !== undefined) audioProperties[key] = item.properties[key];
  }
  if (volumeLane !== undefined) audioProperties.keyframes = { volumeDb: volumeLane };
  const audioEffects = effects.filter(isAudioEffect);
  if (audioEffects.length > 0) audioProperties.effects = audioEffects;

  const videoProperties: TimelineItem["properties"] = { ...rest, linkGroupId: action.linkGroupId, audioDetached: true };
  for (const key of movedAudioProperties) delete videoProperties[key];
  if (Array.isArray(item.properties.effects)) {
    const visualEffects = effects.filter((effect) => !isAudioEffect(effect));
    if (visualEffects.length > 0) videoProperties.effects = visualEffects;
    else delete videoProperties.effects;
  }
  const audioItem: TimelineItem = {
    id: action.audioItemId,
    kind: "audio_clip",
    startSeconds: item.startSeconds,
    durationSeconds: item.durationSeconds,
    source: item.source,
    label: `${item.label} audio`,
    properties: audioProperties,
  };
  return project.timeline.tracks.map((candidate) => {
    if (candidate.id !== track.id && candidate.id !== target.id) return candidate;
    const items = candidate.items.map((entry) => (entry.id === item.id ? { ...entry, properties: videoProperties } : entry));
    return { ...candidate, items: candidate.id === target.id ? [...items, audioItem] : items };
  });
}

function trackKindName(kind: TimelineTrack["kind"]) {
  return kind
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}
