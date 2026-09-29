import { useState } from "react";
import { BackgroundsView } from "./backgrounds-view";
import { EffectsView } from "./effects-view";
import { FilterChips, type FilterChipOption } from "./filter-chips";
import { TransitionsView } from "./transitions-view";

type EffectsPanelView = "effects" | "transitions" | "backgrounds";

const views: readonly FilterChipOption<EffectsPanelView>[] = [
  { value: "effects", label: "Effects" },
  { value: "transitions", label: "Transitions" },
  { value: "backgrounds", label: "Backgrounds" },
];

function EffectsPanelBody({ view }: { readonly view: EffectsPanelView }) {
  switch (view) {
    case "effects":
      return <EffectsView />;
    case "transitions":
      return <TransitionsView />;
    case "backgrounds":
      return <BackgroundsView />;
  }
}

/** The Effects tab: filter chips over the effect catalog, transitions and the shader backgrounds. */
export function EffectsPanel() {
  const [view, setView] = useState<EffectsPanelView>("effects");
  return (
    <div className="flex flex-col gap-3 p-3">
      <FilterChips<EffectsPanelView> label="Effects tab content" value={view} options={views} onChange={setView} />
      <EffectsPanelBody view={view} />
    </div>
  );
}
