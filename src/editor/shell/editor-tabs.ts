import { Captions, Image, Music, Sparkles, Type, WandSparkles, type LucideIcon } from "lucide-react";
import type { EditorTabId } from "../store/persisted-layout";

interface EditorTabDefinition {
  readonly id: EditorTabId;
  readonly label: string;
  readonly icon: LucideIcon;
  readonly shortcutId: string;
}

export const editorTabs: readonly EditorTabDefinition[] = [
  { id: "ai", label: "AI", icon: Sparkles, shortcutId: "editor.tab.ai" },
  { id: "media", label: "Media", icon: Image, shortcutId: "editor.tab.media" },
  { id: "audio", label: "Audio", icon: Music, shortcutId: "editor.tab.audio" },
  { id: "text", label: "Text", icon: Type, shortcutId: "editor.tab.text" },
  { id: "captions", label: "Captions", icon: Captions, shortcutId: "editor.tab.captions" },
  { id: "effects", label: "Effects", icon: WandSparkles, shortcutId: "editor.tab.effects" },
];
