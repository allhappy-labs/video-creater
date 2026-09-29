import { ImageOff, MessageSquare } from "lucide-react";
import { useId, useMemo, useState } from "react";
import { generatedAssetForTimelineItem } from "@/lib/generation/assets";
import { generatedLineageText, generatedOutputReplacementBlocker, generationReferenceTiles, variationChoices, type VariationChoice } from "@/lib/generation/clip-actions";
import { generationModelLabel, generationModelValue, generationProviderDisplayName } from "@/lib/generation/provider-rules";
import { mediaDisplayName } from "@/lib/media/names";
import type { GeneratedAsset } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import { MediaThumbnail } from "../panels/media/media-thumbnail";
import { useEditorEnvironment } from "../services/editor-environment";
import { useGenerationService } from "../services/generation-service";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { AiActionsSection } from "./ai-actions";
import { PropertySection } from "./controls/property-section";

const statusCopy: Readonly<Partial<Record<GeneratedAsset["status"], string>>> = {
  queued: "Queued",
  running: "Generating…",
  failed: "Generation failed",
  cancelled: "Generation cancelled",
};

/** "Kling 2.1 · fal.ai" when the catalog names the model, else the provider/model label. */
function useModelLabel(asset: GeneratedAsset): string {
  const { catalog } = useEditorEnvironment().generation;
  const value = generationModelValue(asset.model);
  const named = Object.values(catalog ?? {})
    .flat()
    .find((model) => generationModelValue(model) === value && model.displayName?.trim());
  return named?.displayName ? `${named.displayName.trim()} · ${generationProviderDisplayName(named.provider)}` : generationModelLabel(asset.model);
}

/** A square thumbnail of project media, or a "missing" glyph once the media left the project. */
function Thumbnail({ mediaId, className }: { readonly mediaId: string | null; readonly className: string }) {
  const media = useEditorStore((state) => (mediaId ? (state.project.media.find((candidate) => candidate.id === mediaId) ?? null) : null));
  const projectDir = useEditorStore((state) => state.projectDir);
  return (
    <span className={`relative block shrink-0 overflow-hidden rounded-control bg-raised ${className}`}>
      {media ? (
        <MediaThumbnail media={media} projectDir={projectDir} />
      ) : (
        <span className="absolute inset-0 grid place-items-center text-dim">
          <ImageOff className="h-4 w-4" aria-hidden />
        </span>
      )}
    </span>
  );
}

