import { useId, type FormEvent, type ReactNode, type RefObject } from "react";
import { Dialog, DialogClose, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

interface TimelineDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly title: string;
  readonly description?: string;
  readonly submitLabel: string;
  readonly destructive?: boolean;
  readonly onSubmit: () => void;
  /** Receives focus on close; menus that open dialogs have already unmounted their items. */
  readonly returnFocusRef?: RefObject<HTMLElement | null>;
  readonly children?: ReactNode;
}

const buttonClass =
  "h-8 rounded-control px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel";

/** A small centred form dialog with Cancel and one submit action. */
export function TimelineDialog({
  open,
  onOpenChange,
  title,
  description,
  submitLabel,
  destructive = false,
  onSubmit,
  returnFocusRef,
  children,
}: TimelineDialogProps) {
  const descriptionId = useId();

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit();
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        aria-describedby={description ? descriptionId : undefined}
        onCloseAutoFocus={(event) => {
          const target = returnFocusRef?.current;
          if (!target) return;
          event.preventDefault();
          target.focus();
        }}
        className="left-1/2 top-1/2 w-[min(360px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 rounded-panel border border-line p-4"
      >
        <form onSubmit={submit} noValidate>
          <DialogTitle className="text-[15px] font-semibold">{title}</DialogTitle>
          {description && (
            <p id={descriptionId} className="mt-1 text-[13px] text-muted-foreground">
              {description}
            </p>
          )}
          {children && <div className="mt-3">{children}</div>}
          <div className="mt-4 flex justify-end gap-2">
            <DialogClose className={cn(buttonClass, "text-foreground hover:bg-raised")}>Cancel</DialogClose>
            <button
              type="submit"
              className={cn(
                buttonClass,
                destructive ? "bg-destructive text-destructive-foreground hover:bg-destructive/90" : "bg-primary text-primary-foreground hover:bg-primary/90",
              )}
            >
              {submitLabel}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}

interface DialogFieldProps {
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  /** Inline validation copy under the input; marks the input invalid. */
  readonly error: string | null;
  readonly hint?: string;
  readonly suffix?: string;
  readonly inputMode?: "text" | "decimal";
}

/** A labelled text input with an inline error wired through `aria-invalid` and `aria-describedby`. */
export function DialogField({ label, value, onChange, error, hint, suffix, inputMode = "text" }: DialogFieldProps) {
  const inputId = useId();
  const messageId = useId();
  const message = error ?? hint;
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={inputId} className="text-[12px] text-muted-foreground">
        {label}
      </label>
      <div className="relative">
        <input
          id={inputId}
          autoFocus
          value={value}
          inputMode={inputMode}
          autoComplete="off"
          spellCheck={false}
          aria-invalid={error !== null}
          aria-describedby={message ? messageId : undefined}
          onChange={(event) => onChange(event.target.value)}
          onFocus={(event) => event.currentTarget.select()}
          className={cn(
            "tabular-time h-8 w-full rounded-control border bg-raised px-2.5 text-[13px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring",
            suffix && "pr-7",
            error ? "border-destructive" : "border-line",
          )}
        />
        {suffix && <span className="pointer-events-none absolute inset-y-0 right-2.5 flex items-center text-[13px] text-dim">{suffix}</span>}
      </div>
      {message && (
        <p id={messageId} role={error ? "alert" : undefined} className={cn("text-[12px]", error ? "text-destructive" : "text-dim")}>
          {message}
        </p>
      )}
    </div>
  );
}
