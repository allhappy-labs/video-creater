import { useId, useState, type FormEvent, type ReactNode, type RefObject } from "react";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";

interface MediaDialogProps {
  readonly title: string;
  readonly description?: ReactNode;
  readonly submitLabel: string;
  readonly destructive?: boolean;
  /** Locks the dialog: no closing, inputs and buttons disabled. */
  readonly busy?: boolean;
  readonly submitDisabled?: boolean;
  readonly error?: string | null;
  readonly returnFocusRef?: RefObject<HTMLElement | null> | undefined;
  readonly children?: ReactNode;
  onSubmit(): void;
  onClose(): void;
}

const buttonClass =
  "h-8 rounded-control px-3 text-[13px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel disabled:pointer-events-none disabled:opacity-50";

/** A centred form dialog for the Media tab with Cancel, one submit action and an inline error. */
export function MediaDialog({ title, description, submitLabel, destructive = false, busy = false, submitDisabled = false, error, returnFocusRef, children, onSubmit, onClose }: MediaDialogProps) {
  const descriptionId = useId();

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!busy && !submitDisabled) onSubmit();
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !busy && onClose()}>
      <DialogContent
        aria-describedby={description ? descriptionId : undefined}
        onEscapeKeyDown={(event) => busy && event.preventDefault()}
        onInteractOutside={(event) => busy && event.preventDefault()}
        onCloseAutoFocus={(event) => {
          const target = returnFocusRef?.current;
          if (!target) return;
          event.preventDefault();
          target.focus();
        }}
        className="left-1/2 top-1/2 max-h-[calc(100dvh-32px)] w-[min(360px,calc(100vw-32px))] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-panel border border-line p-4"
      >
        <form onSubmit={submit} noValidate>
          <DialogTitle className="break-words text-[15px] font-semibold">{title}</DialogTitle>
          {description && (
            <div id={descriptionId} className="mt-1 text-[13px] text-muted-foreground">
              {description}
            </div>
          )}
          {children && <div className="mt-3">{children}</div>}
          {error && (
            <p role="alert" className="mt-3 text-[12px] text-destructive">
              {error}
            </p>
          )}
          <div className="mt-4 flex justify-end gap-2">
            <button type="button" disabled={busy} onClick={onClose} className={cn(buttonClass, "text-foreground hover:bg-raised")}>
              Cancel
            </button>
            <button
              type="submit"
              disabled={busy || submitDisabled}
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

interface NameDialogProps {
  readonly title: string;
  readonly label: string;
  readonly initialName: string;
  readonly submitLabel: string;
  readonly returnFocusRef?: RefObject<HTMLElement | null> | undefined;
  /** Resolves false to keep the dialog open (the failure is shown in the dialog). */
  onSubmit(name: string): Promise<boolean>;
  onClose(): void;
}

/** New folder, Rename folder and Rename media: a trimmed, non-empty name. */
export function NameDialog({ title, label, initialName, submitLabel, returnFocusRef, onSubmit, onClose }: NameDialogProps) {
  const inputId = useId();
  const [name, setName] = useState(initialName);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    setBusy(true);
    setError(null);
    const saved = await onSubmit(name.trim());
    setBusy(false);
    if (saved) onClose();
    else setError(`The name could not be saved.`);
  }

  return (
    <MediaDialog
      title={title}
      submitLabel={submitLabel}
      busy={busy}
      submitDisabled={!name.trim()}
      error={error}
      returnFocusRef={returnFocusRef}
      onSubmit={() => void submit()}
      onClose={onClose}
    >
      <label htmlFor={inputId} className="text-[12px] text-muted-foreground">
        {label}
      </label>
      <input
        id={inputId}
        autoFocus
        value={name}
        disabled={busy}
        autoComplete="off"
        onChange={(event) => setName(event.target.value)}
        onFocus={(event) => event.currentTarget.select()}
        className="mt-1.5 h-8 w-full rounded-control border border-line bg-raised px-2.5 text-[13px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
      />
    </MediaDialog>
  );
}

interface ConfirmDeleteDialogProps {
  readonly title: string;
  readonly description: string;
  /** Why the delete can't run; shown instead of enabling Delete. */
  readonly blockedReason?: string | null;
  readonly returnFocusRef?: RefObject<HTMLElement | null> | undefined;
  onConfirm(): Promise<boolean>;
  onClose(): void;
}

/** Delete confirmation naming the object and what the delete affects. */
export function ConfirmDeleteDialog({ title, description, blockedReason = null, returnFocusRef, onConfirm, onClose }: ConfirmDeleteDialogProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function confirm() {
    setBusy(true);
    setError(null);
    const deleted = await onConfirm();
    setBusy(false);
    if (deleted) onClose();
    else setError("The delete could not be completed.");
  }

  return (
    <MediaDialog
      title={title}
      description={description}
      submitLabel="Delete"
      destructive
      busy={busy}
      submitDisabled={blockedReason !== null}
      error={blockedReason ?? error}
      returnFocusRef={returnFocusRef}
      onSubmit={() => void confirm()}
      onClose={onClose}
    />
  );
}
