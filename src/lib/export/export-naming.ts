import { formatDurationLabel } from "@/lib/format";

/**
 * Export file-name rules. The messages match `validate_export_file_name` in
 * `src-tauri/src/project/export_destination.rs` word for word.
 */
const maxExportNameBytes = 180;
const blankNameMessage = "Give the export a name.";
const slashMessage = "Export names can't contain slashes.";
const leadingDotMessage = "Export names can't start with a dot.";
const controlCharacterMessage = "Export names can't contain control characters.";
const tooLongMessage = `Export names can't be longer than ${maxExportNameBytes} bytes.`;

const controlCharacterPattern = /\p{Cc}/u;

export function exportFileNameProblem(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) return blankNameMessage;
  if (trimmed.includes("/") || trimmed.includes("\\")) return slashMessage;
  if (controlCharacterPattern.test(trimmed)) return controlCharacterMessage;
  if (trimmed.startsWith(".")) return leadingDotMessage;
  if (new TextEncoder().encode(trimmed).length > maxExportNameBytes) return tooLongMessage;
  return null;
}

function exportFileStem(name: string, extension: string): string {
  const trimmed = name.trim();
  const suffix = `.${extension.toLowerCase()}`;
  return trimmed.toLowerCase().endsWith(suffix)
    ? trimmed.slice(0, trimmed.length - suffix.length).trimEnd()
    : trimmed;
}

/** The file name an export saves as, before any collision suffix. */
export function exportFileLabel(name: string, extension: string): string {
  return `${exportFileStem(name, extension)}.${extension}`;
}

/** The file name of a recorded artifact path, relative or absolute. */
export function artifactFileName(path: string): string {
  const segments = path.split(/[\\/]/);
  return segments[segments.length - 1] || path;
}

interface SaveRangeNamingProject {
  readonly name: string;
  readonly timelines?: readonly { readonly id: string; readonly name: string }[] | null;
  readonly activeTimelineId?: string | null;
}

/** `<project or timeline name> mm:ss–mm:ss` for media saved from a timeline range. */
export function saveRangeMediaName(
  project: SaveRangeNamingProject,
  range: { readonly startSeconds: number; readonly endSeconds: number },
): string {
  const timelines = project.timelines ?? [];
  const activeTimelineId = project.activeTimelineId ?? "main";
  const baseName =
    timelines.length >= 2
      ? (timelines.find((entry) => entry.id === activeTimelineId)?.name ?? "")
      : project.name;
  const name = baseName.trim() || "Timeline range";
  return `${name} ${formatDurationLabel(range.startSeconds)}–${formatDurationLabel(range.endSeconds)}`;
}
