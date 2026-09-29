import { useId } from "react";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";

interface SelectOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

interface SelectFieldProps<T extends string> {
  readonly label: string;
  readonly value: T | null;
  readonly options: readonly SelectOption<T>[];
  readonly placeholder?: string;
  readonly disabled?: boolean;
  onChange(value: T): void;
}

/** A labelled Radix select row; choosing the current value does not call `onChange`. */
export function SelectField<T extends string>({ label, value, options, placeholder = "Choose", disabled = false, onChange }: SelectFieldProps<T>) {
  const labelId = useId();
  return (
    <div className="grid grid-cols-[76px_minmax(0,1fr)] items-center gap-x-2">
      <span id={labelId} className="truncate text-[12px] text-muted-foreground">
        {label}
      </span>
      <Select
        // Radix shows the placeholder for an empty value.
        value={value ?? ""}
        disabled={disabled}
        onValueChange={(next) => {
          const option = options.find((candidate) => candidate.value === next);
          if (option && option.value !== value) onChange(option.value);
        }}
      >
        <SelectTrigger aria-labelledby={labelId}>
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}
