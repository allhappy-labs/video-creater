import type { LucideIcon } from "lucide-react";
import { useId } from "react";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { cn } from "@/lib/utils";

interface SegmentedOption<T extends string> {
  readonly value: T;
  readonly label: string;
  /** Icon-only segment; the label stays the accessible name. */
  readonly icon?: LucideIcon;
}

interface SegmentedFieldProps<T extends string> {
  readonly label: string;
  readonly value: T;
  readonly options: readonly SegmentedOption<T>[];
  readonly disabled?: boolean;
  /** "row" puts the label beside the control; "heading" makes it a bold title, as in "Applies to". */
  readonly labelStyle?: "row" | "heading";
  onChange(value: T): void;
}

/** A labelled single-choice segmented control; pressing the active segment never clears it. */
export function SegmentedField<T extends string>({ label, value, options, disabled = false, labelStyle = "row", onChange }: SegmentedFieldProps<T>) {
  const labelId = useId();
  return (
    <div className={cn("grid items-center gap-x-2", labelStyle === "row" ? "grid-cols-[76px_minmax(0,1fr)]" : "grid-cols-[auto_auto] justify-between")}>
      <span id={labelId} className={cn("truncate", labelStyle === "row" ? "text-[12px] text-muted-foreground" : "text-[13px] font-semibold text-foreground")}>
        {label}
      </span>
      <ToggleGroup
        type="single"
        aria-labelledby={labelId}
        value={value}
        disabled={disabled}
        onValueChange={(next) => {
          const option = options.find((candidate) => candidate.value === next);
          if (option && option.value !== value) onChange(option.value);
        }}
      >
        {options.map((option) => (
          <ToggleGroupItem key={option.value} value={option.value} aria-label={option.icon ? option.label : undefined}>
            {option.icon ? <option.icon className="h-4 w-4" aria-hidden /> : option.label}
          </ToggleGroupItem>
        ))}
      </ToggleGroup>
    </div>
  );
}
