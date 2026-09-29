import type { GeneratedAsset, MediaAsset, VideoProject } from "../../project";
import type { Timeline, TimelineItem } from "../../timeline";
import { canonicalJson } from "./conversation-fixture-proposals";

/**
 * The conversation fixture's Undo rules, mirroring `src-tauri/src/project/split/agent_undo_content.rs`
 * and `agent_undo_background.rs` (rule version 4): what Undo compares, and the project it restores.
 */

type Bookkeeping = "jobs" | "renderReports" | "exportArtifacts";

export interface UndoEntryRecords {
  /** Job bookkeeping the batch added; Undo removes exactly these. */
  readonly added: Readonly<Record<Bookkeeping, readonly string[]>>;
  /** Generated assets the batch recorded; their background progress doesn't block Undo. */
  readonly addedGeneratedAssetIds: readonly string[];
  /** Generated assets recorded before the batch whose progress the batch left alone; their progress doesn't block Undo, and Undo keeps it. */
  readonly backgroundGeneratedAssetIds?: readonly string[];
}

/** Placement bookkeeping the editor and completion write on a generation's placeholder or output clip. */
const placementProperties: ReadonlySet<string> = new Set([
  "generatedAssetId",
  "generatedOutputMediaId",
  "generatedTimelinePlaceholder",
  "generatedPlaceholder",
  "pendingGeneratedAssetId",
  "reason",
  "sourceIn",
  "sourceOut",
]);

function ids(records: readonly { id: string }[] | undefined): string[] {
  return (records ?? []).map((record) => record.id);
}

export function addedRecords(before: VideoProject, after: VideoProject): UndoEntryRecords {
  const added = (key: Bookkeeping) => ids(after[key]).filter((id) => !ids(before[key]).includes(id));
  return {
    added: { jobs: added("jobs"), renderReports: added("renderReports"), exportArtifacts: added("exportArtifacts") },
    addedGeneratedAssetIds: ids(after.generatedAssets).filter((id) => !ids(before.generatedAssets).includes(id)),
    backgroundGeneratedAssetIds: after.generatedAssets.filter((asset) => before.generatedAssets.some((previous) => previous.id === asset.id && canonicalJson(progress(previous)) === canonicalJson(progress(asset)))).map((asset) => asset.id),
  };
}

/** A generation's progress fields: status, outputs, provider input URLs and createdAt. */
function progress(asset: GeneratedAsset) {
  return { status: asset.status, outputs: asset.outputs, providerInputUrls: asset.references.providerInputUrls ?? [], createdAt: asset.createdAt };
}

function timelineItems(project: VideoProject): TimelineItem[] {
  return [project.timeline, ...(project.timelines ?? []).map((entry) => entry.timeline)].flatMap((timeline) => timeline.tracks.flatMap((track) => track.items));
}

/** Library media of `project` completion added for an output of a background generation that no clip uses. */
function unusedCompletionMediaIds(project: VideoProject, backgroundIds: readonly string[]): string[] {
  const assets = project.generatedAssets.filter((asset) => backgroundIds.includes(asset.id));
  const items = timelineItems(project);
  return project.media
    .filter((media) => assets.some((asset) => completionAdded(asset, media)) && !items.some((item) => item.source.type === "media" && item.source.mediaId === media.id))
    .map((media) => media.id);
}

/** The batch asset whose generation clip `item` is: its placeholder, an output clip, or a `replace:` target. */
function clipOwner(item: TimelineItem, batch: readonly GeneratedAsset[]): string | null {
  const owner = batch.find((asset) => {
    if (item.source.type === "generated" && item.source.artifactId === asset.id) return true;
    const { source } = item;
    if (source.type === "media" && item.properties.generatedAssetId === asset.id && asset.outputs.some((output) => output.mediaId === source.mediaId)) return true;
    return asset.placementIntent?.startsWith("replace:") === true && asset.placementIntent.slice("replace:".length) === item.id;
  });
  return owner?.id ?? null;
}

function reduceGenerationClips(timeline: Timeline, batch: readonly GeneratedAsset[]): Timeline {
  const owners = new Map(timeline.tracks.flatMap((track) => track.items.flatMap((item) => (clipOwner(item, batch) === null ? [] : [[item.id, clipOwner(item, batch)] as const]))));
  if (owners.size === 0) return timeline;
  const linkGroup = (item: TimelineItem) => (typeof item.properties.linkGroupId === "string" ? item.properties.linkGroupId : null);
  const companion = (item: TimelineItem, owner: string) => {
    const linked = linkGroup(item)?.startsWith("link-") ? linkGroup(item)?.slice("link-".length) : undefined;
    return item.kind === "audio_clip" && linked !== undefined && linked !== item.id && owners.get(linked) === owner;
  };
  return {
    ...timeline,
    tracks: timeline.tracks.map((track) => ({
      ...track,
      items: track.items.flatMap((item): TimelineItem[] => {
        const owner = owners.get(item.id);
        if (owner === undefined || owner === null) return [item];
        if (companion(item, owner)) return [];
        const properties = Object.fromEntries(
          Object.entries(item.properties).filter(([key, value]) => !placementProperties.has(key) && !(key === "linkGroupId" && value === `link-${item.id}`)),
        );
        return [{ ...item, id: `generation-clip:${owner}`, kind: "generated_clip", source: { type: "generated", artifactId: owner }, label: "", properties }];
      }),
    })),
  };
}

