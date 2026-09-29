import { BookOpen, Ellipsis, Keyboard, Plug, Settings, Settings2, SlidersHorizontal } from "lucide-react";
import { useRef } from "react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { IconButton } from "@/components/ui/icon-button";
import { useEditorStore } from "../store/editor-store-context";
import type { EditorOverlay } from "../store/ui-slice";
import { shortcutHint, useShortcutPlatform } from "../shell/use-shortcut-platform";
import { ConnectAgentsDialog } from "./connect-agents-dialog";
import { ProjectSkillsDialog } from "./project-skills-dialog";
import { ShortcutsSheet } from "./shortcuts-sheet";

interface GearMenuProps {
  /** Narrow phones (under 380 px): the same menu behind a "More" button. */
  readonly more?: boolean;
  /** Receives the menu button so settings can return focus to it on close. */
  readonly onOpenSettings: (originElement: HTMLElement) => void;
  readonly onOpenProjectSettings: (originElement: HTMLElement) => void;
}

/** `aria-keyshortcuts` names Meta on macOS and Control elsewhere. */
function ariaKeyShortcut(binding: string, platform: string): string {
  return binding.replace("Mod", platform === "macos" ? "Meta" : "Control");
}

const iconClass = "h-3.5 w-3.5 text-muted-foreground";

/** The top bar's gear menu and the overlays it opens; the native menu opens the same overlays via `openOverlay`. */
export function GearMenu({ more = false, onOpenSettings, onOpenProjectSettings }: GearMenuProps) {
  const platform = useShortcutPlatform();
  const projectDir = useEditorStore((state) => state.projectDir);
  const overlay = useEditorStore((state) => state.overlay);
  const openOverlay = useEditorStore((state) => state.openOverlay);
  const closeOverlay = useEditorStore((state) => state.closeOverlay);
  const menuButtonRef = useRef<HTMLButtonElement>(null);

  const withMenuOrigin = (open: (originElement: HTMLElement) => void) => () => {
    if (menuButtonRef.current) open(menuButtonRef.current);
  };
  const overlayProps = (id: EditorOverlay) => ({
    open: overlay === id,
    onOpenChange: (next: boolean) => {
      if (next) openOverlay(id);
      else if (overlay === id) closeOverlay();
    },
    returnFocusRef: menuButtonRef,
  });

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <IconButton ref={menuButtonRef} label={more ? "More" : "Editor menu"} className="shrink-0">
            {more ? <Ellipsis className="h-4 w-4" aria-hidden /> : <Settings className="h-4 w-4" aria-hidden />}
          </IconButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align="end"
          className="min-w-56"
          // A dialog opened from an item takes focus; the dialog returns it to the menu button when it closes.
          onCloseAutoFocus={(event) => {
            if (overlay !== null) event.preventDefault();
          }}
        >
          <DropdownMenuItem onSelect={withMenuOrigin(onOpenProjectSettings)}>
            <SlidersHorizontal className={iconClass} aria-hidden />
            Project settings
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={withMenuOrigin(onOpenSettings)}>
            <Settings2 className={iconClass} aria-hidden />
            App settings
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem
            onSelect={() => openOverlay("shortcuts")}
            aria-keyshortcuts={ariaKeyShortcut("Mod+/", platform)}
          >
            <Keyboard className={iconClass} aria-hidden />
            Keyboard shortcuts
            <span className="ml-auto pl-4 text-[12px] text-dim" aria-hidden>
              {shortcutHint("editor.shortcuts", platform)}
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => openOverlay("connectAgents")}>
            <Plug className={iconClass} aria-hidden />
            Connect external agents…
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={() => openOverlay("projectSkills")}>
            <BookOpen className={iconClass} aria-hidden />
            Project skills…
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ShortcutsSheet platform={platform} {...overlayProps("shortcuts")} />
      <ConnectAgentsDialog projectDir={projectDir} platform={platform} {...overlayProps("connectAgents")} />
      <ProjectSkillsDialog
        {...overlayProps("projectSkills")}
        onOpenProjectSettings={() => {
          closeOverlay();
          if (menuButtonRef.current) onOpenProjectSettings(menuButtonRef.current);
        }}
      />
    </>
  );
}
