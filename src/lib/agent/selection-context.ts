import { generatedAssetForTimelineItem, generatedReferenceMediaIds, uniqueStringValues } from "@/lib/generation/assets";
import { generatedAssetPlacementContextLabel, mediaTimelineAction } from "@/lib/generation/timeline-placement";
import { mediaFolderPathLabel } from "@/lib/media/folder-tree";
import { filenameFromPath, mediaDisplayName } from "@/lib/media/names";
import { previewUrlForMedia } from "@/lib/media/preview-source";
import { getMotionTemplate } from "@/lib/motion-templates";
import type { GeneratedAssetSettings, GenerationPlacementIntent, VideoProject } from "@/lib/project";
import { getTimelineItemText, isTemplateTimelineItem, type TimelineItem, type TimelineTrack } from "@/lib/timeline";
import { numberProperty, timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";

export interface AgentSelectedReferenceContext {
  role: "First frame" | "Last frame" | "Reference";
  mediaId: string;
  label: string;
  kind: string;
  previewUrl?: string | null;
}

export interface AgentSelectedMediaContext {
  mediaId: string;
  label: string;
  kind: string;
  canInsertOnTimeline?: boolean;
  canQueueReferencedGeneration?: boolean;
  canQueueUpscale?: boolean;
  generatedAssetId?: string | null;
  placementIntent?: GenerationPlacementIntent | null;
  placementLabel?: string | null;
  targetFolderId?: string | null;
  targetFolderLabel?: string | null;
  modelLabel?: string | null;
  settings?: GeneratedAssetSettings | null;
  prompt?: string | null;
  referenceIds?: readonly string[];
  references?: readonly AgentSelectedReferenceContext[];
}

export interface AgentSelectedTimelineClipContext {
  itemId: string;
  label: string;
  kind: string;
  trackId?: string | null;
  trackName?: string | null;
  trackLocked?: boolean;
  trackEnabled?: boolean;
  sourceMediaId: string | null;
  timelineStartSeconds: number;
  durationSeconds: number;
  sourceIn: number | null;
  sourceOut: number | null;
  generatedAssetId?: string | null;
  placementIntent?: GenerationPlacementIntent | null;
  placementLabel?: string | null;
  targetFolderId?: string | null;
  targetFolderLabel?: string | null;
  modelLabel?: string | null;
  settings?: GeneratedAssetSettings | null;
  prompt?: string | null;
  references?: readonly AgentSelectedReferenceContext[];
  fadeInSeconds?: number | null;
  fadeOutSeconds?: number | null;
  volumeDb?: number | null;
  opacity?: number | null;
  text?: string | null;
  templateId?: string | null;
  templateFields?: Record<string, string> | null;
  templateFieldLabels?: Record<string, string> | null;
}

export interface AgentMentionTarget {
  mediaId: string;
  label: string;
  kind: string;
  description?: string | null;
  searchTerms?: readonly string[];
  canInsertOnTimeline?: boolean;
}

export function codexToolDisplayName(toolName: string) {
  switch (toolName) {
    case "video_creater.inspect_timeline":
      return "Inspect Timeline";
    case "video_creater.add_clips":
      return "Add Clips";
    case "video_creater.split_clips":
      return "Split Clips";
    case "video_creater.set_clip_properties":
      return "Set Clip Properties";
    case "video_creater.export_project":
      return "Export Project";
    case "video_creater.search_media":
      return "Search Media";
    default:
      return toolName;
  }
}

export type AgentSetupClient = "codex" | "claudeCode" | "claudeDesktop" | "cursor";

export function projectFolderLabel(projectDir: string) {
  const trimmedProjectDir = projectDir.trim();
  return trimmedProjectDir.length > 0 ? trimmedProjectDir : "project manifest";
}

export function agentSetupSnippet(client: AgentSetupClient, projectDir: string) {
  const projectFolder = projectFolderLabel(projectDir);
  const common = [
    "Video Creater agent setup",
    `Project folder: ${projectFolder}`,
    "Structured proposals only: inspect text project files and return proposal-shaped edits.",
    "Rust validates project actions before canonical state changes.",
    "Editable files: video-creater.project.json, timeline.json, media/index.json, templates, queue records.",
  ];

  if (client === "codex") {
    return [
      ...common,
      "Codex command: codex app-server --stdio",
      "Use the project folder as cwd and ask Codex for validated timeline, media, render, or generation actions.",
    ].join("\n");
  }

  if (client === "claudeCode") {
    return [
      ...common,
      "Claude Code: open this project folder as the workspace and inspect split project files before proposing edits.",
      "Start with timeline.json, media/index.json, templates, and queue records; return structured proposal JSON only.",
    ].join("\n");
  }

  if (client === "claudeDesktop") {
    return [
      ...common,
      "Claude Desktop: add this folder as project context, then request structured proposal JSON only.",
      "Start by reading timeline.json, media/index.json, templates, and recent queue records before proposing changes.",
    ].join("\n");
  }

  return [
    ...common,
    "Cursor: open the project folder, inspect media/index.json and timeline.json, then draft proposal-shaped edits.",
    "Do not directly rewrite canonical project files unless the editor asks for a manual text-file repair.",
  ].join("\n");
}

export function projectProfileInitial(projectName: string) {
  return projectName.trim().charAt(0).toUpperCase() || "V";
}

export function agentSelectedMediaContext(
  project: VideoProject,
  mediaId: string | null,
  projectDir = "",
): AgentSelectedMediaContext | null {
  if (!mediaId) {
    return null;
  }

  const media = project.media.find((asset) => asset.id === mediaId) ?? null;
  if (!media) {
    return null;
  }

  const generatedAsset =
    project.generatedAssets.find((asset) =>
      asset.outputs.some((output) => output.mediaId === mediaId),
    ) ?? null;
  const generatedOutput =
    generatedAsset?.outputs.find((output) => output.mediaId === mediaId) ?? null;
  const referenceIds = generatedAsset
    ? uniqueStringValues([
        generatedAsset.references.firstFrameMediaId,
        generatedAsset.references.lastFrameMediaId,
        ...generatedReferenceMediaIds(generatedAsset.references),
      ])
    : [];
  const references: AgentSelectedReferenceContext[] = [];
  const seenReferenceIds = new Set<string>();
  function addReference(
    role: AgentSelectedReferenceContext["role"],
    referenceMediaId: string | null | undefined,
  ) {
    if (!referenceMediaId || seenReferenceIds.has(referenceMediaId)) {
      return;
    }

    seenReferenceIds.add(referenceMediaId);
    const referenceMedia = project.media.find((asset) => asset.id === referenceMediaId) ?? null;
    references.push({
      role,
      mediaId: referenceMediaId,
      label: referenceMedia ? mediaDisplayName(referenceMedia) : referenceMediaId,
      kind: referenceMedia?.kind ?? "media",
      previewUrl: referenceMedia
        ? previewUrlForMedia(projectDir, referenceMedia.relativePath)
        : null,
    });
  }

  if (generatedAsset) {
    addReference("First frame", generatedAsset.references.firstFrameMediaId);
    addReference("Last frame", generatedAsset.references.lastFrameMediaId);
    for (const referenceMediaId of generatedReferenceMediaIds(generatedAsset.references)) {
      addReference("Reference", referenceMediaId);
    }
  }

  return {
    mediaId,
    label: generatedOutput?.relativePath
      ? filenameFromPath(generatedOutput.relativePath)
      : mediaDisplayName(media),
    kind: media.kind,
    canInsertOnTimeline:
      Boolean(generatedOutput) || Boolean(mediaTimelineAction(project, media.id)),
    canQueueReferencedGeneration: media.kind !== "audio",
    canQueueUpscale: media.kind !== "audio",
    generatedAssetId: generatedAsset?.id ?? null,
    placementIntent: generatedAsset?.placementIntent ?? null,
    placementLabel: generatedAssetPlacementContextLabel(generatedAsset?.placementIntent),
    targetFolderId: generatedAsset?.targetFolderId ?? null,
    targetFolderLabel: mediaFolderPathLabel(
      project.mediaFolders ?? [],
      generatedAsset?.targetFolderId,
    ),
    modelLabel: generatedAsset
      ? `${generatedAsset.model.provider}/${generatedAsset.model.id}`
      : null,
    settings: generatedAsset?.settings ?? null,
    prompt: generatedAsset?.prompt ?? null,
    referenceIds,
    references,
  };
}

export function agentSelectedGeneratedReferences(
  project: VideoProject,
  generatedAsset: VideoProject["generatedAssets"][number],
  projectDir = "",
) {
  const references: AgentSelectedReferenceContext[] = [];
  const seenReferenceIds = new Set<string>();
  function addReference(
    role: AgentSelectedReferenceContext["role"],
    referenceMediaId: string | null | undefined,
  ) {
    if (!referenceMediaId || seenReferenceIds.has(referenceMediaId)) {
      return;
    }

    seenReferenceIds.add(referenceMediaId);
    const referenceMedia = project.media.find((asset) => asset.id === referenceMediaId) ?? null;
    references.push({
      role,
      mediaId: referenceMediaId,
      label: referenceMedia ? mediaDisplayName(referenceMedia) : referenceMediaId,
      kind: referenceMedia?.kind ?? "media",
      previewUrl: referenceMedia
        ? previewUrlForMedia(projectDir, referenceMedia.relativePath)
        : null,
    });
  }

  addReference("First frame", generatedAsset.references.firstFrameMediaId);
  addReference("Last frame", generatedAsset.references.lastFrameMediaId);
  for (const referenceMediaId of generatedReferenceMediaIds(generatedAsset.references)) {
    addReference("Reference", referenceMediaId);
  }

  return references;
}

export function agentSelectedTimelineClipContext(
  project: VideoProject,
  item: TimelineItem | null,
  projectDir = "",
): AgentSelectedTimelineClipContext | null {
  if (!item) {
    return null;
  }

  const generatedAsset = generatedAssetForTimelineItem(project, item);
  const track = trackForTimelineItem(project, item.id);
  const templateId = isTemplateTimelineItem(item)
    ? String(item.properties.templateId)
    : null;
  const template = templateId ? getMotionTemplate(templateId) : null;
  const rawTemplateFields = item.properties.templateFields;
  const itemTemplateFields =
    rawTemplateFields &&
    typeof rawTemplateFields === "object" &&
    !Array.isArray(rawTemplateFields)
      ? Object.fromEntries(
          Object.entries(rawTemplateFields).map(([key, value]) => [
            key,
            String(value ?? ""),
          ]),
        )
      : {};
  const templateFields = templateId
    ? {
        ...(template?.defaultTextFields ?? {}),
        ...itemTemplateFields,
      }
    : null;
  const templateFieldLabels = template
    ? Object.fromEntries(
        template.fieldDefinitions.map((field) => [field.name, field.label]),
      )
    : null;

  return {
    itemId: item.id,
    label: item.label,
    kind: item.kind,
    trackId: track?.id ?? null,
    trackName: track?.name ?? null,
    trackLocked: track?.locked ?? false,
    trackEnabled: track?.enabled ?? true,
    sourceMediaId: timelineItemSourceMediaId(item),
    timelineStartSeconds: item.startSeconds,
    durationSeconds: item.durationSeconds,
    sourceIn: numberProperty(item, "sourceIn"),
    sourceOut: numberProperty(item, "sourceOut"),
    generatedAssetId: generatedAsset?.id ?? null,
    placementIntent: generatedAsset?.placementIntent ?? null,
    placementLabel: generatedAssetPlacementContextLabel(generatedAsset?.placementIntent),
    targetFolderId: generatedAsset?.targetFolderId ?? null,
    targetFolderLabel: mediaFolderPathLabel(
      project.mediaFolders ?? [],
      generatedAsset?.targetFolderId,
    ),
    modelLabel: generatedAsset
      ? `${generatedAsset.model.provider}/${generatedAsset.model.id}`
      : null,
    settings: generatedAsset?.settings ?? null,
    prompt: generatedAsset?.prompt ?? null,
    references: generatedAsset
      ? agentSelectedGeneratedReferences(project, generatedAsset, projectDir)
      : [],
    fadeInSeconds: numberProperty(item, "fadeInSeconds"),
    fadeOutSeconds: numberProperty(item, "fadeOutSeconds"),
    volumeDb: numberProperty(item, "volumeDb"),
    opacity: numberProperty(item, "opacity"),
    text:
      (item.kind === "overlay" || item.kind === "caption") &&
      item.source.type === "text"
        ? getTimelineItemText(item)
        : null,
    templateId,
    templateFields,
    templateFieldLabels,
  };
}

export function agentMentionTargets(project: VideoProject): AgentMentionTarget[] {
  const targets = new Map<string, AgentMentionTarget>();

  for (const media of project.media) {
    targets.set(media.id, {
      mediaId: media.id,
      label: mediaDisplayName(media),
      kind: media.kind,
      canInsertOnTimeline: Boolean(mediaTimelineAction(project, media.id)),
    });
  }

  for (const asset of project.generatedAssets) {
    const placementLabel = generatedAssetPlacementContextLabel(asset.placementIntent);
    const targetFolderLabel = mediaFolderPathLabel(
      project.mediaFolders ?? [],
      asset.targetFolderId,
    );
    for (const output of asset.outputs) {
      const outputFilename = filenameFromPath(output.relativePath);
      targets.set(output.mediaId, {
        mediaId: output.mediaId,
        label: asset.name?.trim() || outputFilename,
        kind: asset.kind,
        description: [
          outputFilename,
          placementLabel,
          targetFolderLabel,
        ].filter(Boolean).join(" | "),
        canInsertOnTimeline: Boolean(mediaTimelineAction(project, output.mediaId)),
        searchTerms: [
          asset.id,
          asset.name ?? "",
          asset.prompt,
          output.relativePath,
          asset.placementIntent ?? "",
          asset.targetFolderId ?? "",
          placementLabel ?? "",
          targetFolderLabel ?? "",
          asset.model.provider,
          asset.model.id,
          asset.references.firstFrameMediaId ?? "",
          asset.references.lastFrameMediaId ?? "",
          ...asset.references.mediaIds,
        ],
      });
    }
  }

  return Array.from(targets.values());
}

export function trackForTimelineItem(project: VideoProject, itemId: string): TimelineTrack | null {
  return (
    project.timeline.tracks.find((track) =>
      track.items.some((trackItem) => trackItem.id === itemId),
    ) ?? null
  );
}
