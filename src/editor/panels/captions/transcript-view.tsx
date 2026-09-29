import { Sparkles } from "lucide-react";
import { Fragment, useEffect, useId, useMemo, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Tooltip } from "@/components/ui/tooltip";
import { currentWordIndex, lowConfidenceWordCount, transcriptParagraphs, type TranscriptParagraph } from "@/lib/captions/transcript-view-model";
import { formatTimelinePlacementTime, pluralize } from "@/lib/format";
import type { ProjectSpeakerIdentity, Transcript } from "@/lib/project";
import { cn } from "@/lib/utils";
import { useSpeakerRegistry } from "../../properties/use-speaker-registry";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { sourceSecondsAtPlayhead, timelineSecondsForSource, transcriptCaptionItems, type CaptionBuildSettings } from "./captions-model";
import { RegenerateCaptionsDialog } from "./regenerate-captions-dialog";
import { transcriptWordFixActions } from "./transcript-word-fix";
import type { CaptionGeneration } from "./use-caption-generation";
import { WordEditor } from "./word-editor";

type TranscriptSpan = TranscriptParagraph["spans"][number];
type WordSpan = Extract<TranscriptSpan, { isPause: false }>;

interface TranscriptViewProps {
  readonly mediaId: string;
  readonly transcript: Transcript;
  readonly header: ReactNode;
  readonly settings: CaptionBuildSettings;
  readonly generation: CaptionGeneration;
  onSettingsChange(settings: CaptionBuildSettings): void;
}

const lowConfidenceHint = "Low confidence — double-click to fix";

function speakerLabel(paragraph: TranscriptParagraph, speakers: readonly ProjectSpeakerIdentity[]): string | null {
  if (paragraph.speakerId === null) return null;
  const named = speakers.find((speaker) => speaker.id === paragraph.speakerId)?.name.trim();
  return named || `Speaker ${(paragraph.speakerNumber ?? 1).toString()}`;
}

function formatPause(seconds: number): string {
  return `${Number(seconds.toFixed(1)).toString()}s`;
}

/**
 * Speaker paragraphs of word buttons (one tab stop, arrow keys move between words). Click seeks,
 * double-click or F2 fixes a word. Word buttons carry `data-word-index` for VC-006 range cutting.
 */
