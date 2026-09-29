import { Search } from "lucide-react";
import { useId, useMemo, useState, type RefObject } from "react";
import { editorShortcuts, formatShortcut, type EditorShortcut, type ShortcutPlatform } from "@/lib/keymap";
import { OverlayDialog } from "./overlay-dialog";

type ShortcutGroup = EditorShortcut["group"];

const groupOrder: readonly ShortcutGroup[] = ["Editing", "Timeline", "Playback", "Navigation", "Panels"];

interface ShortcutRow {
  readonly id: string;
  readonly label: string;
  readonly keys: readonly string[];
}

interface ShortcutSection {
  readonly group: ShortcutGroup;
  readonly rows: readonly ShortcutRow[];
}

/** Registry rows grouped for display; a timeline and preview shortcut with the same label and keys is listed once. */
function shortcutSections(platform: ShortcutPlatform): readonly ShortcutSection[] {
  return groupOrder.map((group) => {
    const seen = new Set<string>();
    const rows: ShortcutRow[] = [];
    for (const shortcut of editorShortcuts) {
      if (shortcut.group !== group) continue;
      const keys = shortcut.bindings.map((binding) => formatShortcut(binding, platform));
      const signature = JSON.stringify([shortcut.label, keys]);
      if (seen.has(signature)) continue;
      seen.add(signature);
      rows.push({ id: shortcut.id, label: shortcut.label, keys });
    }
    return { group, rows };
  });
}

function filterSections(sections: readonly ShortcutSection[], query: string): readonly ShortcutSection[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return sections;
  return sections
    .map((section) => {
      if (section.group.toLowerCase().includes(needle)) return section;
      const rows = section.rows.filter(
        (row) => row.label.toLowerCase().includes(needle) || row.keys.some((key) => key.toLowerCase().includes(needle)),
      );
      return { group: section.group, rows };
    })
    .filter((section) => section.rows.length > 0);
}

interface ShortcutsSheetProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly platform: ShortcutPlatform;
  readonly returnFocusRef?: RefObject<HTMLElement | null>;
}

/** The Keyboard shortcuts sheet, generated from the keymap registry so it always matches the handlers. */
export function ShortcutsSheet({ open, onOpenChange, platform, returnFocusRef }: ShortcutsSheetProps) {
  const [query, setQuery] = useState("");
  const headingPrefix = useId();
  const sections = useMemo(() => shortcutSections(platform), [platform]);
  const visible = filterSections(sections, query);

  function changeOpen(next: boolean) {
    if (!next) setQuery("");
    onOpenChange(next);
  }

  return (
    <OverlayDialog
      open={open}
      onOpenChange={changeOpen}
      title="Keyboard shortcuts"
      size="lg"
      {...(returnFocusRef ? { returnFocusRef } : {})}
    >
      <div className="relative mx-4 mt-3 shrink-0">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-dim" aria-hidden />
        <input
          type="search"
          aria-label="Search shortcuts"
          placeholder="Search shortcuts"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          className="h-8 w-full rounded-control bg-raised pl-8 pr-2 text-[13px] text-foreground outline-none placeholder:text-dim focus-visible:ring-2 focus-visible:ring-ring"
        />
      </div>
      <div className="mt-2 min-h-0 flex-1 overflow-y-auto px-4 pb-4">
        {visible.length === 0 ? (
          <p className="py-6 text-center text-[13px] text-muted-foreground">No shortcuts match “{query.trim()}”.</p>
        ) : (
          <div className="gap-x-6 sm:columns-2">
            {visible.map((section) => {
              const headingId = `${headingPrefix}-${section.group}`;
              return (
                <section key={section.group} aria-labelledby={headingId} className="mt-2 min-w-0 break-inside-avoid">
                  <h3 id={headingId} className="py-1.5 text-[11px] font-medium uppercase tracking-wide text-dim">
                    {section.group}
                  </h3>
                  <ul aria-labelledby={headingId}>
                    {section.rows.map((row) => (
                      <li key={row.id} className="flex min-h-8 items-center gap-3 border-t border-line py-1">
                        <span className="min-w-0 flex-1 text-[13px]">{row.label}</span>
                        <span className="flex shrink-0 flex-wrap justify-end gap-1">
                          {row.keys.map((key) => (
                            <kbd key={key} className="rounded-md bg-raised px-1.5 py-0.5 font-sans text-[12px] text-foreground">
                              {key}
                            </kbd>
                          ))}
                        </span>
                      </li>
                    ))}
                  </ul>
                </section>
              );
            })}
          </div>
        )}
      </div>
    </OverlayDialog>
  );
}
