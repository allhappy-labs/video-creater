import type { ReactNode } from "react";
import { cn } from "@/lib/utils";
import { useRovingGrid } from "./use-roving-grid";

interface PresetOption<T extends string> {
  readonly value: T;
  readonly label: string;
  /** Visual sample drawn in the tile; the label stays the accessible name. */
  readonly preview?: ReactNode;
}

interface PresetGridProps<T extends string> {
  readonly label: string;
  readonly value: T | null;
  readonly options: readonly PresetOption<T>[];
  readonly columns?: 2 | 3 | 4;
  readonly disabled?: boolean;
  onChange(value: T): void;
}

const columnClass = { 2: "grid-cols-2", 3: "grid-cols-3", 4: "grid-cols-4" } as const;

/** Tiles of toggle buttons (`aria-pressed`) with arrow-key navigation between them. */
export function PresetGrid<T extends string>({ label, value, options, columns = 3, disabled = false, onChange }: PresetGridProps<T>) {
  const selectedIndex = options.findIndex((option) => option.value === value);
  const itemProps = useRovingGrid(options.length, selectedIndex, columns);

  return (
    <div role="group" aria-label={label} className={cn("grid gap-1.5", columnClass[columns])}>
      {options.map((option, index) => {
        const pressed = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-label={option.preview ? option.label : undefined}
            aria-pressed={pressed}
            disabled={disabled}
            onClick={() => {
              if (!pressed) onChange(option.value);
            }}
            className={cn(
              "flex min-h-[52px] min-w-0 flex-col items-center justify-center gap-1 rounded-control bg-raised px-2 py-1.5 text-[12px] text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-40",
              pressed && "bg-accent-soft ring-2 ring-primary hover:bg-accent-soft",
            )}
            {...itemProps(index)}
          >
            {option.preview ?? <span className="truncate">{option.label}</span>}
          </button>
        );
      })}
    </div>
  );
}
