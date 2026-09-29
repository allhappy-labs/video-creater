import type { EditorTabId } from "../store/persisted-layout";
import { AiPanel } from "./ai/ai-panel";
import { AudioPanel } from "./audio/audio-panel";
import { CaptionsPanel } from "./captions/captions-panel";
import { EffectsPanel } from "./effects/effects-panel";
import { MediaPanel } from "./media/media-panel";
import { TextPanel } from "./text/text-panel";

/** Renders the content for one left-panel tab (desktop tab body and mobile sheet body). */
export function EditorTabPanel({ tab }: { tab: EditorTabId }) {
  switch (tab) {
    case "ai":
      return <AiPanel />;
    case "media":
      return <MediaPanel />;
    case "audio":
      return <AudioPanel />;
    case "text":
      return <TextPanel />;
    case "captions":
      return <CaptionsPanel />;
    case "effects":
      return <EffectsPanel />;
  }
}
