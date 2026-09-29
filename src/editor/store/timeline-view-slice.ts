import type { ProjectActionKeyframeProperty } from "@/lib/project";
import type { EditorSliceCreator } from "./editor-store";
import {
  clamp,
  loadTimelineView,
  saveTimelineView,
  splitHeightBounds,
  zoomBounds,
  type TimelineTool,
} from "./persisted-layout";

export interface TimelineViewSlice {
  readonly splitHeight: number;
  readonly zoomPercent: number;
  readonly snapEnabled: boolean;
  readonly keyframesVisible: boolean;
  readonly tool: TimelineTool;
  /** Horizontal scroll of the tracks area in content pixels; session-only. */
  readonly scrollLeft: number;
  /** Keyframe lane property for the selected clip. */
  readonly laneProperty: ProjectActionKeyframeProperty;
  setSplitHeight(height: number): void;
  setZoomPercent(percent: number): void;
  setSnapEnabled(enabled: boolean): void;
  setKeyframesVisible(visible: boolean): void;
  setTool(tool: TimelineTool): void;
  setScrollLeft(scrollLeft: number): void;
  setLaneProperty(property: ProjectActionKeyframeProperty): void;
}

/**
 * The project directory is passed in explicitly: zustand's `get()` returns undefined while the
 * initializer runs, so the slice cannot read `projectDir` from the store during creation.
 */
export function createTimelineViewSlice(projectDir: string): EditorSliceCreator<TimelineViewSlice> {
  return (set, get) => {
    const view = loadTimelineView(projectDir);
    const persist = () => {
      const state = get();
      saveTimelineView(projectDir, {
        splitHeight: state.splitHeight,
        zoomPercent: state.zoomPercent,
        snapEnabled: state.snapEnabled,
        keyframesVisible: state.keyframesVisible,
        tool: state.tool,
        laneProperty: state.laneProperty,
      });
    };
    return {
      ...view,
      scrollLeft: 0,
      setSplitHeight: (height) => {
        set({ splitHeight: Math.max(splitHeightBounds.min, Number.isFinite(height) ? Math.round(height) : 0) });
        persist();
      },
      setZoomPercent: (percent) => {
        set({ zoomPercent: clamp(percent, zoomBounds.min, zoomBounds.max) });
        persist();
      },
      setSnapEnabled: (enabled) => {
        set({ snapEnabled: enabled });
        persist();
      },
      setKeyframesVisible: (visible) => {
        set({ keyframesVisible: visible });
        persist();
      },
      setTool: (tool) => {
        set({ tool });
        persist();
      },
      setScrollLeft: (scrollLeft) => set({ scrollLeft: Number.isFinite(scrollLeft) ? Math.max(0, scrollLeft) : 0 }),
      setLaneProperty: (property) => {
        set({ laneProperty: property });
        persist();
      },
    };
  };
}
