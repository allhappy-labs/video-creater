import { Film, Image as ImageIcon } from "lucide-react";
import { useId, useMemo, useState, type KeyboardEvent } from "react";
import { activeMentionQuery, type MentionTarget } from "@/lib/agent/mentions";
import { cn } from "@/lib/utils";

const maxOptions = 8;

const kindLabel: Record<MentionTarget["kind"], string> = { media: "Media", timelineItem: "Clip" };

/** Targets whose name contains the query, names starting with it first. */
function matchingTargets(targets: readonly MentionTarget[], query: string): MentionTarget[] {
  const needle = query.trim().toLowerCase();
  const matches = targets.filter((target) => target.name.toLowerCase().includes(needle));
  const starts = matches.filter((target) => target.name.toLowerCase().startsWith(needle));
  return [...starts, ...matches.filter((target) => !starts.includes(target))].slice(0, maxOptions);
}

interface MentionPickerInput {
  readonly draft: string;
  readonly caret: number;
  readonly focused: boolean;
  readonly targets: readonly MentionTarget[];
  onPick(target: MentionTarget): void;
}

export interface MentionPickerController {
  readonly open: boolean;
  readonly listboxId: string;
  readonly options: readonly MentionTarget[];
  readonly activeIndex: number;
  readonly activeOptionId: string | undefined;
  optionId(index: number): string;
  /** Handles the picker keys while it is open; returns true when the key was used. */
  onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>): boolean;
  pick(target: MentionTarget): void;
}

/** `@` typed before the caret opens the picker; arrows move, Enter or Tab inserts, Escape closes. */
export function useMentionPicker({ draft, caret, focused, targets, onPick }: MentionPickerInput): MentionPickerController {
  const listboxId = useId();
  const active = activeMentionQuery(draft, caret);
  const [dismissedStart, setDismissedStart] = useState<number | null>(null);
  const [selection, setSelection] = useState<{ readonly key: string; readonly index: number }>({ key: "", index: 0 });
  const query = active?.query ?? "";
  const options = useMemo(() => matchingTargets(targets, query), [targets, query]);
  const queryKey = active ? `${active.start.toString()}:${query}` : "";
  const open = focused && active !== null && active.start !== dismissedStart && options.length > 0;
  // A new query starts from the first option.
  const activeIndex = selection.key === queryKey ? Math.min(selection.index, Math.max(0, options.length - 1)) : 0;
  const optionId = (index: number) => `${listboxId}-option-${index.toString()}`;

  function pick(target: MentionTarget) {
    setDismissedStart(null);
    onPick(target);
  }

  return {
    open,
    listboxId,
    options,
    activeIndex,
    activeOptionId: open ? optionId(activeIndex) : undefined,
    optionId,
    pick,
    onKeyDown(event) {
      if (!open || !active) return false;
      const count = options.length;
      switch (event.key) {
        case "ArrowDown":
          setSelection({ key: queryKey, index: (activeIndex + 1) % count });
          return true;
        case "ArrowUp":
          setSelection({ key: queryKey, index: (activeIndex - 1 + count) % count });
          return true;
        case "Enter":
        case "Tab": {
          const target = options[activeIndex];
          if (!target) return false;
          pick(target);
          return true;
        }
        case "Escape":
          setDismissedStart(active.start);
          return true;
        default:
          return false;
      }
    },
  };
}

/** The mention options above the composer, by human name and kind. Focus stays in the textarea. */
export function MentionPicker({ controller }: { readonly controller: MentionPickerController }) {
  if (!controller.open) return null;
  return (
    <ul
      id={controller.listboxId}
      role="listbox"
      aria-label="Mention media or a clip"
      className="absolute inset-x-0 bottom-full z-20 mb-1.5 max-h-64 overflow-y-auto rounded-control border border-line bg-popover p-1 text-popover-foreground shadow-xl"
    >
      {controller.options.map((target, index) => {
        const Icon = target.kind === "media" ? ImageIcon : Film;
        const selected = index === controller.activeIndex;
        return (
          <li
            key={`${target.kind}-${target.id}`}
            id={controller.optionId(index)}
            role="option"
            aria-selected={selected}
            // Keep focus (and the caret) in the textarea.
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => controller.pick(target)}
            className={cn("flex cursor-default items-center gap-2 rounded-md px-2 py-1.5 text-[13px]", selected && "bg-raised")}
          >
            <Icon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
            <span className="min-w-0 flex-1 truncate">{target.name}</span>
            <span className="shrink-0 text-[11px] text-dim">{kindLabel[target.kind]}</span>
          </li>
        );
      })}
    </ul>
  );
}
