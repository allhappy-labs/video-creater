import { ArrowUpRight, AudioLines, Globe, Maximize2, Music, Settings, Sparkles, type LucideIcon } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { isActiveGeneratedAsset } from "@/lib/generation/assets";
import { generationCostLabel, upscaleRequestPlan, variationDraftsForAsset, videoAudioRequestPlan } from "@/lib/generation/clip-actions";
import { generatedAssetHasRerunnableModel, generationModelOptionsFromCatalog, generationModeFromAsset } from "@/lib/generation/provider-rules";
import {
  generatedUpscaleLimitReasonForAsset,
  importedUpscaleLimitReasonForMedia,
  sourceClipGenerationContextForItem,
  sourceClipUpscaleContextForItem,
} from "@/lib/generation/requests";
import { generatedAssetPlacementIntent, generatedReplacementPlacementIntent } from "@/lib/generation/timeline-placement";
import type { GenerationModel, MediaGenerationMode } from "@/lib/generation/types";
import { mediaContentKind } from "@/lib/media/media-filters";
import type { GeneratedAsset, GenerationPlacementIntent, MediaAsset, VideoProject } from "@/lib/project";
import type { TimelineItem } from "@/lib/timeline";
import { cn } from "@/lib/utils";
import { configurationBlocker, providerDisplayName, type ComposerMode } from "../panels/generate/composer-model";
import { useEditorEnvironment } from "../services/editor-environment";
import { providerUploadConfirmationCopy, providerUploadConfirmationRequired, useGenerationService } from "../services/generation-service";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { PropertySection } from "./controls/property-section";

type PendingAction = { readonly kind: "variations"; readonly count: number } | { readonly kind: "upscale" } | { readonly kind: "music" } | { readonly kind: "sfx" };

/** What the inline confirm step shows before a paid, networked generation starts. */
interface ConfirmPlan {
  readonly title: string;
  readonly detail: string;
  readonly startLabel: string;
  readonly cost: string;
  readonly mode: ComposerMode;
  readonly model: GenerationModel;
  readonly upload: Parameters<typeof providerUploadConfirmationRequired>[0];
  start(): Promise<boolean>;
}

interface ActionContext {
  readonly project: VideoProject;
  readonly item: TimelineItem;
  readonly asset: GeneratedAsset | null;
  readonly media: MediaAsset | null;
}

const variationCounts = [1, 2, 4] as const;

const composerModeFor: Readonly<Record<MediaGenerationMode, ComposerMode>> = { video: "video", image: "image", audio: "music" };

function placementText(project: VideoProject, placement: GenerationPlacementIntent): string {
  if (placement === "timeline") return "placed on the timeline when ready";
  if (placement.startsWith("replace:")) {
    const itemId = placement.slice("replace:".length);
    const label = project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === itemId)?.label.trim();
    return label ? `replaces “${label}” when ready` : "replaces its clip when ready";
  }
  return "added to Media when ready";
}

const plural = (count: number, noun: string) => `${count.toString()} ${noun}${count === 1 ? "" : "s"}`;

/** Builds the confirm step for an action, or the reason it cannot run. */
function confirmPlan(action: PendingAction, clip: ActionContext, service: ReturnType<typeof useGenerationService>, catalog: Parameters<typeof generationCostLabel>[1]): ConfirmPlan | { readonly blocked: string } {
  const { project, item, asset, media } = clip;
  if (action.kind === "variations") {
    if (!asset) return { blocked: "Only generated clips can make variations." };
    const drafts = variationDraftsForAsset(asset, action.count);
    const placement = placementText(project, generatedAssetPlacementIntent(asset));
    return {
      title: `Create ${plural(action.count, "variation")}`,
      detail: action.count === 1 ? `Runs the original prompt again, ${placement}.` : `${drafts.map((draft) => draft.name).join(", ")}, each ${placement}.`,
      startLabel: `Create ${plural(action.count, "variation")}`,
      cost: generationCostLabel(asset, catalog, drafts.length),
      mode: composerModeFor[generationModeFromAsset(asset)],
      model: asset.model,
      upload: { placementIntent: generatedAssetPlacementIntent(asset), model: asset.model, settings: asset.settings, references: asset.references },
      start: async () => (await service.createVariations(asset.id, action.count)) !== null,
    };
  }
  if (!media) return { blocked: "This clip has no media in the project." };
  if (action.kind === "upscale") {
    const context = asset ? undefined : sourceClipUpscaleContextForItem(item, media);
    const plan = upscaleRequestPlan(project, media.id, context);
    if ("blocked" in plan) return plan;
    const { request } = plan;
    const size = request.settings.width && request.settings.height ? ` ${request.settings.width.toString()}×${request.settings.height.toString()}` : "";
    return {
      title: "Upscale",
      detail: `Makes a${size} copy of ${context ? "this clip's range" : "this media"}, added to Media when ready.`,
      startLabel: "Upscale",
      cost: generationCostLabel(request, catalog),
      mode: mediaContentKind(media) === "image" ? "image" : "video",
      model: request.model,
      upload: request,
      start: async () => (await service.upscale(media.id, context)) !== null,
    };
  }
  const kind = action.kind;
  const context = sourceClipGenerationContextForItem(item);
  if (!context) return { blocked: "This clip is no longer on the timeline." };
  const plan = videoAudioRequestPlan(project, media.id, kind, context);
  if ("blocked" in plan) return plan;
  const title = kind === "music" ? "Generate music" : "Generate sound effects";
  return {
    title,
    detail: `Fits ${kind === "music" ? "music" : "sound"} to this clip, placed on an audio track under it when ready.`,
    startLabel: title,
    cost: generationCostLabel(plan.request, catalog),
    mode: kind,
    model: plan.request.model,
    upload: plan.request,
    start: async () => (await service.videoToAudio(media.id, kind, context)) !== null,
  };
}

