import { cn } from "@/lib/utils";
import { PropertiesPanel } from "../properties/properties-panel";

/** Properties width in both desktop modes; the overlay also insets the preview content by it. */
export const propertiesPanelWidth = 330;

export function PropertiesRegion({ overlay }: { overlay: boolean }) {
  return (
    <aside
      aria-label="Properties"
      style={{ width: propertiesPanelWidth }}
      className={cn(
        "flex min-h-0 shrink-0 flex-col overflow-hidden rounded-panel bg-panel",
        overlay && "absolute bottom-0 right-0 top-0 z-20 shadow-2xl",
      )}
    >
      <PropertiesPanel />
    </aside>
  );
}
