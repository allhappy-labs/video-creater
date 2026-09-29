import { useState, type KeyboardEvent, type ReactNode } from "react";

export interface SettingsDiagnosticsProps {
  label: string;
  children: ReactNode;
}

export function SettingsDiagnostics({
  label,
  children,
}: SettingsDiagnosticsProps) {
  const [open, setOpen] = useState(false);

  function handleSummaryKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    setOpen((current) => !current);
  }

  return (
    <details
      role="group"
      aria-label={label}
      open={open}
      onToggle={(event) => setOpen(event.currentTarget.open)}
      className="group border-t pt-2 text-xs"
    >
      <summary
        aria-expanded={open}
        onKeyDown={handleSummaryKeyDown}
        className="cursor-pointer select-none font-medium text-muted-foreground outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        {label}
      </summary>
      <div className="mt-2 grid gap-2 text-muted-foreground">{children}</div>
    </details>
  );
}
