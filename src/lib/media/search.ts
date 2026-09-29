import { generatedAssetPlacementContextLabel } from "@/lib/generation/timeline-placement";
import { filenameFromPath } from "@/lib/media/names";
import type {
  GeneratedAsset,
  MediaAsset,
  ProjectMediaSearchResult,
  ProjectMediaSearchScope,
} from "@/lib/project";

export function normalizeSearchText(value: string) {
  return value.trim().toLocaleLowerCase();
}

export function isSearchableText(value: string | null | undefined): value is string {
  return Boolean(value);
}

export function mediaMatchesSearch(asset: MediaAsset, folderLabel: string | null, query: string) {
  if (!query) {
    return true;
  }

  return [
    asset.id,
    asset.name,
    asset.relativePath,
    filenameFromPath(asset.relativePath),
    asset.kind,
    folderLabel,
  ]
    .filter(isSearchableText)
    .some((value) => value.toLocaleLowerCase().includes(query));
}

export function generatedAssetMatchesSearch(
  asset: GeneratedAsset,
  query: string,
  targetFolderLabel: string | null,
) {
  if (!query) {
    return true;
  }

  return [
    asset.id,
    asset.name,
    asset.prompt,
    asset.status,
    asset.model.provider,
    asset.model.id,
    asset.placementIntent,
    generatedAssetPlacementContextLabel(asset.placementIntent),
    targetFolderLabel,
    generatedReferenceLabel(asset),
    generatedLineageLabel(asset),
    ...asset.outputs.flatMap((output) => [output.mediaId, output.relativePath]),
  ]
    .filter(isSearchableText)
    .some((value) => value.toLocaleLowerCase().includes(query));
}

export function stringField(value: Record<string, unknown>, key: string) {
  const field = value[key];
  return typeof field === "string" && field.trim().length > 0 ? field : null;
}

export function mediaIdsFromIndexedSearch(result: ProjectMediaSearchResult | null) {
  if (!result) {
    return new Set<string>();
  }

  const ids = new Set<string>();
  for (const group of [
    ...result.groups.spoken,
    ...result.groups.visual,
    ...result.groups.metadata,
  ]) {
    const mediaId = stringField(group, "mediaId");
    if (mediaId) {
      ids.add(mediaId);
    }
  }
  for (const generated of result.groups.generated) {
    const mediaIds = generated.mediaIds;
    if (!Array.isArray(mediaIds)) {
      continue;
    }
    for (const mediaId of mediaIds) {
      if (typeof mediaId === "string" && mediaId.trim().length > 0) {
        ids.add(mediaId);
      }
    }
  }

  return ids;
}

export function indexedSearchStatusLabels(result: ProjectMediaSearchResult | null) {
  if (!result) {
    return [];
  }

  const spoken =
    result.spokenStatus === "ready"
      ? "Spoken ready"
      : result.spokenStatus === "noTranscripts"
        ? "No transcripts"
        : "Spoken indexing";
  const visual =
    result.visualStatus === "ready"
      ? "Visual ready"
      : result.visualStatus === "notInstalled"
        ? "Visual search not installed"
        : result.visualStatus === "indexing"
          ? "Visual indexing"
          : result.visualStatus === "failed"
            ? "Visual search failed"
            : "Visual unavailable";
  const index =
    result.indexStatus?.stored === true
      ? "Index current"
      : result.indexStatus?.reason === "stale"
        ? "Index rebuilding"
        : result.indexStatus?.reason === "missing"
          ? "Index not saved"
          : result.indexStatus?.reason === "unavailable"
            ? "Index unavailable"
            : result.indexStatus?.stored === false
              ? "Index in memory"
              : null;
  const semantic =
    result.semanticEncoder?.status === "installed"
      ? `Semantic ${result.semanticEncoder.model?.id ?? "encoder"} ready`
      : "Semantic encoder not installed";

  return index ? [spoken, visual, semantic, index] : [spoken, visual, semantic];
}

export function indexedSearchNeedsRebuild(result: ProjectMediaSearchResult | null) {
  return (
    result?.indexStatus?.stored === false &&
    (result.indexStatus.reason === "stale" || result.indexStatus.reason === "missing")
  );
}

export function searchScopeUsesLocalMedia(scope: ProjectMediaSearchScope) {
  return scope === "both" || scope === "metadata";
}

export function searchScopeUsesLocalGeneratedAssets(scope: ProjectMediaSearchScope) {
  return scope === "both" || scope === "generated";
}

function generatedReferenceLabel(asset: GeneratedAsset) {
  const references = [
    ...asset.references.mediaIds,
    asset.references.firstFrameMediaId,
    asset.references.lastFrameMediaId,
  ].filter((value): value is string => Boolean(value));

  return references.length > 0 ? `references ${Array.from(new Set(references)).join(", ")}` : null;
}

function generatedLineageLabel(asset: GeneratedAsset) {
  const labels = [];
  if (asset.parentAssetId) {
    labels.push(`variation of ${asset.parentAssetId}`);
  }

  if (asset.retryOfAssetId) {
    labels.push(`retry of ${asset.retryOfAssetId}`);
  }

  return labels.length > 0 ? labels.join(" - ") : null;
}