export function TranscriptView({ mediaId, transcript, header, settings, generation, onSettingsChange }: TranscriptViewProps) {
  const store = useEditorStoreApi();
  const project = useEditorStore((state) => state.project);
  const transcriptRange = useEditorStore((state) => state.transcriptRange);
  const { speakers } = useSpeakerRegistry();
  const paragraphs = useMemo(() => transcriptParagraphs(project, mediaId), [project, mediaId]);
  const currentIndex = useEditorStore((state) => {
    const sourceSeconds = sourceSecondsAtPlayhead(state.project, mediaId, state.playheadSeconds);
    return sourceSeconds === null ? null : currentWordIndex(paragraphs, sourceSeconds);
  });
  const wordIndices = useMemo(() => paragraphs.flatMap((paragraph) => paragraph.spans.flatMap((span) => (span.isPause ? [] : [span.index]))), [paragraphs]);
  const [focusIndex, setFocusIndex] = useState<number | null>(null);
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const restoreFocusIndex = useRef<number | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const hintId = useId();
  const captionItems = transcriptCaptionItems(project, transcript.id);
  const checkCount = lowConfidenceWordCount(paragraphs);
  const tabStop = focusIndex !== null && wordIndices.includes(focusIndex) ? focusIndex : (currentIndex ?? wordIndices[0] ?? null);

  const focusWord = (index: number) => {
    listRef.current?.querySelector<HTMLButtonElement>(`button[data-word-index="${index.toString()}"]`)?.focus();
  };

  useEffect(() => {
    if (editingIndex !== null || restoreFocusIndex.current === null) return;
    focusWord(restoreFocusIndex.current);
    restoreFocusIndex.current = null;
  });

  const closeEditor = (index: number, restoreFocus: boolean) => {
    if (restoreFocus) restoreFocusIndex.current = index;
    setFocusIndex(index);
    setEditingIndex(null);
  };

  const seekTo = (span: WordSpan) => {
    const state = store.getState();
    state.seek(timelineSecondsForSource(state.project, mediaId, span.startSeconds));
  };

  const commitFix = async (index: number, text: string): Promise<string | null> => {
    const state = store.getState();
    const result = transcriptWordFixActions(state.project, { transcriptId: transcript.id, wordIndex: index, text });
    if ("blocked" in result) return result.blocked;
    if (result.actions.length > 0 && !(await state.applyActions(result.actions))) {
      return store.getState().lastError ?? "The word could not be fixed.";
    }
    closeEditor(index, true);
    return null;
  };

  const onWordKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const position = wordIndices.indexOf(index);
    let next: number | undefined;
    switch (event.key) {
      case "ArrowRight":
      case "ArrowDown":
        next = wordIndices[position + 1];
        break;
      case "ArrowLeft":
      case "ArrowUp":
        next = wordIndices[position - 1];
        break;
      case "Home":
        next = wordIndices[0];
        break;
      case "End":
        next = wordIndices[wordIndices.length - 1];
        break;
      case "F2":
        event.preventDefault();
        setEditingIndex(index);
        return;
      default:
        return;
    }
    event.preventDefault();
    if (next === undefined) return;
    setFocusIndex(next);
    focusWord(next);
  };

  const renderWord = (span: WordSpan) => {
    if (editingIndex === span.index) {
      return (
        <WordEditor
          word={span.text}
          onCommit={(text) => commitFix(span.index, text)}
          onCancel={(restoreFocus) => closeEditor(span.index, restoreFocus)}
        />
      );
    }
    const inRange =
      transcriptRange?.transcriptId === transcript.id && span.index >= transcriptRange.startWordIndex && span.index <= transcriptRange.endWordIndex;
    const word = (
      <button
        type="button"
        data-word-index={span.index}
        data-in-range={inRange ? "true" : undefined}
        tabIndex={tabStop === span.index ? 0 : -1}
        aria-current={span.index === currentIndex ? "true" : undefined}
        aria-describedby={span.lowConfidence ? hintId : undefined}
        aria-keyshortcuts="F2"
        onClick={() => seekTo(span)}
        onDoubleClick={() => setEditingIndex(span.index)}
        onFocus={() => setFocusIndex(span.index)}
        onKeyDown={(event) => onWordKeyDown(event, span.index)}
        className={cn(
          "rounded-sm px-0.5 text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
          span.lowConfidence && "underline decoration-warning decoration-wavy underline-offset-4",
          inRange && "bg-raised",
          span.index === currentIndex && "bg-accent-soft text-primary hover:bg-accent-soft",
        )}
      >
        {span.text}
      </button>
    );
    return span.lowConfidence ? (
      <Tooltip content={lowConfidenceHint}>
        {word}
      </Tooltip>
    ) : (
      word
    );
  };

  const regenerate = () => {
    if (captionItems.length > 0) setConfirmOpen(true);
    else void generation.build(settings);
  };

  return (
    <div className="flex flex-1 flex-col">
      <div className="flex flex-col gap-3 px-3 pt-3">{header}</div>
      <p id={hintId} hidden>
        {lowConfidenceHint}
      </p>
      <div ref={listRef} role="group" aria-label="Transcript" className="flex flex-col gap-4 px-3 py-3">
        {paragraphs.length === 0 && <p className="text-[12px] text-dim">This transcript has no words.</p>}
        {paragraphs.map((paragraph) => {
          const label = speakerLabel(paragraph, speakers);
          return (
            <div key={paragraph.id} className="flex flex-col gap-1">
              <p className="flex items-center gap-1.5 text-[11px] text-dim">
                <span aria-hidden className="h-1.5 w-1.5 rounded-full bg-primary" />
                {label ? `${label} · ` : ""}
                {formatTimelinePlacementTime(paragraph.startSeconds)}
              </p>
              <p className="text-[13px] leading-7">
                {paragraph.spans.map((span) =>
                  span.isPause ? (
                    <span
                      key={`pause-${span.index.toString()}`}
                      role="img"
                      aria-label={`Pause ${formatPause(span.durationSeconds)}`}
                      className="mx-1 inline-flex h-5 items-center rounded-full border border-line px-1.5 align-middle text-[10px] text-dim"
                    >
                      {formatPause(span.durationSeconds)}
                    </span>
                  ) : (
                    <Fragment key={`word-${span.index.toString()}`}>{renderWord(span)} </Fragment>
                  ),
                )}
              </p>
            </div>
          );
        })}
      </div>
      {generation.error && !confirmOpen && (
        <p role="alert" className="px-3 pb-2 text-[12px] text-destructive">
          {generation.error}
        </p>
      )}
      <div className="sticky bottom-0 mt-auto flex items-center gap-2 border-t border-line bg-panel px-3 py-2">
        <p className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">
          {pluralize(captionItems.length, "caption")}
          {checkCount > 0 ? ` · ${pluralize(checkCount, "word")} to check` : ""}
        </p>
        <Button type="button" size="sm" variant="ghost" className="bg-raised" disabled={generation.busy} onClick={regenerate}>
          <Sparkles className="h-4 w-4" aria-hidden />
          {captionItems.length > 0 ? "Regenerate" : "Generate captions"}
        </Button>
      </div>
      <RegenerateCaptionsDialog
        open={confirmOpen}
        captionCount={captionItems.length}
        settings={settings}
        error={generation.error}
        onSettingsChange={onSettingsChange}
        onOpenChange={setConfirmOpen}
        onConfirm={() => generation.build(settings, captionItems.map((item) => item.id))}
      />
    </div>
  );
}
