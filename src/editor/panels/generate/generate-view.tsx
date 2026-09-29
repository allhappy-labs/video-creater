import { ArrowLeft, Globe, Settings, Sparkles } from "lucide-react";
import { useEffect, useId, useMemo, useState, type KeyboardEvent } from "react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import {
  generationModelLabel,
  generationModelValue,
  generationPromptPlaceholder,
  generationProviderDisplayName,
} from "@/lib/generation/provider-rules";
import { generationReferenceLimitMessage, promptWithInsertedReferenceTag, typedGenerationReferenceMediaRefs } from "@/lib/generation/references";
import type { GenerationModelOption, MediaGenerationRequest } from "@/lib/generation/types";
import type { GenerationPlacementIntent } from "@/lib/project";
import { cn } from "@/lib/utils";
import { useEditorEnvironment } from "../../services/editor-environment";
import { providerUploadConfirmationCopy, providerUploadConfirmationRequired, useGenerationService } from "../../services/generation-service";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { MediaDialog } from "../media/folder-dialogs";
import {
  audioComposerModes,
  buildGenerationRequest,
  composerCostLabel,
  composerFields,
  composerModels,
  configurationBlocker,
  generationModeFor,
  initialComposerState,
  mediaComposerModes,
  providerDisplayName,
  readinessReason,
  referenceSupport,
  selectedComposerModel,
  type ComposerMode,
  type ComposerState,
} from "./composer-model";
import { ComposerField, GenerationOptions } from "./generation-options";
import { ReferenceSlots, ReferenceTagPicker, visibleReferenceTags } from "./reference-slots";

interface GenerateViewProps {
  /** Media offers Video/Image; Audio offers Music/SFX/Voice. */
  readonly tab: "media" | "audio";
  /** The chip to start on; ignored when the tab doesn't offer it. */
  readonly initialMode?: ComposerMode;
  /** Folder the output lands in (the open Media folder). */
  readonly targetFolderId: string | null;
  /** `replace:<itemId>` swaps the output into that clip when it completes; defaults to the library. */
  readonly placementIntent?: GenerationPlacementIntent | undefined;
  onClose(): void;
}

const replacePrefix = "replace:";

/** "Kling 2.1 · fal.ai" for catalog models with a display name, else the provider/model label. */
function modelOptionLabel(model: GenerationModelOption): string {
  return model.displayName?.trim() ? `${model.displayName.trim()} · ${generationProviderDisplayName(model.provider)}` : generationModelLabel(model);
}

/**
 * The Generate sub-view that replaces the Media or Audio grid: mode chips, prompt, input media,
 * model and its options, then a footer with the estimated cost, the network notice and Generate.
 * Generation starts only from Generate; referenced local media asks for upload consent first.
 */
