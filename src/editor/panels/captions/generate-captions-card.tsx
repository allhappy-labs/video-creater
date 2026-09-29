import { Captions, Loader2 } from "lucide-react";
import { useId, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { SelectField } from "../../properties/controls/select-field";
import { useEditorEnvironment } from "../../services/editor-environment";
import { CaptionBuildFields } from "./caption-build-fields";
import { captionLanguageOptions, type CaptionBuildSettings, type TranscriptionState } from "./captions-model";

interface GenerateCaptionsCardProps {
  readonly sourceField: ReactNode;
  readonly hasSource: boolean;
  readonly settings: CaptionBuildSettings;
  readonly state: TranscriptionState;
  readonly busy: boolean;
  readonly error: string | null;
  onSettingsChange(settings: CaptionBuildSettings): void;
  onGenerate(): void;
}

/** Empty state for a source without a transcript: options, then transcription followed by the build. */
export function GenerateCaptionsCard({ sourceField, hasSource, settings, state, busy, error, onSettingsChange, onGenerate }: GenerateCaptionsCardProps) {
  const { transcriptionModelReady, openModelSettings } = useEditorEnvironment();
  const titleId = useId();
  const transcribing = state === "transcribing";
  const locked = transcribing || busy;

  return (
    <section aria-labelledby={titleId} className="flex flex-col gap-3 rounded-panel bg-raised p-3">
      <div className="flex items-start gap-2.5">
        <span aria-hidden className="grid h-8 w-8 shrink-0 place-items-center rounded-md bg-panel text-primary">
          <Captions className="h-4 w-4" />
        </span>
        <div className="min-w-0">
          <h2 id={titleId} className="text-[13px] font-semibold text-foreground">
            Generate captions
          </h2>
          <p className="text-[11px] text-dim">Transcribes the source on this device, then places timed captions.</p>
        </div>
      </div>

      {sourceField}
      <SelectField
        label="Language"
        value={settings.language}
        options={captionLanguageOptions}
        disabled={locked}
        onChange={(language) => onSettingsChange({ ...settings, language })}
      />
      <CaptionBuildFields settings={settings} disabled={locked} onChange={onSettingsChange} />

      {!transcriptionModelReady && (
        <div className="flex items-center gap-3 rounded-control border border-line p-2.5">
          <div className="min-w-0 flex-1">
            <p className="text-[12px] font-medium text-warning">Install a transcription model</p>
            <p className="text-[11px] text-dim">Captions need a transcription model on this device.</p>
          </div>
          <Button type="button" size="sm" variant="ghost" className="shrink-0" onClick={openModelSettings}>
            Open model settings
          </Button>
        </div>
      )}

      {state === "failed" && !locked && (
        <p role="alert" className="text-[12px] text-destructive">
          Transcription failed. Try again.
        </p>
      )}
      {error && (
        <p role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}

      <Button type="button" size="sm" disabled={!hasSource || !transcriptionModelReady || locked} onClick={onGenerate}>
        {transcribing ? <Loader2 className="h-4 w-4 motion-safe:animate-spin" aria-hidden /> : null}
        {transcribing ? "Transcribing…" : "Generate captions"}
      </Button>
      {transcribing && (
        <p role="status" className="text-center text-[11px] text-dim">
          Captions are added when the transcript is ready.
        </p>
      )}
    </section>
  );
}
