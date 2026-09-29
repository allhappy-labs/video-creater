import { useEffect, useId, useRef, useState } from "react";

interface WordEditorProps {
  readonly word: string;
  /** Resolves null when the fix was applied, or the reason it was not. */
  onCommit(text: string): Promise<string | null>;
  /** `restoreFocus` is true for Escape, false when focus already moved elsewhere. */
  onCancel(restoreFocus: boolean): void;
}

/** Inline input replacing a transcript word: Enter commits, Escape or leaving the field cancels. */
export function WordEditor({ word, onCommit, onCancel }: WordEditorProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(word);
  const [error, setError] = useState<string | null>(null);
  const committing = useRef(false);
  const errorId = useId();

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const commit = async () => {
    if (committing.current) return;
    committing.current = true;
    const failure = await onCommit(value);
    committing.current = false;
    setError(failure);
  };

  return (
    <span className="relative inline-flex align-baseline">
      <input
        ref={inputRef}
        type="text"
        value={value}
        aria-label={`Fix word “${word}”`}
        aria-invalid={error ? true : undefined}
        aria-describedby={error ? errorId : undefined}
        size={Math.max(value.length, 2)}
        spellCheck
        onChange={(event) => {
          setValue(event.target.value);
          setError(null);
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            void commit();
          } else if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            onCancel(true);
          }
        }}
        onBlur={() => {
          if (!committing.current) onCancel(false);
        }}
        className="h-6 rounded-sm bg-panel px-1 text-[13px] text-foreground ring-2 ring-primary focus-visible:outline-none aria-[invalid=true]:ring-destructive"
      />
      {error && (
        <span id={errorId} role="alert" className="absolute left-0 top-full z-10 mt-1 whitespace-nowrap rounded-md bg-popover px-2 py-1 text-[11px] text-destructive shadow-lg">
          {error}
        </span>
      )}
    </span>
  );
}
