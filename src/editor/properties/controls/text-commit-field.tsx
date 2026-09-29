import { useEffect, useId, useState, type KeyboardEvent } from "react";
import type { ProjectAction } from "@/lib/project";
import type { CommandResult } from "@/lib/timeline-ops/clip-commands";
import { cn } from "@/lib/utils";
import { usePropertyCommit } from "../use-property-commit";

interface TextCommitFieldProps {
  readonly label: string;
  /** Committed text from the project; a change replaces any draft. */
  readonly value: string;
  /** Multiline commits on Cmd/Ctrl+Enter or blur; single-line on Enter or blur. */
  readonly multiline?: boolean;
  readonly rows?: number;
  /** Marks the field required for assistive technology; `build` still owns the validation. */
  readonly required?: boolean;
  readonly placeholder?: string;
  /** Secondary line under the field, such as a reading warning. */
  readonly hint?: string | null;
  /** "stacked" puts the label above the field; "row" beside it, as in other property rows. */
  readonly layout?: "stacked" | "row";
  /** Builds the change for the draft; a blocked result is shown inline instead of committing. */
  build(text: string): CommandResult | readonly ProjectAction[];
}

const fieldClass =
  "w-full min-w-0 rounded-md bg-raised px-2 text-[12px] text-foreground outline-none placeholder:text-dim focus-visible:ring-2 focus-visible:ring-ring";

/** A labelled text input or textarea that commits one project change and shows errors inline. */
export function TextCommitField({ label, value, multiline = false, rows = 3, required = false, placeholder, hint, layout = "stacked", build }: TextCommitFieldProps) {
  const inputId = useId();
  const messageId = useId();
  const { commit } = usePropertyCommit();
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(null);
    setError(null);
  }, [value]);

  const submit = () => {
    if (draft === null || draft === value) {
      setError(null);
      return;
    }
    const result = build(draft);
    if ("blocked" in result) {
      setError(result.blocked);
      return;
    }
    setError(null);
    void commit(result);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    if (event.key === "Escape" && draft !== null) {
      event.preventDefault();
      event.stopPropagation();
      setDraft(null);
      setError(null);
    } else if (event.key === "Enter" && (!multiline || event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      submit();
    }
  };

  const message = error ?? hint ?? null;
  const shared = {
    id: inputId,
    value: draft ?? value,
    placeholder,
    autoComplete: "off",
    "aria-required": required || undefined,
    "aria-invalid": error !== null,
    "aria-describedby": message ? messageId : undefined,
    onChange: (event: { target: { value: string } }) => {
      setDraft(event.target.value);
      setError(null);
    },
    onKeyDown,
    onBlur: submit,
  } as const;

  return (
    <div className={cn(layout === "row" ? "grid grid-cols-[76px_minmax(0,1fr)] items-center gap-x-2" : "flex flex-col gap-1.5")}>
      <label htmlFor={inputId} className="truncate text-[12px] text-muted-foreground">
        {label}
      </label>
      {multiline ? (
        <textarea {...shared} rows={rows} className={cn(fieldClass, "resize-y py-1.5 leading-snug", error && "ring-1 ring-destructive")} />
      ) : (
        <input {...shared} type="text" spellCheck={false} className={cn(fieldClass, "h-7", error && "ring-1 ring-destructive")} />
      )}
      {message && (
        <p
          id={messageId}
          role={error ? "alert" : undefined}
          className={cn("text-[11px]", layout === "row" && "col-start-2 pt-1", error ? "text-destructive" : "text-warning")}
        >
          {message}
        </p>
      )}
    </div>
  );
}
