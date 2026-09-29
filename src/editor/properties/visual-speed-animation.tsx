import type { MotionPresetId } from "@/lib/motion-presets";
import {
  animationPresetActions,
  animationPresets,
  appliedAnimationPreset,
  clearAnimationPresetActions,
  type AnimationPhase,
} from "@/lib/properties/animation-presets";
import type { AnimatableProperty } from "@/lib/properties/keyframe-actions";
import type { TimelineItem } from "@/lib/timeline";
import { inspectorKeyframesByProperty, visualKeyframePropertyConfigs } from "@/lib/timeline-ops/keyframes";
import { useEditorStoreApi } from "../store/editor-store-context";
import { PresetGrid } from "./controls/preset-grid";
import { PropertySection } from "./controls/property-section";
import { usePropertyCommit } from "./use-property-commit";

const phaseTitles: Readonly<Record<AnimationPhase, string>> = { in: "In", out: "Out", loop: "Loop" };

/** One phase's preset grid with "None"; text overlays reuse it for their in and out presets. */
export function AnimationPhaseSection({ item, phase }: { readonly item: TimelineItem; readonly phase: AnimationPhase }) {
  const { commit } = usePropertyCommit();
  const options = [
    { value: "none" as const, label: "None" },
    ...animationPresets(phase).map((preset) => ({ value: preset.id, label: preset.label })),
  ];
  return (
    <PropertySection title={phaseTitles[phase]}>
      <PresetGrid<"none" | MotionPresetId>
        label={`${phaseTitles[phase]} animation`}
        value={appliedAnimationPreset(item, phase) ?? "none"}
        options={options}
        onChange={(value) =>
          void commit(value === "none" ? clearAnimationPresetActions(item, phase) : animationPresetActions(item, value))
        }
      />
    </PropertySection>
  );
}

function KeyframeSummary({ item }: { readonly item: TimelineItem }) {
  const store = useEditorStoreApi();
  const lanes = inspectorKeyframesByProperty(item);
  const rows = visualKeyframePropertyConfigs.flatMap((config) => {
    const count = lanes[config.property]?.length ?? 0;
    return count > 0 ? [{ property: config.property as AnimatableProperty, label: config.label, count }] : [];
  });
  const showInTimeline = (property: AnimatableProperty) => {
    const state = store.getState();
    state.setLaneProperty(property);
    state.setKeyframesVisible(true);
  };
  return (
    <PropertySection title="Keyframes">
      {rows.length === 0 ? (
        <p className="text-[12px] text-dim">No keyframes yet. Use ◇ next to a property to add one.</p>
      ) : (
        <ul aria-label="Keyframed properties" className="flex flex-col gap-1">
          {rows.map((row) => (
            <li key={row.property} className="flex items-center gap-2">
              <span className="min-w-0 flex-1 truncate text-[12px] text-foreground">{row.label}</span>
              <span className="tabular-time text-[11px] text-dim">
                {row.count.toString()} {row.count === 1 ? "keyframe" : "keyframes"}
              </span>
              <button
                type="button"
                aria-label={`Show ${row.label} keyframes in timeline`}
                onClick={() => showInTimeline(row.property)}
                className="h-7 rounded-control px-2 text-[12px] text-muted-foreground transition-colors hover:bg-raised hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                Show in timeline
              </button>
            </li>
          ))}
        </ul>
      )}
    </PropertySection>
  );
}

/** In, out and loop preset grids plus the keyframed-property summary. */
export function VisualAnimationTab({ item }: { readonly item: TimelineItem }) {
  return (
    <>
      <AnimationPhaseSection item={item} phase="in" />
      <AnimationPhaseSection item={item} phase="out" />
      <AnimationPhaseSection item={item} phase="loop" />
      <KeyframeSummary item={item} />
    </>
  );
}
