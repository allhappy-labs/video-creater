import { AudioLines, Pause, Users, type LucideIcon } from "lucide-react";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode, type Ref, type RefObject } from "react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { silenceSummary } from "@/lib/audio/cleanup-summary";
import { pluralize } from "@/lib/format";
import { audioDenoise, denoiseStatusLabel } from "@/lib/properties/audio-properties";
import type { TimelineItem } from "@/lib/timeline";
import { cn } from "@/lib/utils";
import { useSpeakerRegistry } from "../../properties/use-speaker-registry";
import { useEditorEnvironment } from "../../services/editor-environment";
import { useSpeechService } from "../../services/speech-service";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import {
  cleanupTargetOptions,
  resolveCleanupTarget,
  speechAnalysisItem,
  targetAudioItems,
  targetSilenceRanges,
  type CleanupTarget,
} from "./cleanup-target";
import { SilenceReviewDialog } from "./silence-review-dialog";
import { SpeakersDialog } from "./speakers-dialog";

type AnalysisStatus = "analyzing" | "failed";

const cardButtonClass =
  "h-7 rounded-control px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40 motion-reduce:transition-none";

interface CleanupCardProps {
  readonly icon: LucideIcon;
  readonly title: string;
  readonly description: string;
  readonly children: ReactNode;
  /** Set for a card other panels route focus to; the card becomes programmatically focusable. */
  readonly cardRef?: Ref<HTMLLIElement>;
  /** Briefly rings the card after another panel routed here, so mouse users see where they landed. */
  readonly highlighted?: boolean;
}

function CleanupCard({ icon: Icon, title, description, children, cardRef, highlighted = false }: CleanupCardProps) {
  const titleId = useId();
  return (
    <li
      ref={cardRef}
      aria-labelledby={titleId}
      tabIndex={cardRef ? -1 : undefined}
      data-highlighted={highlighted ? "true" : undefined}
      className={cn(
        "flex items-center gap-3 rounded-control bg-raised p-2.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        highlighted && "ring-2 ring-accent motion-safe:animate-pulse",
      )}
    >
      <span aria-hidden className="grid h-8 w-8 shrink-0 place-items-center rounded-md bg-panel text-muted-foreground">
        <Icon className="h-4 w-4" />
      </span>
      <div className="min-w-0 flex-1">
        <p id={titleId} className="truncate text-[13px] font-medium text-foreground">
          {title}
        </p>
        <p className="truncate text-[11px] text-dim">{description}</p>
      </div>
      <div className="flex shrink-0 items-center gap-1">{children}</div>
    </li>
  );
}

const highlightMilliseconds = 1600;

/**
 * Consumes `ui.pendingCleanupFocus` ("Remove silences…" in Properties): focuses Review, or the card
 * when it is disabled, and returns whether the card is highlighted.
 */
function useRemoveSilencesFocus(cardRef: RefObject<HTMLLIElement | null>, reviewRef: RefObject<HTMLButtonElement | null>): boolean {
  const store = useEditorStoreApi();
  const pending = useEditorStore((state) => state.pendingCleanupFocus);
  const [highlighted, setHighlighted] = useState(false);
  useEffect(() => {
    if (pending !== "removeSilences") return;
    const card = cardRef.current;
    const review = reviewRef.current;
    const reduceMotion = typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    card?.scrollIntoView?.({ block: "nearest", behavior: reduceMotion ? "auto" : "smooth" });
    if (review && !review.disabled) review.focus();
    else card?.focus();
    setHighlighted(true);
    store.getState().setPendingCleanupFocus(null);
  }, [store, pending, cardRef, reviewRef]);
  useEffect(() => {
    if (!highlighted) return undefined;
    const timer = window.setTimeout(() => setHighlighted(false), highlightMilliseconds);
    return () => window.clearTimeout(timer);
  }, [highlighted]);
  return highlighted;
}

