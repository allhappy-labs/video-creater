import { useId } from "react";
import { cn } from "@/lib/utils";
import { useRovingGrid } from "./use-roving-grid";

interface ColorSwatchOption {
  /** CSS color stored in the project, for example "#ffd84a". */
  readonly value: string;
  readonly label: string;
}

interface ColorSwatchesProps {
  readonly label: string;
  readonly value: string | null;
  readonly options: readonly ColorSwatchOption[];
  readonly disabled?: boolean;
  onChange(value: string): void;
}

function sameColor(left: string | null, right: string): boolean {
  return left !== null && left.trim().toLowerCase() === right.trim().toLowerCase();
}

/** A labelled row of round color toggles (`aria-pressed`) with arrow-key navigation. */
export function ColorSwatches({ label, value, options, disabled = false, onChange }: ColorSwatchesProps) {
  const labelId = useId();
  const selectedIndex = options.findIndex((option) => sameColor(value, option.value));
  const itemProps = useRovingGrid(options.length, selectedIndex, options.length);

  return (
    <div className="grid grid-cols-[76px_minmax(0,1fr)] items-center gap-x-2">
      <span id={labelId} className="truncate text-[12px] text-muted-foreground">
        {label}
      </span>
      <div role="group" aria-labelledby={labelId} className="flex flex-wrap items-center gap-2">
        {options.map((option, index) => {
          const pressed = index === selectedIndex;
          return (
            <button
              key={option.value}
              type="button"
              aria-label={option.label}
              aria-pressed={pressed}
              disabled={disabled}
              onClick={() => {
                if (!pressed) onChange(option.value);
              }}
              // The swatch shows a project color value, not a UI color.
              style={{ backgroundColor: option.value }}
              className={cn(
                "h-6 w-6 rounded-full border border-line ring-offset-2 ring-offset-panel transition-shadow focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40",
                pressed && "ring-2 ring-foreground",
              )}
              {...itemProps(index)}
            />
          );
        })}
      </div>
    </div>
  );
}
