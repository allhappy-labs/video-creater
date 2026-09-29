import { useId } from "react";
import { Switch } from "@/components/ui/switch";

interface SwitchFieldProps {
  readonly label: string;
  readonly checked: boolean;
  /** Secondary line under the label, for example a preparation status. */
  readonly description?: string;
  readonly disabled?: boolean;
  onChange(checked: boolean): void;
}

/** A labelled switch row: label (and optional description) on the left, switch on the right. */
export function SwitchField({ label, checked, description, disabled = false, onChange }: SwitchFieldProps) {
  const switchId = useId();
  const descriptionId = useId();
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="min-w-0">
        <label htmlFor={switchId} className="block truncate text-[12px] text-muted-foreground">
          {label}
        </label>
        {description && (
          <p id={descriptionId} className="truncate text-[11px] text-dim">
            {description}
          </p>
        )}
      </div>
      <Switch
        id={switchId}
        checked={checked}
        disabled={disabled}
        aria-describedby={description ? descriptionId : undefined}
        onCheckedChange={onChange}
      />
    </div>
  );
}
