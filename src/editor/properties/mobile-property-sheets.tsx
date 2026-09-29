import { Blend, FileText, Gauge, Palette, SlidersHorizontal, Sparkle, Sparkles, Timer, Type, Volume2, WandSparkles, type LucideIcon } from "lucide-react";
import { useEffect, useMemo } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { selectedTimelineItems, type SelectionKind } from "@/lib/preview/selection-kind";
import type { TimelineItem } from "@/lib/timeline";
import { BottomSheet } from "../shell/bottom-sheet";
import { useEditorStore, useEditorStoreApi } from "../store/editor-store-context";
import type { PropertyToolId } from "../store/ui-slice";
import { CaptionScopeProvider } from "./caption-tabs";
import { PropertyTabBody } from "./properties-panel";
import { propertiesSelection, type PropertiesSelection, type PropertyTab, type PropertyTabId } from "./property-tabs";
import { TransitionTabBody } from "./transition-tabs";
import { EffectsSection } from "./visual-look-effects";
import { BlendSection, CropSection, FadeSection, TransformSection } from "./visual-video-tab";

export interface MobilePropertyTool {
  readonly id: PropertyToolId;
  readonly label: string;
  readonly icon: LucideIcon;
  /** The Properties tab whose contents the sheet shows. */
  readonly tab: PropertyTab;
}

const toolAppearance: Readonly<Record<PropertyToolId, { readonly label: string; readonly icon: LucideIcon }>> = {
  speed: { label: "Speed", icon: Gauge },
  volume: { label: "Volume", icon: Volume2 },
  animation: { label: "Animation", icon: Sparkle },
  effects: { label: "Effects", icon: WandSparkles },
  adjust: { label: "Adjust", icon: SlidersHorizontal },
  text: { label: "Text", icon: Type },
  style: { label: "Style", icon: Palette },
  content: { label: "Content", icon: FileText },
  transitionType: { label: "Type", icon: Blend },
  transitionDuration: { label: "Duration", icon: Timer },
  ai: { label: "AI", icon: Sparkles },
};

type ToolTabs = readonly (readonly [PropertyToolId, PropertyTabId])[];

const textTools: ToolTabs = [
  ["text", "text"],
  ["style", "style"],
  ["animation", "animation"],
];

/**
 * Clip tools per selection kind, in bar order, with the tab each sheet shows. A tool appears only
 * when the selection has that tab, so Volume needs a clip with audio, and AI shows for visual clips
 * and generated audio clips. Speed's sheet holds the Reverse switch for video and audio clips.
 * On visual clips Adjust shows the Video tab's Transform, Blend, Crop and Fade sections and
 * Effects its Effects section.
 */
const toolTabsByKind: Readonly<Partial<Record<SelectionKind, ToolTabs>>> = {
  visual: [
    ["speed", "speed"],
    ["volume", "audio"],
    ["animation", "animation"],
    ["effects", "video"],
    ["adjust", "video"],
    ["ai", "ai"],
  ],
  audio: [
    ["speed", "speed"],
    ["volume", "basic"],
    ["ai", "ai"],
  ],
  text: textTools,
  caption: textTools,
  template: [
    ["content", "content"],
    ["style", "style"],
    ["animation", "animation"],
    ["effects", "effects"],
  ],
  multiple: [["adjust", "common"]],
  transition: [
    ["transitionType", "transition"],
    ["transitionDuration", "transition"],
  ],
};

const sheetPrefix = "property:";

/** The store sheet id of a clip tool's sheet. */
export function propertySheetId(tool: PropertyToolId): `property:${PropertyToolId}` {
  return `${sheetPrefix}${tool}`;
}

/** The Properties sheets the mobile clip tools offer for a selection. */
export function mobilePropertyTools(selection: PropertiesSelection | null): readonly MobilePropertyTool[] {
  if (!selection) return [];
  return (toolTabsByKind[selection.kind] ?? []).flatMap(([id, tabId]) => {
    const tab = selection.tabs.find((candidate) => candidate.id === tabId);
    return tab ? [{ id, tab, ...toolAppearance[id] }] : [];
  });
}

