import { cn } from "@/lib/utils";

export interface FilterChipOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

interface FilterChipsProps<T extends string> {
  readonly label: string;
  readonly value: T;
  readonly options: readonly FilterChipOption<T>[];
  readonly size?: "md" | "sm";
  onChange(value: T): void;
}

/** A wrapping row of single-choice chips (`aria-pressed` toggle buttons). */
export function FilterChips<T extends string>({ label, value, options, size = "md", onChange }: FilterChipsProps<T>) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap gap-1.5">
      {options.map((option) => {
        const pressed = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            aria-pressed={pressed}
            onClick={() => onChange(option.value)}
            className={cn(
              "rounded-full font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none",
              size === "md" ? "h-7 px-3 text-[12px]" : "h-6 px-2.5 text-[11px]",
              pressed ? "bg-accent-soft text-foreground" : "bg-raised text-muted-foreground hover:bg-hover hover:text-foreground",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