/** Whether `media` is exactly the library record completion adds for one of `asset`'s outputs. */
function completionAdded(asset: GeneratedAsset, media: MediaAsset): boolean {
  return asset.outputs.some(
    (output) =>
      canonicalJson({ ...media, name: media.name ?? null, folderId: media.folderId ?? null }) ===
      canonicalJson({
        id: output.mediaId,
        name: null,
        relativePath: output.relativePath,
        kind: "generated",
        durationSeconds: output.durationSeconds,
        width: output.width,
        height: output.height,
        fps: output.fps > 0 ? output.fps : null,
        folderId: asset.targetFolderId ?? null,
      }),
  );
}

/**
 * What Undo compares: everything but the revision, thread, timestamp and job bookkeeping, and for the
 * generations the batch recorded, their status, outputs, provider input URLs and createdAt, their
 * generation clips beyond track, timing and user properties, and the output media completion added
 * while only those clips use it. For the background generations, their progress and the output media
 * completion added while no clip uses it.
 */
export function undoContent(project: VideoProject, batchGeneratedAssetIds: readonly string[], backgroundGeneratedAssetIds: readonly string[] = []): string {
  const { contentRevision: _revision, codexThreadId: _thread, updatedAt: _updatedAt, jobs: _jobs, renderReports: _reports, exportArtifacts: _artifacts, ...projectContent } = project;
  const unused = unusedCompletionMediaIds(project, backgroundGeneratedAssetIds);
  const content = {
    ...projectContent,
    media: projectContent.media.filter((media) => !unused.includes(media.id)),
    generatedAssets: projectContent.generatedAssets.map((asset) => (backgroundGeneratedAssetIds.includes(asset.id) ? clearedProgress(asset) : asset)),
  };
  const batch = project.generatedAssets.filter((asset) => batchGeneratedAssetIds.includes(asset.id));
  if (batch.length === 0) return canonicalJson(content);
  const timelines = [project.timeline, ...(project.timelines ?? []).map((entry) => entry.timeline)];
  const background = (media: MediaAsset) =>
    batch.some(
      (asset) =>
        completionAdded(asset, media) &&
        timelines
          .flatMap((timeline) => timeline.tracks.flatMap((track) => track.items))
          .filter((item) => item.source.type === "media" && item.source.mediaId === media.id)
          .every((item) => clipOwner(item, batch) === asset.id),
    );
  return canonicalJson({
    ...content,
    media: content.media.filter((media) => !background(media)),
    timeline: reduceGenerationClips(content.timeline, batch),
    ...(content.timelines ? { timelines: content.timelines.map((entry) => ({ ...entry, timeline: reduceGenerationClips(entry.timeline, batch) })) } : {}),
    generatedAssets: content.generatedAssets.map((asset) => (batchGeneratedAssetIds.includes(asset.id) ? clearedProgress(asset) : asset)),
  });
}

function clearedProgress(asset: GeneratedAsset): GeneratedAsset {
  return { ...asset, status: "queued", outputs: [], references: { ...asset.references, providerInputUrls: [] }, createdAt: "" };
}

/**
 * The project Undo writes: the snapshot before the edit with the current thread and job bookkeeping,
 * minus the bookkeeping the edit added. The batch's generations aren't in the snapshot, so they go.
 * Background generations keep their current progress and unused completion media.
 */
export function restoredProject(before: VideoProject, records: UndoEntryRecords, current: VideoProject): VideoProject {
  const kept = <T extends { id: string }>(list: readonly T[] | undefined, removed: readonly string[]) => (list ?? []).filter((record) => !removed.includes(record.id));
  const { exportArtifacts: _artifacts, ...snapshot } = structuredClone(before);
  const exportArtifacts = kept(current.exportArtifacts, records.added.exportArtifacts);
  const background = records.backgroundGeneratedAssetIds ?? [];
  const currentMediaIds = ids(current.media);
  const removedMedia = unusedCompletionMediaIds(before, background).filter((id) => !currentMediaIds.includes(id));
  const snapshotMedia = snapshot.media.filter((media) => !removedMedia.includes(media.id));
  const carriedMedia = unusedCompletionMediaIds(current, background).filter((id) => !ids(snapshotMedia).includes(id));
  return {
    ...snapshot,
    media: [...snapshotMedia, ...current.media.filter((media) => carriedMedia.includes(media.id))],
    generatedAssets: snapshot.generatedAssets.map((asset) => {
      const now = background.includes(asset.id) ? current.generatedAssets.find((candidate) => candidate.id === asset.id) : undefined;
      if (!now) return asset;
      const { providerInputUrls: _urls, ...references } = asset.references;
      const carriedUrls = now.references.providerInputUrls === undefined ? {} : { providerInputUrls: now.references.providerInputUrls };
      return { ...asset, status: now.status, outputs: now.outputs, references: { ...references, ...carriedUrls }, createdAt: now.createdAt };
    }),
    codexThreadId: current.codexThreadId,
    jobs: kept(current.jobs, records.added.jobs),
    renderReports: kept(current.renderReports, records.added.renderReports),
    ...(current.exportArtifacts === undefined && before.exportArtifacts === undefined ? {} : { exportArtifacts }),
  };
}
