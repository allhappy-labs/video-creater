import { useId, useState, type RefObject } from "react";
import { matteAspectOptions, mattePreviewSize, normalizedHex, type MatteAspectRatio, type MatteCreateInput } from "@/lib/media/matte";
import { cn } from "@/lib/utils";
import { MediaDialog } from "./folder-dialogs";

/** Quick picks; the hex field and color picker take any six-digit color. */
const swatches = ["#000000", "#FFFFFF", "#111827", "#00B140", "#0047BB", "#F5E6C8"] as const;
const invalidHexCopy = "Enter a six-digit hex color, such as #112233.";
const matteFailedCopy = "The matte could not be created.";

interface MatteDialogProps {
  readonly timelineWidth: number;
  readonly timelineHeight: number;
  readonly returnFocusRef?: RefObject<HTMLElement | null>;
  /** Rejects with user-facing copy (e.g. the project needs saving first). */
  onCreate(input: MatteCreateInput): Promise<unknown>;
  onClose(): void;
}

/** Create matte: a solid image in the project, sized from the aspect choice and the project frame. */
export function MatteDialog({ timelineWidth, timelineHeight, returnFocusRef, onCreate, onClose }: MatteDialogProps) {
  const hexId = useId();
  const aspectId = useId();
  const hexErrorId = useId();
  const [hex, setHex] = useState("#000000");
  const [aspectRatio, setAspectRatio] = useState<MatteAspectRatio>("Project");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const validHex = normalizedHex(hex);
  const [width, height] = mattePreviewSize(aspectRatio, timelineWidth || 1920, timelineHeight || 1080);

  async function create() {
    if (!validHex || busy) return;
    setBusy(true);
    setError(null);
    try {
      await onCreate({ hex: validHex, aspectRatio });
      onClose();
    } catch (failure) {
      setError(failure instanceof Error && failure.message ? failure.message : matteFailedCopy);
    } finally {
      setBusy(false);
    }
  }

  return (
    <MediaDialog
      title="Create matte"
      description="Add a solid image to this project."
      submitLabel={busy ? "Creating…" : "Create matte"}
      busy={busy}
      submitDisabled={!validHex}
      error={error}
      returnFocusRef={returnFocusRef}
      onSubmit={() => void create()}
      onClose={onClose}
    >
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1.5">
          <label htmlFor={hexId} className="text-[12px] text-muted-foreground">
            Matte color
          </label>
          <div role="group" aria-label="Color swatches" className="flex flex-wrap gap-1.5">
            {swatches.map((swatch) => (
              <button
                key={swatch}
                type="button"
                disabled={busy}
                aria-label={`Use ${swatch}`}
                aria-pressed={validHex === swatch}
                onClick={() => setHex(swatch)}
                style={{ backgroundColor: swatch }}
                className={cn(
                  "h-7 w-7 rounded-full border border-line focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-panel",
                  validHex === swatch && "ring-2 ring-primary ring-offset-2 ring-offset-panel",
                )}
              />
            ))}
          </div>
          <div className="flex gap-2">
            <input
              type="color"
              aria-label="Matte color picker"
              disabled={busy}
              value={(validHex ?? "#000000").toLowerCase()}
              onChange={(event) => setHex(event.target.value.toUpperCase())}
              className="h-8 w-10 cursor-pointer rounded-control border border-line bg-transparent p-0.5"
            />
            <input
              id={hexId}
              autoFocus
              value={hex}
              disabled={busy}
              spellCheck={false}
              autoComplete="off"
              aria-invalid={!validHex}
              aria-describedby={validHex ? undefined : hexErrorId}
              onChange={(event) => setHex(event.target.value)}
              className={cn(
                "tabular-time h-8 min-w-0 flex-1 rounded-control border bg-raised px-2.5 text-[13px] uppercase text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring",
                validHex ? "border-line" : "border-destructive",
              )}
            />
          </div>
          {!validHex && (
            <p id={hexErrorId} role="alert" className="text-[12px] text-destructive">
              {invalidHexCopy}
            </p>
          )}
        </div>
        <div className="flex flex-col gap-1.5">
          <label htmlFor={aspectId} className="text-[12px] text-muted-foreground">
            Matte aspect
          </label>
          <select
            id={aspectId}
            value={aspectRatio}
            disabled={busy}
            onChange={(event) => setAspectRatio(event.target.value as MatteAspectRatio)}
            className="h-8 rounded-control border border-line bg-raised px-2 text-[13px] text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {matteAspectOptions.map((option) => (
              <option key={option} value={option}>
                {option}
              </option>
            ))}
          </select>
        </div>
        <figure aria-label="Matte preview" className="rounded-control bg-raised p-2">
          <div className="flex aspect-video items-center justify-center overflow-hidden rounded-md bg-background">
            <div
              className="max-h-full max-w-full border border-line"
              style={{
                width: `${Math.min(100, (width / Math.max(width, height)) * 82)}%`,
                aspectRatio: `${width} / ${height}`,
                backgroundColor: validHex ?? "#000000",
              }}
            />
          </div>
          <figcaption className="mt-1.5 flex justify-between text-[11px] text-dim">
            <span>{aspectRatio}</span>
            <span className="tabular-time text-foreground">
              {width} × {height}
            </span>
          </figcaption>
        </figure>
      </div>
    </MediaDialog>
  );
}
