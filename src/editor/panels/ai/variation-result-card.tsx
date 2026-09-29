import { ImageOff } from "lucide-react";
import { useId, useMemo, useState } from "react";
import { generatedOutputReplacementBlocker, variationChoices, type VariationChoice } from "@/lib/generation/clip-actions";
import type { ProjectAction, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { timelineItemSourceMediaId } from "@/lib/timeline-ops/item-properties";
import { useGenerationService } from "../../services/generation-service";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { MediaThumbnail } from "../media/media-thumbnail";
import { CardButton } from "./card-parts";

interface VariationSet {
  readonly item: TimelineItem;
  readonly choices: readonly VariationChoice[];
}

/**
 * The variation set an applied bundle produced: a recorded generation whose family has two or more
 * outputs, and the clip it sits in (the bundle's replacement target, or a clip showing the family).
 */
function variationSetFor(project: VideoProject, actions: readonly ProjectAction[]): VariationSet | null {
  const items = project.timeline.tracks.flatMap((track) => track.items);
  const replaced = actions.flatMap((action) => (action.type === "replaceTimelineItemWithGeneratedOutput" ? [action.replacement.itemId] : []));
  for (const action of actions) {
    if (action.type !== "recordGeneratedAsset") continue;
    const asset = project.generatedAssets.find((candidate) => candidate.id === action.asset.id);
    if (!asset) continue;
    const choices = variationChoices(project, asset);
    if (choices.length < 2) continue;
    const family = new Set(choices.map((choice) => choice.assetId));
    const item = items.find((candidate) => {
      const generatedAssetId = candidate.properties.generatedAssetId;
      return replaced.includes(candidate.id) || (typeof generatedAssetId === "string" && family.has(generatedAssetId));
    });
    if (item) return { item, choices };
  }
  return null;
}

function VariationThumbnail({ mediaId }: { readonly mediaId: string | null }) {
  const media = useEditorStore((state) => (mediaId ? (state.project.media.find((candidate) => candidate.id === mediaId) ?? null) : null));
  const projectDir = useEditorStore((state) => state.projectDir);
  return (
    <span className="relative block aspect-video w-full overflow-hidden rounded bg-panel">
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

/** Variation thumbnails with "Use this", which swaps the output into the clip as one user edit. */
export function VariationResultCard({ actions }: { readonly actions: readonly ProjectAction[] }) {
  const project = useEditorStore((state) => state.project);
  const projectDir = useEditorStore((state) => state.projectDir);
  const store = useEditorStoreApi();
  const service = useGenerationService();
  const reasonId = useId();
  const [error, setError] = useState<string | null>(null);
  const set = useMemo(() => variationSetFor(project, actions), [project, actions]);
  if (!set) return null;
  const { item, choices } = set;
  const currentMediaId = timelineItemSourceMediaId(item);

  async function use(mediaId: string) {
    setError(null);
    if (!(await service.replaceWithOutput(item.id, mediaId))) setError(store.getState().lastError?.trim() || "The clip couldn't be replaced.");
  }

  const blockers = choices.map((choice) => (choice.mediaId ? generatedOutputReplacementBlocker(project, projectDir, item.id, choice.mediaId) : null));
  const shownReason = choices.map((choice, index) => (choice.mediaId !== currentMediaId && choice.status === "completed" ? blockers[index] : null)).find(Boolean) ?? null;

  return (
    <article aria-label="Variations" className="mt-2 overflow-hidden rounded-[10px] bg-raised">
      <header className="px-3 py-2.5 text-[13px] font-semibold text-foreground">Variations</header>
      <ul className="grid grid-cols-2 gap-2 px-3 pb-3">
        {choices.map((choice, index) => {
          const current = choice.mediaId !== null && choice.mediaId === currentMediaId;
          const ready = choice.status === "completed" && choice.mediaId !== null;
          const blocker = blockers[index] ?? null;
          return (
            <li key={`${choice.assetId}-${choice.mediaId ?? "pending"}`} className="flex min-w-0 flex-col gap-1">
              <VariationThumbnail mediaId={ready ? choice.mediaId : null} />
              <span className="truncate text-[11.5px] text-muted-foreground" title={choice.label}>
                {choice.label}
              </span>
              {current ? (
                <span className="self-start rounded-full bg-accent-soft px-2 py-0.5 text-[11px] font-medium text-primary">Current</span>
              ) : ready ? (
                <CardButton
                  aria-label={`Use ${choice.label}`}
                  aria-disabled={blocker !== null || undefined}
                  aria-describedby={blocker ? reasonId : undefined}
                  onClick={() => {
                    if (blocker === null && choice.mediaId) void use(choice.mediaId);
                  }}
                  className="h-7 self-start"
                >
                  Use this
                </CardButton>
              ) : (
                <span className="text-[11px] text-dim">{choice.status === "failed" ? "Generation failed" : choice.status === "cancelled" ? "Cancelled" : "Generating…"}</span>
              )}
            </li>
          );
        })}
      </ul>
      {shownReason && (
        <p id={reasonId} className="px-3 pb-3 text-[11px] text-dim">
          {shownReason}
        </p>
      )}
      {error && (
        <p role="alert" className="px-3 pb-3 text-[12px] text-destructive">
          {error}
        </p>
      )}
    </article>
  );
}