/** The selection's Properties header, tabs and items, memoized on the project and selection. */
export function usePropertiesSelection() {
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const selectedTransitionId = useEditorStore((state) => state.selectedTransitionId);
  return useMemo(
    () => ({
      selection: propertiesSelection(project, selectedItemIds, selectedTransitionId),
      items: selectedTimelineItems(project, selectedItemIds),
    }),
    [project, selectedItemIds, selectedTransitionId],
  );
}

const adjustSections = [
  { id: "transform", label: "Transform", Section: TransformSection },
  { id: "blend", label: "Blend", Section: BlendSection },
  { id: "crop", label: "Crop", Section: CropSection },
  { id: "fade", label: "Fade", Section: FadeSection },
] as const;

/** Adjust on a visual clip: the Video tab's Transform, Blend, Crop and Fade sections, one at a time. */
function AdjustSheetBody({ item }: { readonly item: TimelineItem }) {
  return (
    <Tabs defaultValue="transform" className="flex flex-col">
      <TabsList aria-label="Adjust sections" className="gap-1 px-3 pb-1">
        {adjustSections.map(({ id, label }) => (
          <TabsTrigger
            key={id}
            value={id}
            className="h-8 rounded-control px-3 text-[13px] text-muted-foreground transition-colors hover:text-foreground data-[state=active]:bg-raised motion-reduce:transition-none"
          >
            {label}
          </TabsTrigger>
        ))}
      </TabsList>
      {adjustSections.map(({ id, Section }) => (
        <TabsContent key={id} value={id}>
          <Section item={item} />
        </TabsContent>
      ))}
    </Tabs>
  );
}

function PropertySheetBody({ tool, selection, items }: { readonly tool: MobilePropertyTool; readonly selection: PropertiesSelection; readonly items: readonly TimelineItem[] }) {
  const [only] = items;
  if (selection.kind === "transition") return <TransitionTabBody sections={[tool.id === "transitionType" ? "type" : "duration"]} />;
  if (only && selection.kind === "visual" && tool.id === "adjust") return <AdjustSheetBody item={only} />;
  if (only && selection.kind === "visual" && tool.id === "effects") return <EffectsSection items={[only]} />;
  const body = <PropertyTabBody tab={tool.tab} kind={selection.kind} items={items} />;
  // "Applies to" starts at All captions for each caption selection, as in the panel.
  return selection.kind === "caption" ? <CaptionScopeProvider key={items.map((item) => item.id).join(" ")}>{body}</CaptionScopeProvider> : body;
}

/**
 * The open clip tool's Properties sheet above the clip tools: compact, or tall for AI. It is
 * non-modal, so the preview stays visible while values change and another tool can be tapped. It
 * closes when its tool leaves the selection or crop mode starts ("Edit on canvas" needs the canvas).
 */
export function MobilePropertySheet() {
  const store = useEditorStoreApi();
  const openSheetId = useEditorStore((state) => state.openSheetId);
  const closeSheet = useEditorStore((state) => state.closeSheet);
  const cropModeItemId = useEditorStore((state) => state.cropModeItemId);
  const { selection, items } = usePropertiesSelection();
  const toolId = openSheetId?.startsWith(sheetPrefix) ? openSheetId.slice(sheetPrefix.length) : null;
  const tool = toolId === null ? null : (mobilePropertyTools(selection).find((candidate) => candidate.id === toolId) ?? null);
  const stale = toolId !== null && tool === null;

  useEffect(() => {
    if (stale) closeSheet();
  }, [stale, closeSheet]);

  useEffect(() => {
    if (cropModeItemId !== null && store.getState().openSheetId?.startsWith(sheetPrefix)) store.getState().closeSheet();
  }, [cropModeItemId, store]);

  if (!tool || !selection) return null;
  return (
    // Keyed by tool: tapping another tool mounts a new sheet, so the old sheet's deferred touch
    // outside-press dismissal cannot close the sheet that tap opened.
    <BottomSheet key={tool.id} title={tool.label} open height={tool.id === "ai" ? "tall" : "compact"} modal={false} aboveToolBar onClose={closeSheet}>
      <PropertySheetBody tool={tool} selection={selection} items={items} />
    </BottomSheet>
  );
}
