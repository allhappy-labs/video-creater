import { Plus, X } from "lucide-react";
import { useState, type DragEvent } from "react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuLabel, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { generationReferencePromptTags, trailingReferenceTagQuery } from "@/lib/generation/references";
import type { GenerationReferencePromptTag } from "@/lib/generation/types";
import { mediaDisplayName } from "@/lib/media/names";
import type { MediaAsset } from "@/lib/project";
import { cn } from "@/lib/utils";
import { hasAssetDragData, readAssetDragData } from "../../timeline/drag-data";
import { MediaThumbnail } from "../media/media-thumbnail";
import type { ComposerState, ReferencePane, ReferenceSupport } from "./composer-model";
import { fieldLabelClass } from "./generation-options";

const tileClass = "relative block h-14 w-14 shrink-0 overflow-hidden rounded-control";

/** Accepts media dragged from the grid when it is one of `accepted`. */
function useSlotDrop(accepted: readonly MediaAsset[], onDrop: (mediaId: string) => void) {
  const [over, setOver] = useState(false);
  return {
    over,
    handlers: {
      onDragOver(event: DragEvent<HTMLElement>) {
        if (!hasAssetDragData(event.dataTransfer)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        setOver(true);
      },
      onDragLeave: () => setOver(false),
      onDrop(event: DragEvent<HTMLElement>) {
        setOver(false);
        const payload = readAssetDragData(event.dataTransfer);
        if (payload?.kind !== "media" || !accepted.some((asset) => asset.id === payload.id)) return;
        event.preventDefault();
        onDrop(payload.id);
      },
    },
  };
}

/** A filled slot: the media thumbnail with a remove button. */
function FilledTile({ media, projectDir, removeLabel, onRemove }: { readonly media: MediaAsset; readonly projectDir: string; readonly removeLabel: string; onRemove(): void }) {
  return (
    <span className={cn(tileClass, "bg-raised")} title={mediaDisplayName(media)}>
      <MediaThumbnail media={media} projectDir={projectDir} />
      <button
        type="button"
        aria-label={removeLabel}
        onClick={onRemove}
        className="absolute right-0.5 top-0.5 grid h-5 w-5 place-items-center rounded-full bg-background/85 text-foreground hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <X className="h-3 w-3" aria-hidden />
      </button>
    </span>
  );
}

/** The dashed `+` tile: picks from `choices`, or takes a drop from the media grid. */
function AddTile({ label, choices, emptyCopy, onPick }: { readonly label: string; readonly choices: readonly MediaAsset[]; readonly emptyCopy: string; onPick(mediaId: string): void }) {
  const drop = useSlotDrop(choices, onPick);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={label}
          {...drop.handlers}
          className={cn(
            tileClass,
            "grid place-items-center border border-dashed border-line text-muted-foreground transition-colors hover:border-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
            drop.over && "border-primary bg-accent-soft text-foreground",
          )}
        >
          <Plus className="h-4 w-4" aria-hidden />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" aria-label={label} className="max-h-72 overflow-y-auto">
        <DropdownMenuLabel>{label}</DropdownMenuLabel>
        {choices.length === 0 ? (
          <DropdownMenuItem disabled>{emptyCopy}</DropdownMenuItem>
        ) : (
          choices.map((asset) => (
            <DropdownMenuItem key={asset.id} onSelect={() => onPick(asset.id)}>
              <span className="truncate">{mediaDisplayName(asset)}</span>
            </DropdownMenuItem>
          ))
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function SingleSlot({ label, mediaId, choices, media, projectDir, emptyCopy, onChange }: {
  readonly label: string;
  readonly mediaId: string | null;
  readonly choices: readonly MediaAsset[];
  readonly media: readonly MediaAsset[];
  readonly projectDir: string;
  readonly emptyCopy: string;
  onChange(mediaId: string | null): void;
}) {
  const selected = mediaId ? media.find((asset) => asset.id === mediaId) : undefined;
  return (
    <div className="flex min-w-0 items-center gap-2">
      {selected ? (
        <FilledTile media={selected} projectDir={projectDir} removeLabel={`Remove ${label.toLowerCase()} ${mediaDisplayName(selected)}`} onRemove={() => onChange(null)} />
      ) : (
        <AddTile label={`Choose ${label.toLowerCase()}`} choices={choices} emptyCopy={emptyCopy} onPick={onChange} />
      )}
      <span className="min-w-0 truncate text-[12px] text-muted-foreground">{selected ? mediaDisplayName(selected) : label}</span>
    </div>
  );
}

interface ReferenceSlotsProps {
  readonly state: ComposerState;
  readonly support: ReferenceSupport;
  readonly media: readonly MediaAsset[];
  readonly projectDir: string;
  /** The model's reference limit problem, shown under the slots. */
  readonly limitMessage: string | null;
  onChange(patch: Partial<ComposerState>): void;
}

/**
 * Input media for the selected model: a source video, first and last frames, and references.
 * Each slot renders only when the model takes that input; models that take frames or references
 * (not both) get a pane switch.
 */
export function ReferenceSlots({ state, support, media, projectDir, limitMessage, onChange }: ReferenceSlotsProps) {
  const showPaneSwitch = support.exclusive && support.frames && support.references;
  const pane: ReferencePane = showPaneSwitch ? state.referencePane : support.frames ? "frames" : "references";
  const showFrames = support.frames && (!showPaneSwitch || pane === "frames");
  const showReferences = support.references && (!showPaneSwitch || pane === "references");
  if (!support.sourceVideo && !showFrames && !showReferences && !showPaneSwitch) return null;
  const selectedReferences = state.referenceIds.flatMap((id) => media.find((asset) => asset.id === id) ?? []);
  const addable = support.referenceMedia.filter((asset) => !state.referenceIds.includes(asset.id));

  return (
    <div role="group" aria-label="Input media" className="flex flex-col gap-2.5">
      {showPaneSwitch && (
        <div role="radiogroup" aria-label="Input type" className="flex gap-1">
          {(["frames", "references"] as const).map((value) => (
            <button
              key={value}
              type="button"
              role="radio"
              aria-checked={pane === value}
              onClick={() => onChange(value === "frames" ? { referencePane: value, referenceIds: [] } : { referencePane: value, firstFrameId: null, lastFrameId: null })}
              className={cn(
                "h-7 rounded-full px-3 text-[12px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                pane === value ? "bg-accent-soft text-foreground" : "text-muted-foreground hover:bg-hover hover:text-foreground",
              )}
            >
              {value === "frames" ? "First & last frame" : "References"}
            </button>
          ))}
        </div>
      )}
      {support.sourceVideo && (
        <SingleSlot label="Source video" mediaId={state.sourceVideoId} choices={support.sourceVideoMedia} media={media} projectDir={projectDir} emptyCopy="No videos in this project" onChange={(sourceVideoId) => onChange({ sourceVideoId })} />
      )}
      {showFrames && (
        <div className="grid grid-cols-2 gap-2">
          <SingleSlot label="First frame" mediaId={state.firstFrameId} choices={support.frameMedia} media={media} projectDir={projectDir} emptyCopy="No images in this project" onChange={(firstFrameId) => onChange({ firstFrameId })} />
          <SingleSlot label="Last frame" mediaId={state.lastFrameId} choices={support.frameMedia} media={media} projectDir={projectDir} emptyCopy="No images in this project" onChange={(lastFrameId) => onChange({ lastFrameId })} />
        </div>
      )}
      {showReferences && (
        <div className="flex flex-col gap-1.5">
          <span className={fieldLabelClass}>References</span>
          <div className="flex flex-wrap items-center gap-2">
            {selectedReferences.map((asset) => (
              <FilledTile
                key={asset.id}
                media={asset}
                projectDir={projectDir}
                removeLabel={`Remove reference ${mediaDisplayName(asset)}`}
                onRemove={() => onChange({ referenceIds: state.referenceIds.filter((id) => id !== asset.id) })}
              />
            ))}
            <AddTile label="Add reference" choices={addable} emptyCopy="No usable media in this project" onPick={(id) => onChange({ referenceIds: [...state.referenceIds, id] })} />
            {selectedReferences.length === 0 && <span className="text-[12px] text-dim">Style, subject or composition</span>}
          </div>
        </div>
      )}
      {limitMessage && (
        <p role="status" className="text-[12px] text-warning">
          {limitMessage}
        </p>
      )}
    </div>
  );
}

/** The `@` tags for selected references that match the tag being typed at the end of the prompt. */
export function visibleReferenceTags(prompt: string, referenceIds: readonly string[], media: readonly MediaAsset[]): GenerationReferencePromptTag[] {
  const query = trailingReferenceTagQuery(prompt);
  if (query === null) return [];
  return generationReferencePromptTags(referenceIds, media).filter((tag) => tag.tag.slice(1).toLowerCase().startsWith(query.toLowerCase()));
}

/** Pre-cut reference tag autocomplete under the prompt. */
export function ReferenceTagPicker({ tags, media, onInsert }: { readonly tags: readonly GenerationReferencePromptTag[]; readonly media: readonly MediaAsset[]; onInsert(tag: string): void }) {
  if (tags.length === 0) return null;
  return (
    <div role="listbox" aria-label="Reference tags" className="flex flex-col gap-0.5 rounded-control border border-line bg-popover p-1 shadow-md">
      {tags.map((tag) => {
        const asset = media.find((candidate) => candidate.id === tag.mediaId);
        return (
          <button
            key={`${tag.tag}-${tag.mediaId}`}
            type="button"
            role="option"
            aria-selected="false"
            onClick={() => onInsert(tag.tag)}
            className="flex min-w-0 items-center justify-between gap-2 rounded-md px-2 py-1 text-left text-[12px] hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <span className="font-medium text-foreground">{tag.tag}</span>
            <span className="truncate text-muted-foreground">{asset ? mediaDisplayName(asset) : tag.kindLabel}</span>
          </button>
        );
      })}
    </div>
  );
}
