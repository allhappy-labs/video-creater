import { ChevronDown } from "lucide-react";
import { useMemo, useRef, useState, type RefObject } from "react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip } from "@/components/ui/tooltip";
import {
  nestedSequenceItems,
  planDeleteTimeline,
  planRenameTimeline,
  projectTimelineEntries,
} from "@/lib/timeline-ops/multi-timeline";
import { useEditorStore } from "../store/editor-store-context";
import { blockedReason } from "./timeline-availability";
import { useTimelineCommands } from "./timeline-commands";
import { DialogField, TimelineDialog } from "./timeline-dialog";

type SelectorDialog = "rename" | "delete" | null;

function RenameTimelineDialog({
  open,
  timelineId,
  currentName,
  onClose,
  returnFocusRef,
}: {
  readonly open: boolean;
  readonly timelineId: string;
  readonly currentName: string;
  readonly onClose: () => void;
  readonly returnFocusRef: RefObject<HTMLElement | null>;
}) {
  const project = useEditorStore((state) => state.project);
  const commands = useTimelineCommands();
  const [name, setName] = useState(currentName);
  const [error, setError] = useState<string | null>(null);

  async function submit() {
    const reason = blockedReason(planRenameTimeline(project, timelineId, name));
    if (reason) {
      setError(reason);
      return;
    }
    if (await commands.renameTimeline(timelineId, name)) onClose();
  }

  return (
    <TimelineDialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title="Rename timeline"
      submitLabel="Rename"
      onSubmit={() => void submit()}
      returnFocusRef={returnFocusRef}
    >
      <DialogField
        label="Name"
        value={name}
        error={error}
        onChange={(value) => {
          setName(value);
          setError(null);
        }}
      />
    </TimelineDialog>
  );
}

/** Dropdown for the active timeline: switch, new, duplicate, rename, delete and nested sequences. */
export function TimelineSelector() {
  const project = useEditorStore((state) => state.project);
  const selectItems = useEditorStore((state) => state.selectItems);
  const commands = useTimelineCommands();
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [dialog, setDialog] = useState<SelectorDialog>(null);
  const entries = projectTimelineEntries(project);
  const activeId = project.activeTimelineId ?? entries[0]?.id ?? "main";
  const active = entries.find((entry) => entry.id === activeId) ?? entries[0];
  const activeName = active?.name ?? "Timeline";
  const deleteReason = blockedReason(planDeleteTimeline(project, activeId));
  const nested = useMemo(() => nestedSequenceItems(project), [project]);

  const deleteItem = (
    <DropdownMenuItem disabled={deleteReason !== null} onSelect={() => setDialog("delete")} className="text-destructive data-[disabled]:text-dim">
      Delete…
    </DropdownMenuItem>
  );

  return (
    <>
      <DropdownMenu>
        <Tooltip content="Timelines">
          <DropdownMenuTrigger asChild>
            <button
              ref={triggerRef}
              type="button"
              className="flex h-7 max-w-44 shrink-0 items-center gap-1 rounded-control pl-2 pr-1.5 text-[13px] font-medium text-foreground hover:bg-raised focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <span className="min-w-0 truncate">{activeName}</span>
              <ChevronDown className="h-3.5 w-3.5 shrink-0 text-dim" aria-hidden />
            </button>
          </DropdownMenuTrigger>
        </Tooltip>
        <DropdownMenuContent align="start" className="min-w-56" onCloseAutoFocus={(event) => dialog && event.preventDefault()}>
          <DropdownMenuLabel>Timelines</DropdownMenuLabel>
          <DropdownMenuRadioGroup value={activeId} onValueChange={(timelineId) => void commands.switchTimeline(timelineId)}>
            {entries.map((entry) => (
              <DropdownMenuRadioItem key={entry.id} value={entry.id}>
                <span className="min-w-0 truncate">{entry.name}</span>
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => void commands.createTimeline(false)}>New timeline</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => void commands.createTimeline(true)}>Duplicate timeline</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => setDialog("rename")}>Rename…</DropdownMenuItem>
          {deleteReason ? (
            <Tooltip content={deleteReason} side="right">
              {deleteItem}
            </Tooltip>
          ) : (
            deleteItem
          )}
          {nested.length > 0 && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuLabel>Nested sequences</DropdownMenuLabel>
              {nested.map((entry) => (
                <DropdownMenuItem key={entry.item.id} onSelect={() => selectItems([entry.item.id])}>
                  <span className="min-w-0 flex-1 truncate">{entry.item.label}</span>
                  {entry.timelineName && <span className="shrink-0 text-[11px] text-dim">{entry.timelineName}</span>}
                </DropdownMenuItem>
              ))}
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
      {dialog === "rename" && (
        <RenameTimelineDialog open timelineId={activeId} currentName={activeName} onClose={() => setDialog(null)} returnFocusRef={triggerRef} />
      )}
      <TimelineDialog
        open={dialog === "delete"}
        onOpenChange={(next) => !next && setDialog(null)}
        title={`Delete ${activeName}?`}
        description="This cannot be undone."
        submitLabel="Delete timeline"
        destructive
        returnFocusRef={triggerRef}
        onSubmit={() => {
          setDialog(null);
          void commands.deleteTimeline(activeId);
        }}
      />
    </>
  );
}
