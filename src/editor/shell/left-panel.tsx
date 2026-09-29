import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { EditorTabPanel } from "../panels/editor-tab-panel";
import { useEditorStore } from "../store/editor-store-context";
import type { EditorTabId } from "../store/persisted-layout";
import { editorTabs } from "./editor-tabs";

export function LeftPanel() {
  const activeTab = useEditorStore((state) => state.activeTab);
  const setActiveTab = useEditorStore((state) => state.setActiveTab);

  return (
    <Tabs
      value={activeTab}
      onValueChange={(value) => setActiveTab(value as EditorTabId)}
      className="flex h-full min-h-0 flex-col overflow-hidden rounded-panel bg-panel"
    >
      <TabsList aria-label="Editor tools" className="gap-0.5 px-1.5 pt-1.5">
        {editorTabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            className="group flex flex-1 flex-col items-center gap-1 rounded-control py-2 text-[11.5px] text-muted-foreground data-[state=active]:bg-raised"
          >
            <tab.icon className="h-[19px] w-[19px] group-data-[state=active]:text-primary" aria-hidden />
            {tab.label}
          </TabsTrigger>
        ))}
      </TabsList>
      {editorTabs.map((tab) => (
        <TabsContent key={tab.id} value={tab.id} aria-label={tab.label} className="overflow-y-auto">
          <EditorTabPanel tab={tab.id} />
        </TabsContent>
      ))}
    </Tabs>
  );
}
