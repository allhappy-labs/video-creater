import { commonPropertySupport } from "@/lib/preview/selection-kind";
import { audioPropertyRanges, audioVolumeDb, volumeActions } from "@/lib/properties/audio-properties";
import { opacityActions, visualOpacity, visualPropertyRanges } from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import { useEditorStore } from "../store/editor-store-context";
import { PropertySection } from "./controls/property-section";
import { SliderField } from "./controls/slider-field";
import { formatDecibels, formatPercent, fromPercent, toPercent } from "./formatters";
import { combineResults, usePropertyCommit } from "./use-property-commit";
import { EffectsSection } from "./visual-look-effects";

interface ItemsProps {
  readonly items: readonly TimelineItem[];
}

function allSame(values: readonly number[]): boolean {
  return values.every((value) => value === values[0]);
}

function CommonOpacity({ items }: ItemsProps) {
  const { commit } = usePropertyCommit();
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const values = items.map(visualOpacity);
  const range = visualPropertyRanges.opacity;
  return (
    <PropertySection title="Opacity">
      <SliderField
        label="Opacity"
        value={toPercent(values[0] ?? 1)}
        mixed={!allSame(values)}
        min={toPercent(range.min)}
        max={toPercent(range.max)}
        step={toPercent(range.step)}
        format={formatPercent}
        onCommit={(value) => commit(combineResults(items.map((item) => opacityActions(item, fromPercent(value), playheadSeconds))))}
      />
    </PropertySection>
  );
}

function CommonVolume({ items }: ItemsProps) {
  const { commit } = usePropertyCommit();
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const values = items.map(audioVolumeDb);
  const range = audioPropertyRanges.volumeDb;
  const apply = (volumeDb: number | null) =>
    commit(combineResults(items.map((item) => volumeActions(item, volumeDb, playheadSeconds))));
  return (
    <PropertySection title="Volume">
      <SliderField
        label="Volume"
        value={values[0] ?? 0}
        mixed={!allSame(values)}
        min={range.min}
        max={range.max}
        step={range.step}
        format={formatDecibels}
        onBlank={() => apply(null)}
        onCommit={apply}
      />
    </PropertySection>
  );
}

/** Common properties of several items: each change is one batch for all of them. */
export function MultipleTabBody({ items }: ItemsProps) {
  const project = useEditorStore((state) => state.project);
  const support = commonPropertySupport(
    project,
    items.map((item) => item.id),
  );
  if (!support.opacity && !support.volume && !support.effects) {
    return <p className="px-3 py-6 text-center text-[12px] text-dim">These items share no editable properties.</p>;
  }
  return (
    <>
      {support.opacity && <CommonOpacity items={items} />}
      {support.volume && <CommonVolume items={items} />}
      {support.effects && <EffectsSection items={items} />}
    </>
  );
}