function GeneratedDetailsSection({ asset }: { readonly asset: GeneratedAsset }) {
  const project = useEditorStore((state) => state.project);
  const modelLabel = useModelLabel(asset);
  const lineage = generatedLineageText(project, asset);
  const tiles = generationReferenceTiles(project, asset);
  const status = statusCopy[asset.status];
  return (
    <PropertySection title="Generated">
      {status && (
        <p role="status" className="text-[12px] font-medium text-muted-foreground">
          {status}
        </p>
      )}
      <dl className="grid grid-cols-[64px_minmax(0,1fr)] gap-x-3 gap-y-2 text-[12px]">
        <dt className="text-muted-foreground">Prompt</dt>
        <dd className="whitespace-pre-wrap break-words text-foreground">{asset.prompt.trim() || "No prompt"}</dd>
        <dt className="text-muted-foreground">Model</dt>
        <dd className="break-words text-foreground">{modelLabel}</dd>
        {lineage && (
          <>
            <dt className="text-muted-foreground">Lineage</dt>
            <dd className="break-words text-foreground">{lineage}</dd>
          </>
        )}
      </dl>
      {tiles.length > 0 && (
        <ul aria-label="References" className="grid grid-cols-4 gap-2">
          {tiles.map((tile) => {
            const name = tile.media ? mediaDisplayName(tile.media) : "Missing media";
            return (
              <li key={`${tile.role}-${tile.mediaId}`} className="flex min-w-0 flex-col gap-1" aria-label={`${tile.role}: ${name}`}>
                <Thumbnail mediaId={tile.media ? tile.mediaId : null} className="aspect-square w-full" />
                <span className="truncate text-[11px] text-muted-foreground" title={name}>
                  {tile.role}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </PropertySection>
  );
}

function VariationRow({ choice, current, blocker, reasonId, onUse }: { readonly choice: VariationChoice; readonly current: boolean; readonly blocker: string | null; readonly reasonId: string; onUse(): void }) {
  const ready = choice.status === "completed" && choice.mediaId !== null;
  return (
    <li className="flex min-w-0 items-center gap-2">
      <Thumbnail mediaId={choice.mediaId} className="h-9 w-9" />
      <span className="min-w-0 flex-1 truncate text-[12px] text-foreground" title={choice.label}>
        {choice.label}
      </span>
      {current ? (
        <span className="rounded-full bg-accent-soft px-2 py-0.5 text-[11px] font-medium text-primary">Current</span>
      ) : ready ? (
        <button
          type="button"
          aria-label={`Use ${choice.label}`}
          aria-disabled={blocker !== null || undefined}
          aria-describedby={blocker ? reasonId : undefined}
          onClick={() => blocker === null && onUse()}
          className="h-7 rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:cursor-not-allowed aria-disabled:opacity-50 aria-disabled:hover:bg-raised motion-reduce:transition-none"
        >
          Use this
        </button>
      ) : (
        <span className="text-[11px] text-dim">{statusCopy[choice.status] ?? "No output"}</span>
      )}
    </li>
  );
}

/** The variation family's outputs; "Use this" swaps one into the clip as one undo step. */
function VariationSetSection({ asset, item }: { readonly asset: GeneratedAsset; readonly item: TimelineItem }) {
  const service = useGenerationService();
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const [error, setError] = useState<string | null>(null);
  const store = useEditorStoreApi();
  const reasonId = useId();
  const choices = useMemo(() => variationChoices(project, asset), [project, asset]);
  if (choices.length < 2) return null;
  const currentMediaId = timelineItemSourceMediaId(item);
  const blockers = new Map(choices.map((choice) => [choice, choice.mediaId ? generatedOutputReplacementBlocker(project, projectDir, item.id, choice.mediaId) : null]));
  const shownReason = choices.map((choice) => (choice.mediaId !== currentMediaId && choice.status === "completed" ? blockers.get(choice) : null)).find(Boolean) ?? null;

  async function use(mediaId: string) {
    setError(null);
    if (!(await service.replaceWithOutput(item.id, mediaId))) setError(store.getState().lastError?.trim() || "The clip could not be replaced.");
  }

  return (
    <PropertySection title="Variations">
      <ul aria-label="Variation set" className="flex flex-col gap-1.5">
        {choices.map((choice) => (
          <VariationRow
            key={`${choice.assetId}-${choice.mediaId ?? "pending"}`}
            choice={choice}
            current={choice.mediaId !== null && choice.mediaId === currentMediaId}
            blocker={blockers.get(choice) ?? null}
            reasonId={reasonId}
            onUse={() => choice.mediaId && void use(choice.mediaId)}
          />
        ))}
      </ul>
      {shownReason && (
        <p id={reasonId} className="text-[11px] text-dim">
          {shownReason}
        </p>
      )}
      {error && (
        <p role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </PropertySection>
  );
}

/** Hands the clip to the AI tab as composer context. */
function AskAiSection({ item }: { readonly item: TimelineItem }) {
  const store = useEditorStoreApi();
  function ask() {
    store.getState().requestAgent({ itemIds: [item.id] });
  }
  return (
    <PropertySection title="Ask AI">
      <button
        type="button"
        onClick={ask}
        className="flex h-8 items-center gap-2 self-start rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none"
      >
        <MessageSquare className="h-3.5 w-3.5 text-muted-foreground" aria-hidden />
        Ask AI about this clip
      </button>
    </PropertySection>
  );
}

/**
 * The clip AI tab: generated details and the variation set switcher for generated clips, AI
 * actions (variations, replace with generated, upscale, music and sound effects from video), and
 * a hand-off to the AI tab.
 */
export function AiTabBody({ item }: { readonly item: TimelineItem }) {
  const project = useEditorStore((state) => state.project);
  const asset = useMemo(() => generatedAssetForTimelineItem(project, item), [project, item]);
  const mediaId = timelineItemSourceMediaId(item);
  const media = useMemo(() => (mediaId ? (project.media.find((candidate) => candidate.id === mediaId) ?? null) : null), [project, mediaId]);
  return (
    <>
      {asset && <GeneratedDetailsSection asset={asset} />}
      {asset && <VariationSetSection asset={asset} item={item} />}
      <AiActionsSection item={item} asset={asset} media={media} />
      <AskAiSection item={item} />
    </>
  );
}
