import {
  ChevronLeft,
  ChevronRight,
  Diamond,
  FoldHorizontal,
  Link,
  Magnet,
  MousePointer2,
  Scan,
  Scissors,
  SquareSplitHorizontal,
  Trash2,
  Unlink,
  ZoomIn,
  ZoomOut,
  type LucideIcon,
} from "lucide-react";
import { IconButton } from "@/components/ui/icon-button";
import { Slider } from "@/components/ui/slider";
import { cn } from "@/lib/utils";
import { labelWithShortcut, useShortcutPlatform } from "../shell/use-shortcut-platform";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import { clamp, zoomBounds } from "../store/persisted-layout";
import { usePlayheadAvailability, useSelectionAvailability } from "./timeline-availability";
import { useTimelineCommands } from "./timeline-commands";
import { TimelineSelector } from "./timeline-selector";
import { fitZoom } from "./use-timeline-geometry";

const zoomStepFactor = 1.25;
/** The slider is logarithmic: position 0–100 spans the zoom bounds, with 100% in the middle. */
const sliderSteps = 100;
const zoomRatio = zoomBounds.max / zoomBounds.min;

function zoomToPosition(zoomPercent: number): number {
  return (sliderSteps * Math.log(clamp(zoomPercent, zoomBounds.min, zoomBounds.max) / zoomBounds.min)) / Math.log(zoomRatio);
}

function positionToZoom(position: number): number {
  return Math.round(zoomBounds.min * zoomRatio ** (position / sliderSteps));
}

interface ToolButtonProps {
  readonly label: string;
  readonly icon: LucideIcon;
  readonly onClick: () => void;
  readonly shortcutId?: string;
  /** Planner copy shown in the tooltip; the button is inert while set. */
  readonly reason?: string | null;
  /** Toggle state; renders `aria-pressed`. */
  readonly pressed?: boolean;
  readonly pressedClassName?: string;
}

/**
 * An icon button whose tooltip reads "<label> (<shortcut>)". A blocked command stays focusable
 * with `aria-disabled` so hovering or focusing it still shows why it is unavailable.
 */
function ToolButton({ label, icon: Icon, onClick, shortcutId, reason = null, pressed, pressedClassName }: ToolButtonProps) {
  const platform = useShortcutPlatform();
  const blocked = reason !== null;
  return (
    <IconButton
      size="sm"
      label={label}
      tooltip={blocked ? reason : labelWithShortcut(label, shortcutId, platform)}
      aria-disabled={blocked || undefined}
      aria-pressed={pressed}
      active={pressed === true}
      className={cn(
        "h-7 w-7 aria-disabled:cursor-default aria-disabled:text-dim aria-disabled:opacity-60 aria-disabled:hover:bg-transparent",
        pressed && pressedClassName,
      )}
      onClick={() => {
        if (!blocked) onClick();
      }}
    >
      <Icon className="h-4 w-4" aria-hidden />
    </IconButton>
  );
}

function ToolbarSeparator() {
  return <div role="separator" aria-orientation="vertical" className="mx-1 h-4 w-px shrink-0 bg-line" />;
}

interface TimelineToolbarProps {
  /** Width of the lanes viewport, used by Fit. */
  readonly viewportWidth: number;
}