const buttonClass =
  "h-8 rounded-control px-3 text-[12px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none";

/** The inline confirm step: what runs, where it lands, cost, provider and network, then Start. */
function ConfirmStep({ plan, onCancel, onStarted }: { readonly plan: ConfirmPlan; onCancel(): void; onStarted(): void }) {
  const store = useEditorStoreApi();
  const { generation, probeGeneration, openSettings } = useEditorEnvironment();
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const groupRef = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useEffect(() => probeGeneration(), [probeGeneration]);
  useEffect(() => groupRef.current?.focus(), []);
  const blocker = configurationBlocker(generation, plan.mode, plan.model, plan.title);
  const upload = providerUploadConfirmationRequired(plan.upload, generation.preferences);

  async function start() {
    setSubmitting(true);
    setError(null);
    const started = await plan.start();
    setSubmitting(false);
    if (started) {
      onStarted();
      return;
    }
    const failure = store.getState().lastError?.trim() || "Generation could not start.";
    setError(/[.!?]$/.test(failure) ? failure : `${failure}.`);
  }

  return (
    <div ref={groupRef} role="group" aria-labelledby={titleId} tabIndex={-1} className="flex flex-col gap-2 rounded-control bg-raised p-3 focus-visible:outline-none">
      <h4 id={titleId} className="text-[13px] font-semibold text-foreground">
        {plan.title}
      </h4>
      <p className="text-[12px] text-muted-foreground">{plan.detail}</p>
      <div className="flex items-center justify-between gap-2 text-[12px]">
        <span className="tabular-time font-medium text-foreground">{plan.cost}</span>
        <span className="flex min-w-0 items-center gap-1 text-muted-foreground">
          <Globe className="h-3.5 w-3.5 shrink-0" aria-hidden />
          <span className="truncate">Uses {providerDisplayName(generation, plan.model)} · network</span>
        </span>
      </div>
      {upload && <p className="text-[12px] text-muted-foreground">{providerUploadConfirmationCopy}</p>}
      {blocker && (
        <div role="status" className="flex flex-col gap-1.5 text-[12px] text-muted-foreground">
          <p>{blocker.message}</p>
          {blocker.kind === "action" && openSettings && (
            <button type="button" onClick={(event) => openSettings(blocker.target, event.currentTarget)} className={cn(buttonClass, "flex items-center gap-1.5 self-start bg-panel text-foreground hover:bg-hover")}>
              <Settings className="h-3.5 w-3.5" aria-hidden />
              {blocker.label}
            </button>
          )}
        </div>
      )}
      {error && (
        <p role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
      <div className="flex justify-end gap-2">
        <button type="button" disabled={submitting} onClick={onCancel} className={cn(buttonClass, "text-foreground hover:bg-hover disabled:opacity-50")}>
          Cancel
        </button>
        <button
          type="button"
          disabled={submitting || blocker !== null}
          onClick={() => void start()}
          className={cn(buttonClass, "bg-primary text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50")}
        >
          {submitting ? "Starting…" : upload ? `Upload and ${plan.startLabel.toLowerCase()}` : plan.startLabel}
        </button>
      </div>
    </div>
  );
}

interface ActionRowProps {
  readonly label: string;
  readonly icon: LucideIcon;
  /** Why the action cannot run; the row stays focusable with `aria-disabled` and shows it. */
  readonly reason: string | null;
  onClick(): void;
}

function ActionRow({ label, icon: Icon, reason, onClick }: ActionRowProps) {
  const reasonId = useId();
  return (
    <div className="flex flex-col gap-1">
      <button
        type="button"
        aria-disabled={reason !== null || undefined}
        aria-describedby={reason ? reasonId : undefined}
        onClick={() => reason === null && onClick()}
        className="flex h-8 items-center gap-2 rounded-control bg-raised px-2.5 text-left text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:cursor-not-allowed aria-disabled:opacity-50 aria-disabled:hover:bg-raised motion-reduce:transition-none"
      >
        <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
        {label}
      </button>
      {reason && (
        <p id={reasonId} className="text-[11px] text-dim">
          {reason}
        </p>
      )}
    </div>
  );
}

/**
 * Clip AI actions. Each paid action opens an inline confirm step with its cost and network notice
 * before anything starts, and every action is unavailable while the clip's generation is queued or
 * running. "Replace with generated…" opens the Generate view, which has its own cost footer.
 */
export function AiActionsSection({ item, asset, media }: { readonly item: TimelineItem; readonly asset: GeneratedAsset | null; readonly media: MediaAsset | null }) {
  const store = useEditorStoreApi();
  const service = useGenerationService();
  const { generation } = useEditorEnvironment();
  const project = useEditorStore((state) => state.project);
  const [pending, setPending] = useState<PendingAction | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const variationsReasonId = useId();

  const busy = asset && isActiveGeneratedAsset(asset) ? "Generation in progress" : null;
  const content = media ? mediaContentKind(media) : null;
  const visual = item.kind !== "audio_clip" && media !== null;
  const locked = project.timeline.tracks.some((track) => track.locked && track.items.some((candidate) => candidate.id === item.id));
  const catalog = generation.catalog;
  const variationsReason =
    busy ??
    (asset && !asset.prompt.trim() ? "This generation has no prompt to vary." : null) ??
    (asset && !generatedAssetHasRerunnableModel(asset, catalog, generationModelOptionsFromCatalog(catalog)) ? "This model can't make variations." : null);
  const upscaleReason =
    busy ?? (asset ? generatedUpscaleLimitReasonForAsset(asset) : null) ?? (media && content ? importedUpscaleLimitReasonForMedia({ ...media, kind: content }) : null);

  const plan = pending ? confirmPlan(pending, { project, item, asset, media }, service, catalog) : null;

  function open(action: PendingAction) {
    setNotice(null);
    setPending(action);
  }

  function replaceWithGenerated() {
    const state = store.getState();
    state.setGenerateView({ open: true, mode: content === "image" ? "image" : "video", placementIntent: generatedReplacementPlacementIntent(item.id) });
    state.setActiveTab("media");
    if (state.openSheetId !== null) state.openSheet("media");
  }

  return (
    <PropertySection title="AI actions">
      {notice && (
        <p role="status" className="text-[12px] text-muted-foreground">
          {notice}
        </p>
      )}
      {plan && "blocked" in plan && (
        <p role="alert" className="text-[12px] text-destructive">
          {plan.blocked}
        </p>
      )}
      {plan && !("blocked" in plan) ? (
        <ConfirmStep
          plan={plan}
          onCancel={() => setPending(null)}
          onStarted={() => {
            setPending(null);
            setNotice(`${plan.title} started. Progress shows in Media.`);
          }}
        />
      ) : (
        <div className="flex flex-col gap-2">
          {asset && (
            <div className="flex flex-col gap-1">
              <div role="group" aria-label="Create variations" aria-describedby={variationsReason ? variationsReasonId : undefined} className="flex items-center gap-2">
                <span className="flex flex-1 items-center gap-2 text-[12px] font-medium text-foreground">
                  <Sparkles className="h-3.5 w-3.5 text-muted-foreground" aria-hidden />
                  Create variations
                </span>
                {variationCounts.map((count) => (
                  <button
                    key={count}
                    type="button"
                    aria-label={`Create ${plural(count, "variation")}`}
                    aria-disabled={variationsReason !== null || undefined}
                    onClick={() => variationsReason === null && open({ kind: "variations", count })}
                    className="h-7 min-w-8 rounded-control bg-raised px-2 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring aria-disabled:cursor-not-allowed aria-disabled:opacity-50 aria-disabled:hover:bg-raised motion-reduce:transition-none"
                  >
                    {count}
                  </button>
                ))}
              </div>
              {variationsReason && (
                <p id={variationsReasonId} className="text-[11px] text-dim">
                  {variationsReason}
                </p>
              )}
            </div>
          )}
          {visual && (
            <ActionRow label="Replace with generated…" icon={ArrowUpRight} reason={busy ?? (locked ? "Unlock the track to replace this clip." : null)} onClick={replaceWithGenerated} />
          )}
          {visual && (content === "image" || content === "video") && <ActionRow label="Upscale" icon={Maximize2} reason={upscaleReason} onClick={() => open({ kind: "upscale" })} />}
          {visual && content === "video" && (
            <>
              <ActionRow label="Generate music from video" icon={Music} reason={busy} onClick={() => open({ kind: "music" })} />
              <ActionRow label="Generate sound effects from video" icon={AudioLines} reason={busy} onClick={() => open({ kind: "sfx" })} />
            </>
          )}
          {!asset && !visual && <p className="text-[12px] text-dim">AI actions need a video or image clip.</p>}
        </div>
      )}
    </PropertySection>
  );
}
