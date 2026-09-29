import { ChevronDown, Folder } from "lucide-react";
import { useId, useState } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip } from "@/components/ui/tooltip";
import { exportFileLabel, exportFileNameProblem } from "@/lib/export/export-naming";
import type { ExportChoiceOption, ExportChoiceOptionSet, ExportChoices, ExportFrameRateOption, ExportStartPlan } from "@/lib/export/export-plan";
import { formatEstimatedSize } from "@/lib/export/profiles";
import { cn } from "@/lib/utils";

const timelineFrameRateValue = "timeline";

interface ChoiceFieldProps<T extends string> {
  readonly label: string;
  readonly value: T;
  readonly options: readonly ExportChoiceOption<T>[];
  onChange(value: T): void;
}

/** A labelled segmented control; unavailable segments stay focusable, explain why, and can't be chosen. */
function ChoiceField<T extends string>({ label, value, options, onChange }: ChoiceFieldProps<T>) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <span id={`${id}-label`} className="text-[12px] text-muted-foreground">
        {label}
      </span>
      <ToggleGroup
        type="single"
        aria-labelledby={`${id}-label`}
        value={value}
        className="flex w-full"
        onValueChange={(next) => {
          const option = options.find((candidate) => candidate.value === next);
          if (option && option.value !== value && option.disabledReason === null) onChange(option.value);
        }}
      >
        {options.map((option) => {
          const reasonId = `${id}-${option.value}-reason`;
          const item = (
            <ToggleGroupItem
              key={option.value}
              value={option.value}
              aria-disabled={option.disabledReason !== null || undefined}
              aria-describedby={option.disabledReason ? reasonId : undefined}
              className="aria-disabled:cursor-not-allowed aria-disabled:opacity-40 aria-disabled:hover:text-muted-foreground"
            >
              {option.label}
            </ToggleGroupItem>
          );
          if (option.disabledReason === null) return item;
          return (
            <Tooltip key={option.value} content={option.disabledReason}>
              {/* The span carries the tooltip so it never overrides the segment's pressed state. */}
              <span className="flex min-w-0 flex-1">
                {item}
                <span id={reasonId} className="sr-only">
                  {option.disabledReason}
                </span>
              </span>
            </Tooltip>
          );
        })}
      </ToggleGroup>
    </div>
  );
}

/** The Advanced frame-rate select; the timeline rate is the empty choice. */
function FrameRateField({ value, options, draft, onChange }: { readonly value: number | null; readonly options: readonly ExportFrameRateOption[]; readonly draft: boolean; onChange(fps: number | null): void }) {
  const id = useId();
  return (
    <div className="flex flex-col gap-1.5">
      <span id={`${id}-label`} className="text-[12px] text-muted-foreground">
        Frame rate
      </span>
      <Select
        value={value === null ? timelineFrameRateValue : String(value)}
        onValueChange={(next) => onChange(next === timelineFrameRateValue ? null : Number(next))}
      >
        <SelectTrigger aria-labelledby={`${id}-label`} className="h-8 rounded-control text-[13px]">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.label} value={option.value === null ? timelineFrameRateValue : String(option.value)}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {draft && <span className="text-[11px] text-dim">Draft renders at up to 24 fps.</span>}
    </div>
  );
}

/** The Save to folder chooser; `disabledReason` explains why it can't be used here. */
interface ExportFolderChooser {
  readonly disabledReason: string | null;
  choose(): void;
}

interface ExportOptionsProps {
  readonly choices: ExportChoices;
  readonly options: ExportChoiceOptionSet;
  readonly plan: ExportStartPlan;
  readonly sizeBytes: number;
  /** The folder exports save into by default: the project's `exports` directory. */
  readonly projectExportsFolder: string;
  readonly folderChooser: ExportFolderChooser;
  readonly frameRateOptions: readonly ExportFrameRateOption[];
  onChange(patch: Partial<ExportChoices>): void;
}

export function ExportOptions({ choices, options, plan, sizeBytes, projectExportsFolder, folderChooser, frameRateOptions, onChange }: ExportOptionsProps) {
  const id = useId();
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const saveLocation = choices.directory ?? projectExportsFolder;
  const nameProblem = exportFileNameProblem(choices.name);
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-col gap-1.5">
        <label htmlFor={`${id}-name`} className="text-[12px] text-muted-foreground">
          Name
        </label>
        <input
          id={`${id}-name`}
          value={choices.name}
          aria-describedby={`${id}-name-hint`}
          onChange={(event) => onChange({ name: event.target.value })}
          className="h-8 rounded-control border border-line bg-raised px-2.5 text-[13px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <span id={`${id}-name-hint`} className={cn("truncate text-[11px]", nameProblem ? "text-warning" : "text-dim")}>
          {nameProblem ?? `Saves as ${exportFileLabel(choices.name, plan.extension)}`}
        </span>
      </div>
      <div role="group" aria-labelledby={`${id}-save-to`} className="flex flex-col gap-1.5">
        <span id={`${id}-save-to`} className="text-[12px] text-muted-foreground">
          Save to
        </span>
        <div className="flex h-8 items-center gap-1 rounded-control bg-raised pl-2.5">
          <span className="min-w-0 flex-1 truncate text-[13px] text-foreground" title={saveLocation}>
            {saveLocation}
          </span>
          <IconButton
            label="Choose export folder"
            tooltip={folderChooser.disabledReason ?? undefined}
            aria-disabled={folderChooser.disabledReason !== null || undefined}
            aria-describedby={folderChooser.disabledReason ? `${id}-folder-reason` : undefined}
            onClick={() => folderChooser.disabledReason === null && folderChooser.choose()}
            className="aria-disabled:cursor-not-allowed aria-disabled:opacity-40"
          >
            <Folder className="h-4 w-4" aria-hidden />
          </IconButton>
          {folderChooser.disabledReason && (
            <span id={`${id}-folder-reason`} className="sr-only">
              {folderChooser.disabledReason}
            </span>
          )}
        </div>
      </div>
      <ChoiceField label="Format" value={choices.format} options={options.format} onChange={(format) => onChange({ format })} />
      <ChoiceField label="Resolution" value={choices.resolution} options={options.resolution} onChange={(resolution) => onChange({ resolution })} />
      <ChoiceField label="Quality" value={choices.quality} options={options.quality} onChange={(quality) => onChange({ quality })} />
      <div className="flex items-center gap-2">
        <p className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground">
          {`${plan.codecLabel} · ${plan.fps} fps · ≈ ${formatEstimatedSize(sizeBytes)}`}
        </p>
        <button
          type="button"
          aria-expanded={advancedOpen}
          aria-controls={`${id}-advanced`}
          onClick={() => setAdvancedOpen((open) => !open)}
          className="flex h-7 items-center gap-1 rounded-control px-1.5 text-[12px] text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Advanced
          <ChevronDown className={cn("h-3.5 w-3.5 transition-transform motion-reduce:transition-none", advancedOpen && "rotate-180")} aria-hidden />
        </button>
      </div>
      {advancedOpen && (
        <div id={`${id}-advanced`} className="flex flex-col gap-3 rounded-control border border-line p-2.5">
          <ChoiceField label="Codec" value={choices.codec} options={options.codec} onChange={(codec) => onChange({ codec })} />
          <FrameRateField value={choices.fps ?? null} options={frameRateOptions} draft={plan.quality === "draft"} onChange={(fps) => onChange({ fps })} />
        </div>
      )}
    </div>
  );
}
