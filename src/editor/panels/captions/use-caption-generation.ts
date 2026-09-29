import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { useSpeechService } from "../../services/speech-service";
import type { EditorStore } from "../../store/editor-store";
import { useEditorStore, useEditorStoreApi } from "../../store/editor-store-context";
import { captionBuildRange, transcriptForMedia, transcriptionState, type CaptionBuildSettings, type TranscriptionState } from "./captions-model";

/**
 * "Generate captions" clicked before the transcript existed, per source media. It lives beside the
 * store rather than in the panel, so switching tabs while Temporal transcribes keeps the intent.
 */
interface PendingBuilds {
  readonly intents: Map<string, CaptionBuildSettings>;
  readonly listeners: Set<() => void>;
  version: number;
}

const pendingBuildsByStore = new WeakMap<EditorStore, PendingBuilds>();

function pendingBuildsFor(store: EditorStore): PendingBuilds {
  let pending = pendingBuildsByStore.get(store);
  if (!pending) {
    pending = { intents: new Map(), listeners: new Set(), version: 0 };
    pendingBuildsByStore.set(store, pending);
  }
  return pending;
}

function notify(pending: PendingBuilds) {
  pending.version += 1;
  for (const listener of pending.listeners) listener();
}

export interface CaptionGeneration {
  /** The latest transcription job, reported as transcribing while a build waits for its transcript. */
  readonly state: TranscriptionState;
  readonly busy: boolean;
  readonly error: string | null;
  /** Builds captions from the transcript, or starts transcription and builds once the transcript loads. */
  generate(settings: CaptionBuildSettings): Promise<void>;
  /** Builds from the existing transcript, replacing `replaceItemIds` in the same batch. */
  build(settings: CaptionBuildSettings, replaceItemIds?: readonly string[]): Promise<boolean>;
}

export function useCaptionGeneration(mediaId: string | null): CaptionGeneration {
  const store = useEditorStoreApi();
  const speech = useSpeechService();
  const pending = pendingBuildsFor(store);
  const subscribe = useCallback(
    (listener: () => void) => {
      pending.listeners.add(listener);
      return () => pending.listeners.delete(listener);
    },
    [pending],
  );
  useSyncExternalStore(subscribe, () => pending.version);
  const project = useEditorStore((state) => state.project);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<{ readonly mediaId: string | null; readonly message: string } | null>(null);

  const build = useCallback(
    async (source: string, settings: CaptionBuildSettings, replaceItemIds: readonly string[] = []) => {
      const { project: current, selectedItemIds } = store.getState();
      const transcript = transcriptForMedia(current, source);
      const range = transcript ? captionBuildRange(current, source, transcript, selectedItemIds) : null;
      if (!transcript || !range) {
        const message = transcript?.words.length
          ? "Captions need a clip of this source that plays forward."
          : "This transcript has no words to caption.";
        setError({ mediaId: source, message });
        return false;
      }
      const built = await speech.buildCaptions({
        transcript,
        range,
        wordsPerCue: settings.maxWords,
        stylePreset: "boldReadableLower",
        censorProfanity: settings.censorProfanity,
        replaceItemIds,
      });
      if (!built) setError({ mediaId: source, message: store.getState().lastError ?? "Captions could not be built." });
      return built;
    },
    [speech, store],
  );

  const transcript = mediaId ? transcriptForMedia(project, mediaId) : null;
  const intent = mediaId ? pending.intents.get(mediaId) : undefined;
  const jobState = mediaId ? transcriptionState(project, mediaId) : "idle";

  useEffect(() => {
    if (!mediaId || !intent) return;
    if (transcript) {
      // Cleared before building, so a re-render during the build never builds twice.
      pending.intents.delete(mediaId);
      notify(pending);
      void build(mediaId, intent);
    } else if (jobState === "failed") {
      pending.intents.delete(mediaId);
      notify(pending);
    }
  }, [build, intent, jobState, mediaId, pending, transcript]);

  const generate = useCallback(
    async (settings: CaptionBuildSettings) => {
      if (!mediaId || busy) return;
      setBusy(true);
      setError(null);
      if (transcriptForMedia(store.getState().project, mediaId)) {
        await build(mediaId, settings);
      } else if (await speech.transcribe(mediaId, settings.language)) {
        pending.intents.set(mediaId, settings);
        notify(pending);
      } else {
        setError({ mediaId, message: store.getState().lastError ?? "Transcription could not start." });
      }
      setBusy(false);
    },
    [build, busy, mediaId, pending, speech, store],
  );

  const buildForSource = useCallback(
    (settings: CaptionBuildSettings, replaceItemIds?: readonly string[]) => (mediaId ? build(mediaId, settings, replaceItemIds) : Promise.resolve(false)),
    [build, mediaId],
  );

  return useMemo(
    () => ({
      state: intent && jobState !== "failed" ? "transcribing" : jobState,
      busy,
      error: error && error.mediaId === mediaId ? error.message : null,
      generate,
      build: buildForSource,
    }),
    [buildForSource, busy, error, generate, intent, jobState, mediaId],
  );
}
