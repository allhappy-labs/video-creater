import type { TimelineItem } from "@/lib/timeline";
import { useEditorStore } from "../store/editor-store-context";
import { useTimelineCommands } from "../timeline/timeline-commands";
import { AudioBasicSections, DenoiseSection } from "./audio-tabs";
import { linkedAudioItem } from "./linked-audio";
import type { PropertyTab } from "./property-tabs";
import { ClipSpeedTab } from "./speed-tab";
import { VisualAnimationTab } from "./visual-speed-animation";
import { VisualVideoTab } from "./visual-video-tab";

/**
 * Audio of a video clip. Its sound plays and exports from the linked audio clip, and the audio
 * actions accept audio clips only, so the controls edit that clip. Without one, Detach audio
 * creates it.
 */
function VisualAudioTab({ item }: { readonly item: TimelineItem }) {
  const audio = useEditorStore((state) => linkedAudioItem(state.project, item));
  const commands = useTimelineCommands();
  if (!audio) {
    return (
      <div className="flex flex-col items-center gap-2 px-3 py-6">
        <p className="text-center text-[12px] text-dim">This clip's sound isn't on its own audio clip yet.</p>
        <button
          type="button"
          onClick={() => void commands.detachAudio(item.id)}
          className="h-7 rounded-control bg-raised px-2.5 text-[12px] font-medium text-foreground transition-colors hover:bg-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          Detach audio
        </button>
      </div>
    );
  }
  return (
    <>
      <AudioBasicSections item={audio} />
      <DenoiseSection item={audio} />
    </>
  );
}

/** Video · Audio (only when the clip has audio) · Speed · Animation. */
export function VisualTabBody({ tab, item }: { readonly tab: PropertyTab; readonly item: TimelineItem }) {
  switch (tab.id) {
    case "audio":
      return <VisualAudioTab item={item} />;
    case "speed":
      return <ClipSpeedTab item={item} />;
    case "animation":
      return <VisualAnimationTab item={item} />;
    default:
      return <VisualVideoTab item={item} />;
  }
}
