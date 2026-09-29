import { formatShortcut, shortcutById, type ShortcutPlatform } from "@/lib/keymap";
import { useHostPlatform } from "@/lib/runtime/platform";

/** The keymap platform for the host: ⌘ on macOS, Ctrl everywhere else. */
export function useShortcutPlatform(): ShortcutPlatform {
  const host = useHostPlatform();
  return host === "macos" ? "macos" : host === "windows" ? "windows" : "linux";
}

/** The first binding of a registry shortcut, formatted for the platform ("⌘K", "Ctrl+K"). */
export function shortcutHint(shortcutId: string, platform: ShortcutPlatform): string {
  return formatShortcut(shortcutById(shortcutId).bindings[0] ?? "", platform);
}

/** Tooltip copy for a control: "<label> (<shortcut>)", or just the label without a shortcut. */
export function labelWithShortcut(label: string, shortcutId: string | undefined, platform: ShortcutPlatform): string {
  return shortcutId ? `${label} (${shortcutHint(shortcutId, platform)})` : label;
}