export function GenerateView({ tab, initialMode, targetFolderId, placementIntent, onClose }: GenerateViewProps) {
  const store = useEditorStoreApi();
  const service = useGenerationService();
  const environment = useEditorEnvironment();
  const media = useEditorStore((state) => state.project.media);
  const projectDir = useEditorStore((state) => state.projectDir);
  const replaceItemId = placementIntent?.startsWith(replacePrefix) ? placementIntent.slice(replacePrefix.length) : null;
  // The clip's label, or null once it leaves the timeline (the output then goes to the library).
  const replaceLabel = useEditorStore((state) =>
    replaceItemId === null ? null : (state.project.timeline.tracks.flatMap((track) => track.items).find((item) => item.id === replaceItemId)?.label ?? null),
  );
  const placement: GenerationPlacementIntent = replaceItemId !== null && replaceLabel === null ? "library" : (placementIntent ?? "library");
  const modes = tab === "audio" ? audioComposerModes : mediaComposerModes;
  const [state, setState] = useState<ComposerState>(() =>
    initialComposerState(modes.find((option) => option.value === initialMode)?.value ?? modes[0]?.value ?? "video"),
  );
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [pendingUpload, setPendingUpload] = useState<MediaGenerationRequest | null>(null);
  const promptId = useId();
  const modelLabelId = useId();
  const reasonId = useId();

  const { generation, probeGeneration } = environment;
  useEffect(() => probeGeneration(), [probeGeneration]);
  const models = useMemo(() => composerModels(state.mode, generation.catalog), [generation.catalog, state.mode]);
  const model = selectedComposerModel(state, models);
  const modeLabel = modes.find((option) => option.value === state.mode)?.label ?? "";
  const blocker = configurationBlocker(generation, state.mode, model, modeLabel);
  const reason = readinessReason(state, model, media, blocker);
  const generationMode = generationModeFor(state.mode);
  const support = model ? referenceSupport(state, model, media) : null;
  const fields = model ? composerFields(state, model) : null;
  const limitMessage = model ? generationReferenceLimitMessage(generationMode, model, typedGenerationReferenceMediaRefs(generationMode, model, state.referenceIds, media), media) : null;
  const tags = visibleReferenceTags(state.prompt, state.referenceIds, media);
  const providerName = model ? providerDisplayName(generation, model) : null;

  const update = (patch: Partial<ComposerState>) => setState((current) => ({ ...current, ...patch }));

  async function start(request: MediaGenerationRequest) {
    setSubmitting(true);
    setSubmitError(null);
    const started = await service.startGeneration(request);
    setSubmitting(false);
    if (started) {
      onClose();
      return;
    }
    const failure = store.getState().lastError?.trim() || "Generation could not start.";
    const sentence = /[.!?]$/.test(failure) ? failure : `${failure}.`;
    setSubmitError(providerName ? `${sentence} Check ${providerName} in Settings, then try again.` : sentence);
  }

  function submit() {
    if (!model || reason !== null || submitting) return;
    const request = buildGenerationRequest(state, model, media, targetFolderId, placement);
    if (!request) return;
    if (providerUploadConfirmationRequired(request, generation.preferences)) setPendingUpload(request);
    else void start(request);
  }

  function onPromptKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    const [first] = tags;
    if (event.key === "Enter" && first) {
      event.preventDefault();
      update({ prompt: promptWithInsertedReferenceTag(state.prompt, first.tag) });
    }
  }

  return (
    <section aria-label="Generate" className="flex min-h-full flex-col">
      <div className="flex flex-col gap-3 p-3 pb-4">
        <div className="flex items-center gap-1">
          <button
            type="button"
            aria-label={tab === "audio" ? "Back to audio" : "Back to media"}
            onClick={onClose}
            className="grid h-8 w-8 place-items-center rounded-control text-muted-foreground hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <ArrowLeft className="h-4 w-4" aria-hidden />
          </button>
          <h2 className="text-[14px] font-semibold">Generate</h2>
        </div>

        {replaceLabel !== null && (
          <p role="status" className="rounded-control bg-accent-soft px-3 py-2 text-[12px] text-foreground">
            Will replace {replaceLabel.trim() || "the selected timeline clip"}.
          </p>
        )}

        <div role="radiogroup" aria-label="Generation type" className="flex flex-wrap gap-1">
          {modes.map((option) => (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={state.mode === option.value}
              onClick={() => update({ mode: option.value })}
              className={cn(
                "h-7 rounded-full px-3 text-[12px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
                state.mode === option.value ? "bg-raised text-foreground" : "text-muted-foreground hover:bg-hover hover:text-foreground",
              )}
            >
              {option.label}
            </button>
          ))}
        </div>

        {blocker && (
          <div role="status" className="flex flex-col gap-2 rounded-control bg-raised px-3 py-2.5 text-[12px] text-muted-foreground">
            <p>{blocker.message}</p>
            {blocker.kind === "action" && environment.openSettings && (
              <button
                type="button"
                onClick={(event) => environment.openSettings?.(blocker.target, event.currentTarget)}
                className="flex h-7 items-center gap-1.5 self-start rounded-control bg-panel px-2.5 text-[12px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <Settings className="h-3.5 w-3.5" aria-hidden />
                {blocker.label}
              </button>
            )}
          </div>
        )}

        <div className="flex flex-col gap-1.5">
          <label htmlFor={promptId} className="sr-only">
            Prompt
          </label>
          <textarea
            id={promptId}
            rows={4}
            value={state.prompt}
            placeholder={model ? generationPromptPlaceholder(generationMode, model) : "Describe what to generate"}
            onChange={(event) => update({ prompt: event.target.value })}
            onKeyDown={onPromptKeyDown}
            className="min-h-24 w-full resize-y rounded-panel bg-raised px-3 py-2.5 text-[13px] leading-relaxed text-foreground placeholder:text-dim focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          />
          <ReferenceTagPicker tags={tags} media={media} onInsert={(tag) => update({ prompt: promptWithInsertedReferenceTag(state.prompt, tag) })} />
        </div>

        {support && <ReferenceSlots state={state} support={support} media={media} projectDir={projectDir} limitMessage={limitMessage} onChange={update} />}

        {models.length > 0 && model && (
          <ComposerField label="Model" labelId={modelLabelId}>
            <Select value={generationModelValue(model)} onValueChange={(value) => update({ modelValues: { ...state.modelValues, [state.mode]: value } })}>
              <SelectTrigger aria-labelledby={modelLabelId} className="h-8 text-[13px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {models.map((option) => (
                  <SelectItem key={generationModelValue(option)} value={generationModelValue(option)}>
                    {modelOptionLabel(option)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </ComposerField>
        )}

        {fields && <GenerationOptions state={state} fields={fields} onChange={update} />}
      </div>

      <div className="sticky bottom-0 mt-auto flex flex-col gap-2 border-t border-line bg-panel p-3">
        {submitError && (
          <p role="alert" className="text-[12px] text-destructive">
            {submitError}
          </p>
        )}
        <div className="flex items-center justify-between gap-2 text-[12px]">
          <span className="tabular-time font-medium text-foreground">{model ? composerCostLabel(state, model) : "Est. varies"}</span>
          {providerName && (
            <span className="flex min-w-0 items-center gap-1 text-muted-foreground">
              <Globe className="h-3.5 w-3.5 shrink-0" aria-hidden />
              <span className="truncate">Uses {providerName} · network</span>
            </span>
          )}
        </div>
        {reason && (
          <p id={reasonId} className="text-[12px] text-muted-foreground">
            {reason}
          </p>
        )}
        <button
          type="button"
          disabled={reason !== null || submitting}
          aria-describedby={reason ? reasonId : undefined}
          onClick={submit}
          className="flex h-9 items-center justify-center gap-2 rounded-control bg-primary text-[13px] font-medium text-primary-foreground hover:bg-primary/90 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50"
        >
          <Sparkles className="h-4 w-4" aria-hidden />
          {submitting ? "Starting…" : "Generate"}
        </button>
      </div>

      {pendingUpload && (
        <MediaDialog
          title="Upload referenced media?"
          description={providerUploadConfirmationCopy}
          submitLabel="Upload and generate"
          busy={submitting}
          onClose={() => setPendingUpload(null)}
          onSubmit={() => {
            const request = pendingUpload;
            setPendingUpload(null);
            void start(request);
          }}
        />
      )}
    </section>
  );
}
