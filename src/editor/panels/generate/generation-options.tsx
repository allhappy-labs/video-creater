import { useId, type ReactNode } from "react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { generationQualityLabel, generationResolutionLabel } from "@/lib/generation/settings-options";
import { cn } from "@/lib/utils";
import { generationModeFor, type ComposerFields, type ComposerState } from "./composer-model";

type ComposerPatch = Partial<ComposerState>;

interface Choice {
  readonly value: string;
  readonly label: string;
}

/** Short lists render as a row of chips that fit the panel width; the rest use a select. */
function fitsAsChips(choices: readonly Choice[]): boolean {
  const longest = Math.max(0, ...choices.map((choice) => choice.label.length));
  return (choices.length <= 3 && longest <= 10) || (choices.length === 4 && longest <= 5);
}

export const fieldLabelClass = "text-[12px] font-medium text-muted-foreground";
const inputClass =
  "h-8 w-full min-w-0 rounded-control bg-raised px-2.5 text-[12px] text-foreground placeholder:text-dim focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring";

/** A stacked field: label above its control, as in the Generate mockup. */
export function ComposerField({ label, labelId, children }: { readonly label: string; readonly labelId: string; readonly children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <span id={labelId} className={fieldLabelClass}>
        {label}
      </span>
      {children}
    </div>
  );
}

/** A single choice: chips for a few options, a select for many. */
function ChoiceField({ label, value, choices, onChange }: { readonly label: string; readonly value: string; readonly choices: readonly Choice[]; onChange(value: string): void }) {
  const labelId = useId();
  return (
    <ComposerField label={label} labelId={labelId}>
      {fitsAsChips(choices) ? (
        <div role="radiogroup" aria-labelledby={labelId} className={cn("grid gap-1.5", choices.length === 4 ? "grid-cols-4" : "grid-cols-3")}>
          {choices.map((choice) => {
            const checked = choice.value === value;
            return (
              <button
                key={choice.value}
                type="button"
                role="radio"
                aria-checked={checked}
                onClick={() => onChange(choice.value)}
                className={cn(
                  "h-8 min-w-0 truncate rounded-control px-1.5 text-[12px] transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
                  checked ? "bg-accent-soft font-medium text-foreground ring-1 ring-inset ring-primary" : "bg-raised text-muted-foreground hover:bg-hover hover:text-foreground",
                )}
              >
                {choice.label}
              </button>
            );
          })}
        </div>
      ) : (
        <Select value={value} onValueChange={onChange}>
          <SelectTrigger aria-labelledby={labelId} className="h-8">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {choices.map((choice) => (
              <SelectItem key={choice.value} value={choice.value}>
                {choice.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
    </ComposerField>
  );
}

function SwitchRow({ label, checked, onChange }: { readonly label: string; readonly checked: boolean; onChange(checked: boolean): void }) {
  const id = useId();
  return (
    <div className="flex items-center justify-between gap-3">
      <label htmlFor={id} className={fieldLabelClass}>
        {label}
      </label>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}

function TextAreaField({ label, value, placeholder, rows, onChange }: { readonly label: string; readonly value: string; readonly placeholder: string; readonly rows: number; onChange(value: string): void }) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className={fieldLabelClass}>
        {label}
      </label>
      <textarea id={id} rows={rows} value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} className={cn(inputClass, "h-auto resize-none py-2 leading-snug")} />
    </div>
  );
}

interface GenerationOptionsProps {
  readonly state: ComposerState;
  readonly fields: ComposerFields;
  onChange(patch: ComposerPatch): void;
}

/**
 * Model-specific options. Each renders only when the selected model offers choices for it: voice,
 * duration, aspect, resolution, quality, image count, and the audio, instrumental, lyrics and style
 * inputs.
 */
export function GenerationOptions({ state, fields, onChange }: GenerationOptionsProps) {
  const durationId = useId();
  const mode = generationModeFor(state.mode);
  const bounds = fields.durationBounds;
  return (
    <div className="flex flex-col gap-3">
      {fields.voices.length > 0 && <ChoiceField label="Voice" value={fields.voice} choices={fields.voices.map((voice) => ({ value: voice, label: voice }))} onChange={(voice) => onChange({ voice })} />}
      {fields.aspectRatio !== null && fields.aspectChoices.length > 0 && (
        <ChoiceField label="Aspect" value={fields.aspectRatio} choices={fields.aspectChoices.map((ratio) => ({ value: ratio, label: ratio === "auto" ? "Auto" : ratio }))} onChange={(aspectRatio) => onChange({ aspectRatio })} />
      )}
      {bounds ? (
        <ComposerField label="Duration" labelId={durationId}>
          <div className="flex items-center gap-2">
            <input
              type="number"
              aria-labelledby={durationId}
              min={bounds.minSeconds}
              max={bounds.maxSeconds}
              step={1}
              value={fields.duration}
              onChange={(event) => onChange({ duration: event.target.value })}
              className={cn(inputClass, "tabular-time w-24")}
            />
            <span className="text-[12px] text-dim">
              seconds ({bounds.minSeconds}–{bounds.maxSeconds})
            </span>
          </div>
        </ComposerField>
      ) : (
        fields.durationChoices.length > 0 && (
          <ChoiceField label="Duration" value={fields.duration} choices={fields.durationChoices.map((option) => ({ value: option.value, label: option.value }))} onChange={(duration) => onChange({ duration })} />
        )
      )}
      {fields.resolutionChoices.length > 0 && (
        <ChoiceField
          label="Resolution"
          value={fields.resolution}
          choices={fields.resolutionChoices.map((option) => ({ value: option.value, label: generationResolutionLabel(mode, option) }))}
          onChange={(resolution) => onChange({ resolution })}
        />
      )}
      {fields.quality !== null && fields.qualityChoices.length > 0 && (
        <ChoiceField label="Quality" value={fields.quality} choices={fields.qualityChoices.map((quality) => ({ value: quality, label: generationQualityLabel(quality) }))} onChange={(quality) => onChange({ quality })} />
      )}
      {fields.imageCountChoices.length > 0 && (
        <ChoiceField label="Variations" value={fields.imageCount} choices={fields.imageCountChoices.map((count) => ({ value: count, label: count }))} onChange={(imageCount) => onChange({ imageCount })} />
      )}
      {fields.showAudioToggle && <SwitchRow label="Generate audio" checked={state.generateAudio} onChange={(generateAudio) => onChange({ generateAudio })} />}
      {fields.showInstrumental && <SwitchRow label="Instrumental" checked={state.instrumental} onChange={(instrumental) => onChange({ instrumental })} />}
      {fields.showLyrics && <TextAreaField label="Lyrics" rows={3} value={state.lyrics} placeholder="[Verse]" onChange={(lyrics) => onChange({ lyrics })} />}
      {fields.showStyleInstructions && <TextAreaField label="Style" rows={2} value={state.styleInstructions} placeholder="Tone, delivery, instrumentation" onChange={(styleInstructions) => onChange({ styleInstructions })} />}
    </div>
  );
}
