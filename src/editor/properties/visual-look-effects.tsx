import { ChevronRight, Trash2 } from "lucide-react";
import { useId, useState } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { Switch } from "@/components/ui/switch";
import type { VisualEffectDescriptor } from "@/lib/project";
import {
  colorGrade,
  colorGradeAction,
  colorGradeRanges,
  resetColorGradeAction,
  type ColorGradeValues,
} from "@/lib/properties/visual-properties";
import type { TimelineItem } from "@/lib/timeline";
import { cn } from "@/lib/utils";
import { PresetGrid } from "./controls/preset-grid";
import { PropertySection } from "./controls/property-section";
import { SliderField } from "./controls/slider-field";
import { effectParamControls, effectParamValue, effectRowActions, effectRows, withEffectParam, type EffectRow } from "./effect-rows";
import { formatNumber } from "./formatters";
import { lookPresetAction, lookPresetForGrade, lookPresets } from "./look-presets";
import { useEffectCatalog } from "./use-effect-catalog";
import { usePropertyCommit } from "./use-property-commit";

const gradeFields: readonly { readonly key: keyof ColorGradeValues; readonly label: string }[] = [
  { key: "exposure", label: "Exposure" },
  { key: "contrast", label: "Contrast" },
  { key: "saturation", label: "Saturation" },
  { key: "temperature", label: "Temperature (K)" },
];

/** Look presets plus a "Color" disclosure with the grade sliders. */
export function LookSection({ item }: { readonly item: TimelineItem }) {
  const { commit, preview } = usePropertyCommit();
  const [colorOpen, setColorOpen] = useState(false);
  const colorId = useId();
  const grade = colorGrade(item);
  return (
    <PropertySection title="Look" onReset={() => commit([resetColorGradeAction([item.id])])}>
      <PresetGrid
        label="Look preset"
        columns={4}
        value={lookPresetForGrade(grade)}
        options={lookPresets}
        onChange={(preset) => void commit([lookPresetAction([item.id], preset)])}
      />
      <button
        type="button"
        aria-expanded={colorOpen}
        aria-controls={colorId}
        onClick={() => setColorOpen((open) => !open)}
        className="flex h-7 items-center gap-1 self-start rounded-md pr-2 text-[12px] font-medium text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <ChevronRight className={cn("h-3.5 w-3.5 transition-transform motion-reduce:transition-none", colorOpen && "rotate-90")} aria-hidden />
        Color
      </button>
      {colorOpen && (
        <div id={colorId} className="flex flex-col gap-2.5">
          {gradeFields.map(({ key, label }) => {
            const range = colorGradeRanges[key];
            return (
              <SliderField
                key={key}
                label={label}
                value={grade[key]}
                min={range.min}
                max={range.max}
                step={range.step}
                format={formatNumber}
                onPreview={(value) => preview(item.id, { colorGrade: { ...grade, [key]: value } })}
                onCommit={(value) => commit(colorGradeAction([item.id], { [key]: value }))}
              />
            );
          })}
        </div>
      )}
    </PropertySection>
  );
}

function sameValues(values: readonly number[]): boolean {
  return values.every((value) => value === values[0]);
}

function ResourceField({ row, resourceKey }: { readonly row: EffectRow; readonly resourceKey: string }) {
  const { commit } = usePropertyCommit();
  const inputId = useId();
  const values = row.targets.map(({ effect }) => {
    const value = effect.params[resourceKey];
    return typeof value === "string" ? value : "";
  });
  const mixed = !values.every((value) => value === values[0]);
  const current = mixed ? "" : (values[0] ?? "");
  const submit = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed || (!mixed && trimmed === current)) return;
    void commit(effectRowActions(row, (effect) => ({ ...effect, params: { ...effect.params, [resourceKey]: trimmed } })));
  };
  return (
    <div className="grid grid-cols-[76px_minmax(0,1fr)] items-center gap-x-2">
      <label htmlFor={inputId} className="truncate text-[12px] text-muted-foreground">
        {resourceKey}
      </label>
      <input
        id={inputId}
        key={current}
        type="text"
        defaultValue={current}
        placeholder={mixed ? "Mixed" : undefined}
        autoComplete="off"
        spellCheck={false}
        onBlur={(event) => submit(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") submit(event.currentTarget.value);
        }}
        className="h-7 w-full min-w-0 rounded-md bg-raised px-2 text-[12px] text-foreground outline-none placeholder:text-dim focus-visible:ring-2 focus-visible:ring-ring"
      />
    </div>
  );
}

function EffectRowView({ row, descriptor }: { readonly row: EffectRow; readonly descriptor: VisualEffectDescriptor | undefined }) {
  const { commit } = usePropertyCommit();
  const name = descriptor?.displayName ?? row.effectType;
  const enabled = row.targets.every(({ effect }) => effect.enabled);
  const controls = effectParamControls(row.effectType, descriptor);
  return (
    <li className="flex flex-col gap-2.5 rounded-control border border-line p-2">
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-foreground">{name}</span>
        <Switch
          aria-label={`Enable ${name}`}
          checked={enabled}
          onCheckedChange={(checked) => void commit(effectRowActions(row, (effect) => ({ ...effect, enabled: checked })))}
        />
        <IconButton label={`Remove ${name}`} size="sm" onClick={() => void commit(effectRowActions(row, () => null))}>
          <Trash2 className="h-3.5 w-3.5" aria-hidden />
        </IconButton>
      </div>
      {enabled &&
        controls.map((control) => {
          const values = row.targets.map(({ effect }) => effectParamValue(effect, row.effectType, control));
          return (
            <SliderField
              key={control.key}
              label={control.label}
              value={values[0] ?? control.defaultValue}
              mixed={!sameValues(values)}
              min={control.min}
              max={control.max}
              step={control.step}
              format={formatNumber}
              onCommit={(value) => commit(effectRowActions(row, (effect) => withEffectParam(effect, control.key, value)))}
            />
          );
        })}
      {enabled && descriptor?.resourceKey && <ResourceField row={row} resourceKey={descriptor.resourceKey} />}
    </li>
  );
}

/** Applied effects with catalog parameter controls and remove; several items show the shared effects. */
export function EffectsSection({ items }: { readonly items: readonly TimelineItem[] }) {
  const catalog = useEffectCatalog();
  const rows = effectRows(items);
  const shared = items.length > 1;
  return (
    <PropertySection title="Effects">
      {rows.length === 0 ? (
        <p className="text-[12px] text-dim">
          {shared ? "No effect is applied to every selected item." : "No effects yet. Add one from the Effects tab."}
        </p>
      ) : (
        <ul aria-label={shared ? "Shared effects" : "Applied effects"} className="flex flex-col gap-2">
          {rows.map((row) => (
            <EffectRowView key={row.id} row={row} descriptor={catalog.effects.find((effect) => effect.id === row.effectType)} />
          ))}
        </ul>
      )}
      {rows.length > 0 && catalog.status === "ready" && catalog.effects.length === 0 && (
        <p className="text-[11px] text-dim">Effect catalog unavailable. Parameters can't be edited right now.</p>
      )}
    </PropertySection>
  );
}
