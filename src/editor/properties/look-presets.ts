import type { ProjectAction } from "@/lib/project";
import { colorGradeRanges, resetColorGradeAction, type ColorGradeValues } from "@/lib/properties/visual-properties";

export type LookPresetId = "none" | "film" | "warm" | "mono";

interface LookPreset {
  readonly value: LookPresetId;
  readonly label: string;
  /** The whole grade the preset stores; null clears the grade. */
  readonly grade: ColorGradeValues | null;
}

const neutral: ColorGradeValues = {
  exposure: colorGradeRanges.exposure.defaultValue,
  contrast: colorGradeRanges.contrast.defaultValue,
  saturation: colorGradeRanges.saturation.defaultValue,
  temperature: colorGradeRanges.temperature.defaultValue,
  tint: colorGradeRanges.tint.defaultValue,
};

/**
 * Look presets as `updateItemColorGrade` values. The renderer warms the image above 6500 K
 * (`temperature_tint` in the frame compositor), so Film and Warm raise the temperature.
 */
export const lookPresets: readonly LookPreset[] = [
  { value: "none", label: "None", grade: null },
  { value: "film", label: "Film", grade: { ...neutral, contrast: 1.1, saturation: 0.85, temperature: 6800 } },
  { value: "warm", label: "Warm", grade: { ...neutral, saturation: 1.1, temperature: 8000 } },
  { value: "mono", label: "B&W", grade: { ...neutral, contrast: 1.15, saturation: 0 } },
];

const gradeKeys = Object.keys(neutral) as (keyof ColorGradeValues)[];

function sameGrade(left: ColorGradeValues, right: ColorGradeValues): boolean {
  return gradeKeys.every((key) => Math.abs(left[key] - right[key]) < 0.000_1);
}

/** The preset matching a grade, "none" for the neutral grade, or null for a custom grade. */
export function lookPresetForGrade(grade: ColorGradeValues): LookPresetId | null {
  return lookPresets.find((preset) => sameGrade(grade, preset.grade ?? neutral))?.value ?? null;
}

/** None clears the grade; a preset replaces it (reset plus grade) so no earlier value lingers. */
export function lookPresetAction(itemIds: readonly string[], presetId: LookPresetId): ProjectAction {
  const grade = lookPresets.find((preset) => preset.value === presetId)?.grade;
  if (!grade) return resetColorGradeAction(itemIds);
  return { type: "updateItemColorGrade", itemIds, reset: true, grade: { ...grade } };
}
