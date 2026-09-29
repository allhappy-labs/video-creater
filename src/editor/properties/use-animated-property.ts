import { keyframeStateAtPlayhead, toggleKeyframeAction, type AnimatableProperty } from "@/lib/properties/keyframe-actions";
import type { TimelineItem } from "@/lib/timeline";
import { useEditorStore } from "../store/editor-store-context";
import type { KeyframeToggle } from "./controls/keyframe-button";
import { usePropertyCommit } from "./use-property-commit";

interface AnimatedProperty {
  readonly playheadSeconds: number;
  /** The value at the playhead: interpolated when keyframed, else the static value. */
  readonly value: number;
  readonly keyframe: KeyframeToggle;
}

/** ◇ state and the shown value for one animatable property of `item` at the timeline playhead. */
export function useAnimatedProperty(item: TimelineItem, property: AnimatableProperty, staticValue: number): AnimatedProperty {
  const playheadSeconds = useEditorStore((state) => state.playheadSeconds);
  const { commit } = usePropertyCommit();
  const state = keyframeStateAtPlayhead(item, property, playheadSeconds);
  const value = state.keyframed ? state.value : staticValue;
  return {
    playheadSeconds,
    value,
    keyframe: {
      active: state.atPlayhead !== null,
      keyframed: state.keyframed,
      onToggle: () => void commit([toggleKeyframeAction(item, property, playheadSeconds, value)]),
    },
  };
}
