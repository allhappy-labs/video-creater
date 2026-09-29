export type ShortcutPlatform = "macos" | "linux" | "windows";
export type ShortcutScope = "global" | "timeline" | "preview";

export interface EditorShortcut {
  readonly id: string;
  readonly label: string;
  readonly group: "Editing" | "Timeline" | "Playback" | "Navigation" | "Panels";
  readonly scope: ShortcutScope;
  /** Bindings like "Mod+K", "Shift+Delete", "Space". "Mod" is ⌘ on macOS, Ctrl elsewhere. */
  readonly bindings: readonly string[];
}

export const editorShortcuts: readonly EditorShortcut[] = [
  { id: "editor.undo", label: "Undo", group: "Editing", scope: "global", bindings: ["Mod+Z"] },
  { id: "editor.redo", label: "Redo", group: "Editing", scope: "global", bindings: ["Shift+Mod+Z"] },
  { id: "editor.export", label: "Export", group: "Editing", scope: "global", bindings: ["Mod+E"] },
  { id: "editor.import", label: "Import media", group: "Editing", scope: "global", bindings: ["Mod+I"] },
  { id: "editor.shortcuts", label: "Keyboard shortcuts", group: "Panels", scope: "global", bindings: ["Mod+/"] },
  { id: "editor.tab.ai", label: "AI", group: "Panels", scope: "global", bindings: ["Mod+1"] },
  { id: "editor.tab.media", label: "Media", group: "Panels", scope: "global", bindings: ["Mod+2"] },
  { id: "editor.tab.audio", label: "Audio", group: "Panels", scope: "global", bindings: ["Mod+3"] },
  { id: "editor.tab.text", label: "Text", group: "Panels", scope: "global", bindings: ["Mod+4"] },
  { id: "editor.tab.captions", label: "Captions", group: "Panels", scope: "global", bindings: ["Mod+5"] },
  { id: "editor.tab.effects", label: "Effects", group: "Panels", scope: "global", bindings: ["Mod+6"] },
  { id: "editor.clearSelection", label: "Clear selection", group: "Editing", scope: "global", bindings: ["Escape"] },
  { id: "timeline.selectTool", label: "Select tool", group: "Timeline", scope: "timeline", bindings: ["V"] },
  { id: "timeline.bladeTool", label: "Blade tool", group: "Timeline", scope: "timeline", bindings: ["C"] },
  { id: "timeline.split", label: "Split at playhead", group: "Timeline", scope: "timeline", bindings: ["S", "Mod+K"] },
  { id: "timeline.markIn", label: "Mark in", group: "Timeline", scope: "timeline", bindings: ["I"] },
  { id: "timeline.markOut", label: "Mark out", group: "Timeline", scope: "timeline", bindings: ["O"] },
  { id: "timeline.delete", label: "Delete", group: "Timeline", scope: "timeline", bindings: ["Delete", "Backspace"] },
  { id: "timeline.rippleDelete", label: "Ripple delete", group: "Timeline", scope: "timeline", bindings: ["Shift+Delete", "Shift+Backspace"] },
  { id: "timeline.duplicate", label: "Duplicate", group: "Timeline", scope: "timeline", bindings: ["Mod+D"] },
  { id: "timeline.copy", label: "Copy", group: "Timeline", scope: "timeline", bindings: ["Mod+C"] },
  { id: "timeline.cut", label: "Cut", group: "Timeline", scope: "timeline", bindings: ["Mod+X"] },
  { id: "timeline.paste", label: "Paste", group: "Timeline", scope: "timeline", bindings: ["Mod+V"] },
  { id: "timeline.pasteInsert", label: "Paste insert", group: "Timeline", scope: "timeline", bindings: ["Shift+Mod+V"] },
  { id: "timeline.trimStart", label: "Trim start to playhead", group: "Timeline", scope: "timeline", bindings: ["["] },
  { id: "timeline.trimEnd", label: "Trim end to playhead", group: "Timeline", scope: "timeline", bindings: ["]"] },
  { id: "timeline.nudgeLeft", label: "Nudge left", group: "Timeline", scope: "timeline", bindings: ["Shift+ArrowLeft"] },
  { id: "timeline.nudgeRight", label: "Nudge right", group: "Timeline", scope: "timeline", bindings: ["Shift+ArrowRight"] },
  { id: "timeline.moveTrackUp", label: "Move to track above", group: "Timeline", scope: "timeline", bindings: ["Shift+ArrowUp"] },
  { id: "timeline.moveTrackDown", label: "Move to track below", group: "Timeline", scope: "timeline", bindings: ["Shift+ArrowDown"] },
  { id: "timeline.selectForwardTrack", label: "Select forward on track", group: "Timeline", scope: "timeline", bindings: ["A"] },
  { id: "timeline.selectForwardAll", label: "Select forward on all tracks", group: "Timeline", scope: "timeline", bindings: ["Shift+A"] },
  { id: "navigation.stepBack", label: "Step playhead back", group: "Navigation", scope: "timeline", bindings: ["ArrowLeft"] },
  { id: "navigation.stepForward", label: "Step playhead forward", group: "Navigation", scope: "timeline", bindings: ["ArrowRight"] },
  { id: "navigation.previousEdit", label: "Previous edit point", group: "Navigation", scope: "timeline", bindings: ["PageUp"] },
  { id: "navigation.nextEdit", label: "Next edit point", group: "Navigation", scope: "timeline", bindings: ["PageDown"] },
  { id: "navigation.start", label: "Go to start", group: "Navigation", scope: "timeline", bindings: ["Home"] },
  { id: "navigation.end", label: "Go to end", group: "Navigation", scope: "timeline", bindings: ["End"] },
  { id: "playback.toggle", label: "Play / pause", group: "Playback", scope: "timeline", bindings: ["Space"] },
  { id: "preview.toggle", label: "Play / pause", group: "Playback", scope: "preview", bindings: ["Space"] },
  { id: "preview.stepBack", label: "Previous frame", group: "Playback", scope: "preview", bindings: ["ArrowLeft"] },
  { id: "preview.stepForward", label: "Next frame", group: "Playback", scope: "preview", bindings: ["ArrowRight"] },
];