function RemoveSilencesCard({ target }: { readonly target: CleanupTarget | null }) {
  const project = useEditorStore((state) => state.project);
  const speech = useSpeechService();
  const [reviewOpen, setReviewOpen] = useState(false);
  const cardRef = useRef<HTMLLIElement>(null);
  const reviewRef = useRef<HTMLButtonElement>(null);
  const ranges = useMemo(() => (target ? targetSilenceRanges(project, target) : []), [project, target]);
  const { count, secondsSaved } = silenceSummary(ranges);
  const description = count > 0 ? `${pluralize(count, "pause")} · saves ${secondsSaved.toString()}s` : "No pauses detected";
  const highlighted = useRemoveSilencesFocus(cardRef, reviewRef);
  return (
    <CleanupCard icon={Pause} title="Remove silences" description={description} cardRef={cardRef} highlighted={highlighted}>
      <button ref={reviewRef} type="button" aria-label="Review pauses" disabled={count === 0} onClick={() => setReviewOpen(true)} className={cardButtonClass}>
        Review
      </button>
      <SilenceReviewDialog
        open={reviewOpen && count > 0}
        ranges={ranges}
        onOpenChange={setReviewOpen}
        onApply={async (checked) => {
          const applied = await speech.removeSilences(checked);
          if (applied) setReviewOpen(false);
          return applied;
        }}
      />
    </CleanupCard>
  );
}

function ReduceNoiseCard({ audioItems }: { readonly audioItems: readonly TimelineItem[] }) {
  const speech = useSpeechService();
  const states = audioItems.map((item) => ({ item, denoise: audioDenoise(item) }));
  const allOn = states.length > 0 && states.every((entry) => entry.denoise.enabled);
  const firstStatus = states[0]?.denoise.status ?? null;
  const description =
    states.length === 0 ? "No audio clips for this target" : allOn ? `On · ${denoiseStatusLabel(firstStatus)}` : "Local model · reversible";
  return (
    <CleanupCard icon={AudioLines} title="Reduce noise" description={description}>
      {allOn ? (
        <button
          type="button"
          aria-label="Turn off noise reduction"
          onClick={() => void speech.setDenoise(states.map((entry) => entry.item.id), false)}
          className={cardButtonClass}
        >
          Turn off
        </button>
      ) : (
        <button
          type="button"
          aria-label="Apply noise reduction"
          disabled={states.length === 0}
          onClick={() => void speech.setDenoise(states.filter((entry) => !entry.denoise.enabled).map((entry) => entry.item.id), true)}
          className={cardButtonClass}
        >
          Apply
        </button>
      )}
    </CleanupCard>
  );
}

function speechAnalysisFailed(item: TimelineItem | null): boolean {
  const analysis = item?.properties.speechAnalysis;
  return typeof analysis === "object" && analysis !== null && (analysis as Record<string, unknown>).status === "failed";
}

interface DetectSpeakersCardProps {
  readonly analysisItem: TimelineItem | null;
  readonly modelReady: boolean;
}

