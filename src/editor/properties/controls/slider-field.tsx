import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";
import { KeyframeButton, type KeyframeToggle } from "./keyframe-button";

export interface SliderFieldProps {
  readonly label: string;
  /** Committed value from the project. */
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly step?: number;
  /** Display text for the numeric field and `aria-valuetext`, for example "100%". */
  readonly format?: (value: number) => string;
  /** Parses typed text; null rejects it. The default accepts a number with an optional unit suffix. */
  readonly parse?: (text: string) => number | null;
  readonly disabled?: boolean;
  /** Called with each transient value while dragging; keyboard steps commit without a preview. */
  readonly onPreview?: (value: number) => void;
  /**
   * Called once per gesture: slider release, keyboard step, or numeric Enter/blur. Return the
   * `applyActions` promise so the field keeps showing the new value until the project updates.
   */
  readonly onCommit: (value: number) => unknown;
  readonly keyframe?: KeyframeToggle;
  /** Multiple selection with differing values: shows "Mixed" until the user changes it. */
  readonly mixed?: boolean;
  /** Called when the numeric field is committed empty; without it an empty field is invalid. */
  readonly onBlank?: () => unknown;
  readonly className?: string;
}

const defaultFormat = (value: number) => String(value);

/** A number with an optional unit suffix such as `%`, `°`, `×`, `x`, `s` or `dB`. */
function parseNumberWithUnit(text: string): number | null {
  const match = /^\s*([-+]?(?:\d+\.?\d*|\.\d+))\s*(?:%|°|×|x|s|db)?\s*$/i.exec(text);
  if (!match?.[1]) return null;
  const value = Number(match[1]);
  return Number.isFinite(value) ? value : null;
}

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  return typeof value === "object" && value !== null && typeof (value as { then?: unknown }).then === "function";
}

/** Label, slider, numeric field and optional ◇ keyframe toggle, holding the drag value locally. */
export function SliderField({
  label,
  value,
  min,
  max,
  step = 1,
  format = defaultFormat,
  parse = parseNumberWithUnit,
  disabled = false,
  onPreview,
  onCommit,
  keyframe,
  mixed = false,
  onBlank,
  className,
}: SliderFieldProps) {
  const inputId = useId();
  const errorId = useId();
  const [draft, setDraft] = useState<number | null>(null);
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const commitToken = useRef(0);
  const mounted = useRef(true);
  /**
   * Radix commits a keyboard step before it reports the change, so a step commits directly
   * and skips the preview; pointer drags preview each change and commit on release.
   */
  const keyboardStep = useRef(false);
  const shown = draft ?? value;
  const shownText = mixed && draft === null ? "Mixed" : format(shown);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const commit = (next: number) => {
    const token = ++commitToken.current;
    setDraft(next);
    const settle = () => {
      if (mounted.current && commitToken.current === token) setDraft(null);
    };
    const result = onCommit(next);
    if (isPromiseLike(result)) result.then(settle, settle);
    else settle();
  };

  const preview = (next: number) => {
    commitToken.current += 1;
    setDraft(next);
    onPreview?.(next);
  };

  const cancelEdit = () => {
    setText(null);
    setError(null);
  };

  const submitText = () => {
    if (text === null) return;
    if (onBlank && text.trim().length === 0) {
      cancelEdit();
      onBlank();
      return;
    }
    const parsed = parse(text);
    if (parsed === null) {
      setError("Enter a number.");
      return;
    }
    if (parsed < min || parsed > max) {
      setError(`Enter a value from ${format(min)} to ${format(max)}.`);
      return;
    }
    cancelEdit();
    if (mixed || parsed !== value) commit(parsed);
  };

  const onInputKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      submitText();
    } else if (event.key === "Escape" && text !== null) {
      event.preventDefault();
      event.stopPropagation();
      cancelEdit();
    }
  };

  return (
    <div className={cn("grid grid-cols-[76px_minmax(0,1fr)_60px_26px] items-center gap-x-2", className)}>
      <label htmlFor={inputId} className="truncate text-[12px] text-muted-foreground">
        {label}
      </label>
      <Slider
        label={label}
        valueText={shownText}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        value={[shown]}
        onKeyDown={() => {
          keyboardStep.current = true;
        }}
        onKeyUp={() => {
          keyboardStep.current = false;
        }}
        onPointerDown={() => {
          keyboardStep.current = false;
        }}
        onValueChange={([next]) => {
          if (next !== undefined && !keyboardStep.current) preview(next);
        }}
        onValueCommit={([next]) => {
          if (next !== undefined) commit(next);
        }}
      />
      <input
        id={inputId}
        type="text"
        inputMode="decimal"
        autoComplete="off"
        spellCheck={false}
        disabled={disabled}
        value={text ?? shownText}
        aria-invalid={error !== null}
        aria-describedby={error ? errorId : undefined}
        onChange={(event) => {
          setText(event.target.value);
          setError(null);
        }}
        onKeyDown={onInputKeyDown}
        onBlur={submitText}
        className={cn(
          "tabular-time h-7 w-full min-w-0 rounded-md bg-raised px-2 text-center text-[12px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-40",
          error && "ring-1 ring-destructive focus-visible:ring-destructive",
        )}
      />
      {keyframe ? <KeyframeButton label={label} {...keyframe} /> : <span aria-hidden />}
      {error && (
        <p id={errorId} className="col-span-4 pt-1 text-right text-[11px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
