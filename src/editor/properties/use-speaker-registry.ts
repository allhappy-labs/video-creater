import { useCallback, useEffect, useState } from "react";
import { getProjectSpeakerRegistry, type ProjectSpeakerIdentity } from "@/lib/project";
import { isBackendUnavailableError } from "@/lib/runtime/backend-transport";
import { useSpeechService } from "../services/speech-service";
import { useEditorStore } from "../store/editor-store-context";

interface SpeakerRegistry {
  /** Speakers live in the split project folder: schema 2+ with a project directory. */
  readonly available: boolean;
  readonly speakers: readonly ProjectSpeakerIdentity[];
  /** Renames with the trimmed name; blank or unchanged names are ignored. */
  rename(speakerId: string, name: string): Promise<void>;
  /** Loads the registry again, e.g. after speech analysis identified new speakers. */
  reload(): void;
}

function speakersOf(registry: unknown): ProjectSpeakerIdentity[] {
  const speakers = (registry as { speakers?: unknown } | undefined)?.speakers;
  return Array.isArray(speakers) ? (speakers as ProjectSpeakerIdentity[]) : [];
}

/** Pre-cut speaker registry wiring: load when the project is split, rename through the speech service. */
export function useSpeakerRegistry(): SpeakerRegistry {
  const projectDir = useEditorStore((state) => state.projectDir);
  const schemaVersion = useEditorStore((state) => state.project.schemaVersion);
  const speech = useSpeechService();
  const available = schemaVersion >= 2 && projectDir.trim().length > 0;
  const [speakers, setSpeakers] = useState<readonly ProjectSpeakerIdentity[]>([]);
  const [loadCount, setLoadCount] = useState(0);

  useEffect(() => {
    if (!available) return;
    let cancelled = false;
    Promise.resolve()
      .then(() => getProjectSpeakerRegistry({ projectDir }))
      .then(
        (registry) => {
          if (!cancelled) setSpeakers(speakersOf(registry));
        },
        (error: unknown) => {
          // Legacy: a missing backend is silent; other load failures are logged, not surfaced.
          if (!isBackendUnavailableError(error)) console.warn("Speaker registry unavailable", error);
        },
      );
    return () => {
      cancelled = true;
    };
  }, [available, projectDir, loadCount]);

  const rename = useCallback(
    async (speakerId: string, name: string) => {
      const current = speakers.find((speaker) => speaker.id === speakerId);
      if (!available || !current || current.name === name.trim()) return;
      const next = await speech.renameSpeaker(speakerId, name);
      if (next) setSpeakers(next);
    },
    [available, speakers, speech],
  );

  const reload = useCallback(() => setLoadCount((count) => count + 1), []);

  return { available, speakers: available ? speakers : [], rename, reload };
}