function DetectSpeakersCard({ analysisItem, modelReady }: DetectSpeakersCardProps) {
  const speech = useSpeechService();
  const registry = useSpeakerRegistry();
  const [statuses, setStatuses] = useState<Readonly<Record<string, AnalysisStatus>>>({});
  const [renameOpen, setRenameOpen] = useState(false);
  const status = analysisItem ? statuses[analysisItem.id] : undefined;
  const failed = status === "failed" || (status === undefined && speechAnalysisFailed(analysisItem));
  const speakerCount = registry.speakers.length;

  const analyze = async (item: TimelineItem) => {
    setStatuses((current) => ({ ...current, [item.id]: "analyzing" }));
    const analyzed = await speech.analyzeSpeakers(item.id);
    setStatuses((current) => {
      const { [item.id]: _previous, ...rest } = current;
      return analyzed ? rest : { ...rest, [item.id]: "failed" };
    });
    if (analyzed) registry.reload();
  };

  let description: string;
  if (!registry.available) description = "Save the project first";
  else if (status === "analyzing") description = "Analyzing speech…";
  else if (failed) description = "Analysis failed";
  else if (speakerCount > 0) description = `${pluralize(speakerCount, "speaker")} found`;
  else description = "No speakers identified yet";

  return (
    <CleanupCard icon={Users} title="Detect speakers" description={description}>
      <button
        type="button"
        aria-label={status === "analyzing" ? "Analyzing speech" : failed ? "Retry speech analysis" : "Analyze speech"}
        disabled={!modelReady || !registry.available || !analysisItem || status === "analyzing"}
        onClick={() => {
          if (analysisItem) void analyze(analysisItem);
        }}
        className={cardButtonClass}
      >
        {status === "analyzing" ? "Analyzing…" : failed ? "Retry" : "Analyze"}
      </button>
      <button type="button" aria-label="Rename speakers" disabled={speakerCount === 0} onClick={() => setRenameOpen(true)} className={cardButtonClass}>
        Rename
      </button>
      <SpeakersDialog open={renameOpen} speakers={registry.speakers} onOpenChange={setRenameOpen} rename={registry.rename} />
    </CleanupCard>
  );
}

function MissingModelNotice({ onOpenModelSettings }: { onOpenModelSettings(): void }) {
  return (
    <div className="flex items-center gap-3 rounded-control border border-line p-2.5">
      <div className="min-w-0 flex-1">
        <p className="text-[12px] font-medium text-warning">Install speech models</p>
        <p className="truncate text-[11px] text-dim">Needed to detect speakers</p>
      </div>
      <button type="button" onClick={onOpenModelSettings} className={cn(cardButtonClass, "shrink-0 bg-panel")}>
        Open model settings
      </button>
    </div>
  );
}

/** "Clean up speech": a target (selected clip or a whole-project source) and the cleanup cards. */
export function CleanupCards() {
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  // Speaker detection runs the production speech models (VAD and diarization); silence ranges and
  // noise reduction need no model, and transcription belongs to the Captions tab.
  const { speechModelsReady, openModelSettings } = useEditorEnvironment();
  const labelId = useId();
  // A choice applies to the selection it was made under, so selecting a clip targets it again.
  const selectionKey = selectedItemIds.join("\n");
  const [choice, setChoice] = useState<{ readonly value: string; readonly selectionKey: string } | null>(null);
  const options = useMemo(() => cleanupTargetOptions(project, selectedItemIds), [project, selectedItemIds]);
  const chosen = choice?.selectionKey === selectionKey && options.some((option) => option.value === choice.value) ? choice.value : null;
  const value = chosen ?? options[0]?.value ?? null;
  const target = useMemo(() => resolveCleanupTarget(project, value), [project, value]);
  const audioItems = target ? targetAudioItems(project, target) : [];
  const analysisItem = target ? speechAnalysisItem(project, target) : null;

  return (
    <section aria-labelledby={labelId} className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <h2 id={labelId} className="shrink-0 text-[12px] text-muted-foreground">
          Clean up speech
        </h2>
        <Select value={value ?? ""} disabled={options.length === 0} onValueChange={(next) => setChoice({ value: next, selectionKey })}>
          <SelectTrigger aria-label="Clean up speech target" className="w-auto max-w-[60%] bg-transparent">
            <SelectValue placeholder="No speech source" />
          </SelectTrigger>
          <SelectContent>
            {options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>
      {!speechModelsReady && <MissingModelNotice onOpenModelSettings={openModelSettings} />}
      <ul aria-label="Speech cleanup" className="flex flex-col gap-2">
        <RemoveSilencesCard target={target} />
        <ReduceNoiseCard audioItems={audioItems} />
        <DetectSpeakersCard analysisItem={analysisItem} modelReady={speechModelsReady} />
      </ul>
    </section>
  );
}