const byId = new Map(editorShortcuts.map((shortcut) => [shortcut.id, shortcut]));

export function shortcutById(id: string): EditorShortcut {
  const shortcut = byId.get(id);
  if (!shortcut) throw new Error(`Unknown shortcut: ${id}`);
  return shortcut;
}

interface ParsedBinding {
  readonly key: string;
  readonly mod: boolean;
  readonly shift: boolean;
  readonly alt: boolean;
}

function parseBinding(binding: string): ParsedBinding {
  const parts = binding.split("+");
  const key = parts[parts.length - 1] ?? "";
  return {
    key: key === "Space" ? " " : key.length === 1 ? key.toLowerCase() : key,
    mod: parts.includes("Mod"),
    shift: parts.includes("Shift"),
    alt: parts.includes("Alt"),
  };
}

function eventKey(event: KeyboardEvent): string {
  return event.key.length === 1 ? event.key.toLowerCase() : event.key;
}

export function matchShortcut(
  event: KeyboardEvent,
  scope: ShortcutScope,
  platform: ShortcutPlatform,
): EditorShortcut | null {
  const modPressed = platform === "macos" ? event.metaKey : event.ctrlKey;
  const otherModPressed = platform === "macos" ? event.ctrlKey : event.metaKey;
  if (otherModPressed) return null;
  for (const shortcut of editorShortcuts) {
    if (shortcut.scope !== scope) continue;
    for (const binding of shortcut.bindings) {
      const parsed = parseBinding(binding);
      if (
        parsed.key === eventKey(event) &&
        parsed.mod === modPressed &&
        parsed.shift === event.shiftKey &&
        parsed.alt === event.altKey
      ) {
        return shortcut;
      }
    }
  }
  return null;
}

const macSymbols: Record<string, string> = { Mod: "⌘", Shift: "⇧", Alt: "⌥" };

export function formatShortcut(binding: string, platform: ShortcutPlatform): string {
  const parts = binding.split("+");
  if (platform === "macos") {
    return parts.map((part) => macSymbols[part] ?? part).join("");
  }
  const order = ["Mod", "Shift", "Alt"];
  const modifiers = order.filter((modifier) => parts.includes(modifier));
  const key = parts.filter((part) => !order.includes(part));
  return [...modifiers.map((modifier) => (modifier === "Mod" ? "Ctrl" : modifier)), ...key].join("+");
}
