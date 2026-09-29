import { describe, expect, it } from "vitest";
import {
  editorShortcuts,
  formatShortcut,
  matchShortcut,
  shortcutById,
  type ShortcutPlatform,
} from "./keymap";

function keyEvent(init: Partial<KeyboardEventInit> & { key: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", init);
}

describe("keymap", () => {
  it("has unique ids and unique bindings per scope", () => {
    const ids = editorShortcuts.map((shortcut) => shortcut.id);
    expect(new Set(ids).size).toBe(ids.length);
    const bindings = editorShortcuts.flatMap((shortcut) =>
      shortcut.bindings.map((binding) => `${shortcut.scope}:${binding}`),
    );
    expect(new Set(bindings).size).toBe(bindings.length);
  });

  it("preserves the existing timeline and preview shortcuts", () => {
    expect(shortcutById("timeline.selectTool").bindings).toEqual(["V"]);
    expect(shortcutById("timeline.bladeTool").bindings).toEqual(["C"]);
    expect(shortcutById("timeline.split").bindings).toEqual(["S", "Mod+K"]);
    expect(shortcutById("timeline.markIn").bindings).toEqual(["I"]);
    expect(shortcutById("timeline.markOut").bindings).toEqual(["O"]);
    expect(shortcutById("timeline.delete").bindings).toEqual(["Delete", "Backspace"]);
    expect(shortcutById("timeline.rippleDelete").bindings).toEqual(["Shift+Delete", "Shift+Backspace"]);
    expect(shortcutById("timeline.trimStart").bindings).toEqual(["["]);
    expect(shortcutById("timeline.trimEnd").bindings).toEqual(["]"]);
    expect(shortcutById("timeline.nudgeLeft").bindings).toEqual(["Shift+ArrowLeft"]);
    expect(shortcutById("timeline.nudgeRight").bindings).toEqual(["Shift+ArrowRight"]);
    expect(shortcutById("timeline.moveTrackUp").bindings).toEqual(["Shift+ArrowUp"]);
    expect(shortcutById("timeline.moveTrackDown").bindings).toEqual(["Shift+ArrowDown"]);
    expect(shortcutById("timeline.selectForwardTrack").bindings).toEqual(["A"]);
    expect(shortcutById("timeline.selectForwardAll").bindings).toEqual(["Shift+A"]);
    expect(shortcutById("navigation.stepBack").bindings).toEqual(["ArrowLeft"]);
    expect(shortcutById("navigation.stepForward").bindings).toEqual(["ArrowRight"]);
    expect(shortcutById("playback.toggle").bindings).toEqual(["Space"]);
    expect(shortcutById("preview.stepBack").bindings).toEqual(["ArrowLeft"]);
    expect(shortcutById("preview.stepForward").bindings).toEqual(["ArrowRight"]);
  });

  it("adds global undo, redo, export, shortcuts, and tab bindings", () => {
    expect(shortcutById("editor.undo").bindings).toEqual(["Mod+Z"]);
    expect(shortcutById("editor.redo").bindings).toEqual(["Shift+Mod+Z"]);
    expect(shortcutById("editor.export").bindings).toEqual(["Mod+E"]);
    expect(shortcutById("editor.shortcuts").bindings).toEqual(["Mod+/"]);
    expect(shortcutById("editor.tab.ai").bindings).toEqual(["Mod+1"]);
    expect(shortcutById("editor.tab.effects").bindings).toEqual(["Mod+6"]);
  });

  it("matches events using the platform modifier", () => {
    const mac: ShortcutPlatform = "macos";
    const linux: ShortcutPlatform = "linux";
    expect(matchShortcut(keyEvent({ key: "z", metaKey: true }), "global", mac)?.id).toBe("editor.undo");
    expect(matchShortcut(keyEvent({ key: "z", ctrlKey: true }), "global", linux)?.id).toBe("editor.undo");
    expect(matchShortcut(keyEvent({ key: "z", ctrlKey: true }), "global", mac)).toBeNull();
    expect(matchShortcut(keyEvent({ key: "Z", metaKey: true, shiftKey: true }), "global", mac)?.id).toBe("editor.redo");
    expect(matchShortcut(keyEvent({ key: "s" }), "timeline", linux)?.id).toBe("timeline.split");
    expect(matchShortcut(keyEvent({ key: " " }), "timeline", linux)?.id).toBe("playback.toggle");
    expect(matchShortcut(keyEvent({ key: "ArrowLeft" }), "timeline", linux)?.id).toBe("navigation.stepBack");
    expect(matchShortcut(keyEvent({ key: "ArrowLeft", shiftKey: true }), "timeline", linux)?.id).toBe(
      "timeline.nudgeLeft",
    );
  });

  it("formats bindings per platform", () => {
    expect(formatShortcut("Shift+Mod+Z", "macos")).toBe("⇧⌘Z");
    expect(formatShortcut("Shift+Mod+Z", "linux")).toBe("Ctrl+Shift+Z");
    expect(formatShortcut("Space", "linux")).toBe("Space");
  });
});
