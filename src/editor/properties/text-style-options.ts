/** Font, size and color choices shared by the text and caption Style controls. */

interface Option {
  readonly value: string;
  readonly label: string;
}

/** Stored as `fontName`; export splits "Family-Face" into the FCPXML font and face. */
const fontNames = ["Helvetica", "Helvetica-Bold", "Inter", "Inter-Bold", "Georgia", "Menlo"] as const;

export const fontSizeRange = { min: 12, max: 200, step: 1 } as const;

/** Sentinel swatch value for "none"; committing it removes the property. */
export const noColor = "transparent";

/** The catalog fonts, plus the stored font when it is not one of them. */
export function fontOptions(current: string): Option[] {
  const options = fontNames.map((name) => ({ value: name, label: name.replace("-", " ") }));
  return options.some((option) => option.value === current) ? options : [{ value: current, label: current }, ...options];
}

export const textColorOptions: readonly Option[] = [
  { value: "#ffffff", label: "White" },
  { value: "#111111", label: "Black" },
  { value: "#ffcf5a", label: "Yellow" },
  { value: "#22d3ee", label: "Cyan" },
  { value: "#ff5c7a", label: "Pink" },
];

/** Caption highlight colors; yellow is the legacy word accent. */
export const highlightColorOptions: readonly Option[] = [
  { value: "#ffcf5a", label: "Yellow" },
  { value: "#22d3ee", label: "Cyan" },
  { value: "#ff5c7a", label: "Pink" },
];

export const strokeColorOptions: readonly Option[] = [
  { value: noColor, label: "No stroke" },
  { value: "#000000", label: "Black" },
  { value: "#ffffff", label: "White" },
  { value: "#22d3ee", label: "Cyan" },
];

export const backgroundColorOptions: readonly Option[] = [
  { value: noColor, label: "No background" },
  { value: "#000000", label: "Black" },
  { value: "rgba(0, 0, 0, 0.55)", label: "Translucent black" },
  { value: "#ffffff", label: "White" },
];
