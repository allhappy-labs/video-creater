import { House, Redo2, Undo2 } from "lucide-react";
import { IconButton } from "@/components/ui/icon-button";
import { cn } from "@/lib/utils";
import { ExportPopover } from "../overlays/export-popover";
import { GearMenu } from "../overlays/gear-menu";
import { TasksIndicator } from "../overlays/tasks-indicator";
import { useEditorStore } from "../store/editor-store-context";
import type { SaveStatus } from "../store/project-slice";
import { useNarrowTopBar } from "./use-layout-mode";
import { safeAreaTopClass } from "./use-safe-area";
import { RemoteAccessIndicator } from "./remote-access-indicator";

const saveStatusLabel: Record<SaveStatus, string> = {
  saved: "Saved",
  saving: "Saving…",
  unsaved: "Not saved",
  failed: "Save failed",
  uncertain: "Save unconfirmed",
};

interface TopBarProps {
  readonly compact: boolean;
  readonly onOpenProjectHome: () => void;
  /** Receives the editor menu button so settings can return focus to it on close. */
  readonly onOpenSettings: (originElement: HTMLElement) => void;
  readonly onOpenProjectSettings: (originElement: HTMLElement) => void;
}

export function TopBar({ compact, onOpenProjectHome, onOpenSettings, onOpenProjectSettings }: TopBarProps) {
  const name = useEditorStore((state) => state.project.name);
  const saveStatus = useEditorStore((state) => state.saveStatus);
  const canUndo = useEditorStore((state) => state.canUndo());
  const canRedo = useEditorStore((state) => state.history.future.length > 0);
  const undo = useEditorStore((state) => state.undo);
  const redo = useEditorStore((state) => state.redo);
  const narrow = useNarrowTopBar();

  // Phones: the name truncates first; Home, Undo, Redo, the tasks pill and Export never shrink.
  return (
    <header className={cn("flex h-12 shrink-0 items-center", compact ? "gap-1 px-1.5" : "gap-1.5 px-2.5", safeAreaTopClass)}>
      <IconButton label="Home" className="shrink-0" onClick={onOpenProjectHome}>
        <House className="h-4 w-4" aria-hidden />
      </IconButton>
      <span data-testid="project-name" className={cn("min-w-0 truncate font-semibold", compact ? "px-1" : "px-2")} title={name}>
        {name}
      </span>
      {!compact && (
        <span className="text-xs text-dim" role="status">
          {saveStatusLabel[saveStatus]}
        </span>
      )}
      <div className="min-w-0 flex-1" />
      <RemoteAccessIndicator compact={compact} />
      <IconButton label="Undo" className="shrink-0" disabled={!canUndo} onClick={() => void undo()}>
        <Undo2 className="h-4 w-4" aria-hidden />
      </IconButton>
      <IconButton label="Redo" className="shrink-0" disabled={!canRedo} onClick={() => void redo()}>
        <Redo2 className="h-4 w-4" aria-hidden />
      </IconButton>
      <TasksIndicator compact={compact} />
      <GearMenu more={narrow} onOpenSettings={onOpenSettings} onOpenProjectSettings={onOpenProjectSettings} />
      <ExportPopover compact={compact} />
    </header>
  );
}
