import { speedActions, visualPropertyRanges, visualSpeed } from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import { setItemReverse } from "@/lib/timeline-ops/clip-commands";
import { isReversedItem, isReversibleItem } from "@/lib/timeline-ops/reverse";
import { useEditorStore } from "../store/editor-store-context";
import { PresetGrid } from "./controls/preset-grid";
import { PropertySection } from "./controls/property-section";
import { SliderField } from "./controls/slider-field";
import { SwitchField } from "./controls/switch-field";
import { formatMultiplier } from "./formatters";
import { usePropertyCommit } from "./use-property-commit";

const speedPresets = [
  { value: "0.5", label: "0.5×" },
  { value: "1", label: "1×" },
  { value: "1.5", label: "1.5×" },
  { value: "2", label: "2×" },
] as const;

type SpeedPreset = (typeof speedPresets)[number]["value"];

/**
 * Constant speed for visual and audio clips through the timeline speed command: the clip's speed
 * action (`updateVisualClipSpeed` or `updateAudioClipSpeed`) and its linked partners', plus the
 * rescaled `resizeItems`, in one batch. Video clips of video media and audio clips also get a
 * Reverse switch (`updateClipReverse` for the clip and its linked partners, one batch); on a
 * locked track it is disabled with the reason.
 */
export function ClipSpeedTab({ item }: { readonly item: TimelineItem }) {
  const project = useEditorStore((state) => state.project);
  const { commit } = usePropertyCommit();
  const speed = visualSpeed(item);
  const range = visualPropertyRanges.speed;
  const preset = speedPresets.find((option) => Number(option.value) === speed)?.value ?? null;
  const apply = (next: number) => commit(speedActions(project, item.id, next));
  const reversed = isReversedItem(item);
  const reverseResult = setItemReverse(project, item.id, !reversed);
  const reverseBlocked = "blocked" in reverseResult ? reverseResult.blocked : null;
  return (
    <PropertySection title="Speed" onReset={() => apply(1)} resetDisabled={speed === 1}>
      <SliderField label="Speed" value={speed} min={range.min} max={range.max} step={range.step} format={formatMultiplier} onCommit={apply} />
      <PresetGrid<SpeedPreset>
        label="Speed presets"
        columns={4}
        value={preset}
        options={speedPresets}
        onChange={(value) => void apply(Number(value))}
      />
      {isReversibleItem(project, item) && (
        <SwitchField
          label="Reverse"
          checked={reversed}
          disabled={reverseBlocked !== null}
          {...(reverseBlocked === null ? {} : { description: reverseBlocked })}
          onChange={() => void commit(reverseResult)}
        />
      )}
    </PropertySection>
  );
}