/** One row: timeline selector, tools, clip edits, then snap, keyframes and zoom on the right. */
export function TimelineToolbar({ viewportWidth }: TimelineToolbarProps) {
  const store = useEditorStoreApi();
  const commands = useTimelineCommands();
  const tool = useEditorStore((state) => state.tool);
  const setTool = useEditorStore((state) => state.setTool);
  const snapEnabled = useEditorStore((state) => state.snapEnabled);
  const setSnapEnabled = useEditorStore((state) => state.setSnapEnabled);
  const keyframesVisible = useEditorStore((state) => state.keyframesVisible);
  const setKeyframesVisible = useEditorStore((state) => state.setKeyframesVisible);
  const zoomPercent = useEditorStore((state) => state.zoomPercent);
  const setZoomPercent = useEditorStore((state) => state.setZoomPercent);
  const selection = useSelectionAvailability();
  const playhead = usePlayheadAvailability();
  const unlink = selection.linkMode === "unlink";

  function fit() {
    const state = store.getState();
    setZoomPercent(fitZoom(state.project.timeline.durationSeconds, viewportWidth));
    state.setScrollLeft(0);
  }

  return (
    <div role="toolbar" aria-label="Timeline tools" className="flex h-10 shrink-0 items-center gap-0.5 overflow-x-auto border-b border-line px-2 [scrollbar-width:none]">
      <TimelineSelector />
      <ToolbarSeparator />
      <div role="group" aria-label="Tool" className="flex items-center gap-0.5">
        <ToolButton label="Select" icon={MousePointer2} shortcutId="timeline.selectTool" pressed={tool === "select"} onClick={() => setTool("select")} />
        <ToolButton label="Blade" icon={Scissors} shortcutId="timeline.bladeTool" pressed={tool === "blade"} onClick={() => setTool("blade")} />
      </div>
      <ToolbarSeparator />
      <ToolButton label="Split" icon={SquareSplitHorizontal} shortcutId="timeline.split" reason={playhead.split} onClick={() => void commands.splitAtPlayhead()} />
      <ToolButton label="Delete" icon={Trash2} shortcutId="timeline.delete" reason={selection.delete} onClick={() => void commands.deleteSelection()} />
      <ToolButton
        label="Ripple delete"
        icon={FoldHorizontal}
        shortcutId="timeline.rippleDelete"
        reason={selection.rippleDelete}
        onClick={() => void commands.rippleDeleteSelection()}
      />
      <ToolButton
        label={unlink ? "Unlink" : "Link"}
        icon={unlink ? Unlink : Link}
        reason={selection.link}
        onClick={() => void (unlink ? commands.unlinkSelection() : commands.linkSelection())}
      />
      <ToolButton label="Nudge left" icon={ChevronLeft} shortcutId="timeline.nudgeLeft" reason={selection.nudgeLeft} onClick={() => void commands.nudgeSelection(-1)} />
      <ToolButton label="Nudge right" icon={ChevronRight} shortcutId="timeline.nudgeRight" reason={selection.nudgeRight} onClick={() => void commands.nudgeSelection(1)} />
      <div className="min-w-2 flex-1" />
      <ToolButton label="Snap" icon={Magnet} pressed={snapEnabled} pressedClassName="text-primary" onClick={() => setSnapEnabled(!snapEnabled)} />
      <ToolButton
        label="Keyframes"
        icon={Diamond}
        pressed={keyframesVisible}
        pressedClassName="text-keyframe"
        onClick={() => setKeyframesVisible(!keyframesVisible)}
      />
      <ToolbarSeparator />
      <ToolButton
        label="Zoom out"
        icon={ZoomOut}
        reason={zoomPercent <= zoomBounds.min ? "Zoomed out all the way." : null}
        onClick={() => setZoomPercent(Math.round(zoomPercent / zoomStepFactor))}
      />
      <Slider
        label="Zoom"
        valueText={`${Math.round(zoomPercent).toString()}%`}
        min={0}
        max={sliderSteps}
        step={1}
        value={[zoomToPosition(zoomPercent)]}
        onValueChange={([position]) => {
          if (position !== undefined) setZoomPercent(positionToZoom(position));
        }}
        className="mx-1.5 w-24 shrink-0"
      />
      <ToolButton
        label="Zoom in"
        icon={ZoomIn}
        reason={zoomPercent >= zoomBounds.max ? "Zoomed in all the way." : null}
        onClick={() => setZoomPercent(Math.round(zoomPercent * zoomStepFactor))}
      />
      <ToolButton label="Fit" icon={Scan} onClick={fit} />
    </div>
  );
}
