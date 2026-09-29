import { Paperclip, Replace } from "lucide-react";

/** Replace mode ("Replace with media…"): the next tile click replaces the named clip. */
export function ReplaceBanner({ label, error, onCancel }: { readonly label: string; readonly error: string | null; onCancel(): void }) {
  return (
    <div role="region" aria-label="Replace mode" className="flex flex-col gap-1 rounded-control bg-accent-soft px-2.5 py-2">
      <div className="flex items-center gap-2">
        <Replace className="h-4 w-4 shrink-0 text-primary" aria-hidden />
        <p className="min-w-0 flex-1 text-[12px] text-foreground">
          Choose media to replace <span className="font-medium">{label}</span>
        </p>
        <button
          type="button"
          onClick={onCancel}
          className="h-7 shrink-0 rounded-md px-2 text-[12px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Cancel
        </button>
      </div>
      {error && (
        <p role="alert" className="text-[12px] text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

/** Attach mode (the AI composer's attach button): the next tile click mentions that media in the AI draft. */
export function AttachBanner({ onCancel }: { onCancel(): void }) {
  return (
    <div role="region" aria-label="Attach mode" className="flex items-center gap-2 rounded-control bg-accent-soft px-2.5 py-2">
      <Paperclip className="h-4 w-4 shrink-0 text-primary" aria-hidden />
      <p className="min-w-0 flex-1 text-[12px] text-foreground">Choose media to attach to your AI request</p>
      <button
        type="button"
        onClick={onCancel}
        className="h-7 shrink-0 rounded-md px-2 text-[12px] font-medium text-foreground hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        Cancel
      </button>
    </div>
  );
}
