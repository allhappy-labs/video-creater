import { quickEdits, type QuickEdit } from "@/lib/agent/quick-edits";

/** Suggestions for an empty conversation; choosing one fills the composer without sending. */
export function QuickEdits({ onChoose }: { onChoose(edit: QuickEdit): void }) {
  return (
    <ul aria-label="Quick edits" className="flex flex-wrap gap-1.5 px-3 pb-1">
      {quickEdits.map((edit) => (
        <li key={edit.id}>
          <button
            type="button"
            onClick={() => onChoose(edit)}
            className="rounded-[14px] border border-line px-2.5 py-1 text-[12px] text-muted-foreground transition-colors hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-none"
          >
            {edit.label}
          </button>
        </li>
      ))}
    </ul>
  );
}
