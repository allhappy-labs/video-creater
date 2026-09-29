import { useMemo } from "react";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { selectedTimelineItems, type SelectionKind } from "@/lib/preview/selection-kind";
import type { TimelineItem } from "@/lib/timeline";
import { useEditorStore } from "../store/editor-store-context";
import { AiTabBody } from "./ai-tab";
import { AudioTabBody } from "./audio-tabs";
import { CaptionScopeProvider, CaptionTabBody } from "./caption-tabs";
import { MultipleTabBody } from "./multiple-tabs";
import { propertiesSelection, type PropertyTab } from "./property-tabs";
import { TemplateTabBody } from "./template-tabs";
import { TextTabBody } from "./text-tabs";
import { TransitionTabBody } from "./transition-tabs";
import { VisualTabBody } from "./visual-tabs";

interface PropertyTabBodyProps {
  readonly tab: PropertyTab;
  readonly kind: SelectionKind;
  readonly items: readonly TimelineItem[];
}

/** Routes a tab to its contents; the mobile property sheets reuse it. */
export function PropertyTabBody({ tab, kind, items }: PropertyTabBodyProps) {
  const [only] = items;
  if (kind === "transition") return <TransitionTabBody />;
  if (kind === "multiple") return <MultipleTabBody items={items} />;
  if (only && tab.id === "ai") return <AiTabBody item={only} />;
  if (only && kind === "visual") return <VisualTabBody tab={tab} item={only} />;
  if (only && kind === "audio") return <AudioTabBody tab={tab} item={only} />;
  if (only && kind === "text") return <TextTabBody tab={tab} item={only} />;
  if (only && kind === "caption") return <CaptionTabBody tab={tab} item={only} />;
  if (only && kind === "template") return <TemplateTabBody tab={tab} item={only} />;
  return null;
}

/** Selection header and contextual tabs; renders nothing when the selection has no tab set. */
export function PropertiesPanel() {
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const selectedTransitionId = useEditorStore((state) => state.selectedTransitionId);
  const tabByKind = useEditorStore((state) => state.propertiesTabByKind);
  const setPropertiesTab = useEditorStore((state) => state.setPropertiesTab);
  const selection = useMemo(
    () => propertiesSelection(project, selectedItemIds, selectedTransitionId),
    [project, selectedItemIds, selectedTransitionId],
  );
  const items = useMemo(() => selectedTimelineItems(project, selectedItemIds), [project, selectedItemIds]);
  const firstTab = selection?.tabs[0];
  if (!selection || !firstTab) return null;

  const { kind, name, tabs } = selection;
  const remembered = tabByKind[kind];
  const activeTab = tabs.find((tab) => tab.id === remembered)?.id ?? firstTab.id;
  const tabSet = (
    <Tabs value={activeTab} onValueChange={(tab) => setPropertiesTab(kind, tab)} className="flex min-h-0 flex-1 flex-col">
      <TabsList aria-label="Property tabs" className="shrink-0 gap-4 border-b border-line px-3">
        {tabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            className="-mb-px h-9 border-b-2 border-transparent text-[13px] text-muted-foreground transition-colors hover:text-foreground data-[state=active]:border-primary"
          >
            {tab.label}
          </TabsTrigger>
        ))}
      </TabsList>
      {tabs.map((tab) => (
        <TabsContent key={tab.id} value={tab.id} className="overflow-y-auto overscroll-contain">
          <PropertyTabBody tab={tab} kind={kind} items={items} />
        </TabsContent>
      ))}
    </Tabs>
  );

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex h-10 shrink-0 items-center px-3 pt-1">
        <h2 className="truncate text-[13px] font-semibold text-foreground" title={name}>
          {name}
        </h2>
      </header>
      {kind === "caption" ? (
        // "Applies to" starts at All captions for each caption selection.
        <CaptionScopeProvider key={items.map((item) => item.id).join(" ")}>{tabSet}</CaptionScopeProvider>
      ) : (
        tabSet
      )}
    </div>
  );
}
