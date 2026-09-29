/** Display text for Properties numeric fields; each stays parseable by the slider field. */
function trimNumber(value: number, digits = 2): string {
  return Number(value.toFixed(digits)).toString();
}

export const formatPercent = (value: number) => `${Math.round(value).toString()}%`;
export const formatDegrees = (value: number) => `${trimNumber(value, 1)}°`;
export const formatSeconds = (value: number) => `${trimNumber(value)}s`;
export const formatDecibels = (value: number) => `${trimNumber(value, 1)} dB`;
export const formatMultiplier = (value: number) => `${trimNumber(value)}×`;
export const formatNumber = (value: number) => trimNumber(value);

/** Fraction (0–1) ↔ percent slider units, rounded to avoid float noise. */
export const toPercent = (fraction: number) => Number((fraction * 100).toFixed(2));
export const fromPercent = (percent: number) => Number((percent / 100).toFixed(4));
