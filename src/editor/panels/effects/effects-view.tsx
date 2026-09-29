import { Search } from "lucide-react";
import { useMemo, useState } from "react";
import type { VisualEffectDescriptor } from "@/lib/project";
import { useEffectCatalog } from "../../properties/use-effect-catalog";
import { useEditorStore } from "../../store/editor-store-context";
import { appliedEffectTypes, effectCategories, effectMatchesSearch, effectTarget, panelEffects } from "./effect-apply";
import { EffectTile } from "./effect-tile";
import { FilterChips } from "./filter-chips";
import { useApplyEffect } from "./use-apply-effect";

const allCategories = "";

/** Searchable, category-filtered effect tiles; `+` applies to the one selected visual clip. */
export function EffectsView() {
  const catalog = useEffectCatalog();
  const project = useEditorStore((state) => state.project);
  const selectedItemIds = useEditorStore((state) => state.selectedItemIds);
  const applyEffect = useApplyEffect();
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState(allCategories);

  const effects = useMemo(() => panelEffects(catalog.effects), [catalog.effects]);
  const categories = useMemo(() => effectCategories(effects), [effects]);
  const target = useMemo(() => effectTarget(project, selectedItemIds), [project, selectedItemIds]);
  const applied = "item" in target ? appliedEffectTypes(target.item) : new Set<string>();
  const blockedReason = "blocked" in target ? target.blocked : null;
  const activeCategory = categories.includes(category) ? category : allCategories;
  const visible = effects.filter(
    (effect) => (activeCategory === allCategories || effect.category === activeCategory) && effectMatchesSearch(effect, query),
  );

  function apply(effect: VisualEffectDescriptor) {
    if ("item" in target) void applyEffect(target.item.id, effect);
  }

  return (
    <div className="flex flex-col gap-2.5">
      <label className="relative block">
        <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-dim" aria-hidden />
        <input
          type="search"
          aria-label="Search effects"
          placeholder="Search effects"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          className="h-8 w-full rounded-control bg-raised pl-7 pr-2 text-[12px] text-foreground outline-none placeholder:text-dim focus-visible:ring-2 focus-visible:ring-ring"
        />
      </label>
      {categories.length > 1 && (
        <FilterChips
          label="Effect categories"
          size="sm"
          value={activeCategory}
          options={[{ value: allCategories, label: "All" }, ...categories.map((value) => ({ value, label: value }))]}
          onChange={setCategory}
        />
      )}
      {blockedReason !== null && effects.length > 0 && (
        <p role="status" className="text-[12px] text-muted-foreground">
          {blockedReason}.
        </p>
      )}
      {catalog.status === "loading" ? (
        <p className="text-[12px] text-dim">Loading effects…</p>
      ) : effects.length === 0 ? (
        <p className="text-[12px] text-dim">Effect catalog unavailable.</p>
      ) : visible.length === 0 ? (
        <p className="text-[12px] text-dim">No effects match this filter</p>
      ) : (
        <ul aria-label="Effects" className="grid grid-cols-2 gap-2">
          {visible.map((effect) => (
            <EffectTile key={effect.id} effect={effect} applied={applied.has(effect.id)} blockedReason={blockedReason} onApply={apply} />
          ))}
        </ul>
      )}
    </div>
  );
}
